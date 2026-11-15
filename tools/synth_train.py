#!/usr/bin/env python3
"""Train a LoKr or LoRA adapter on Transkun 2.0 from a ``tools/synth_dataset.py`` dataset.

    C:/tmp/ns4-ft-venv/Scripts/python.exe tools/synth_train.py C:/tmp/ns4-ft-data \
        --arm lokr --steps 8000 --out C:/tmp/ns4-ft-runs/lokr1

Plan and targets: ``docs/research/transkun-synth-finetune-plan.md`` (sections 7.2 and 7.3).
Loss, as upstream ``transkun.train``: ``-log_prob(audio, notes).sum(-1).mean()`` (semi-CRF
intervals + velocity + refined onset/offset + presence). The optimizer is AdamW with a OneCycle
schedule (5 % warm-up, ``div_factor`` 20, ``final_div_factor`` 2, as upstream); gradient norms
are clipped at the 0.8 quantile of the last 100 norms, as upstream's adaptive clipping.
ASSUMED: AdamW instead of upstream's AdaBelief (same state size), weight decay 1e-4 as upstream.

Every ``--eval-every`` steps: validation NLL on fixed held-out segments, onset F1 / velocity MAE
on ``--eval-items`` held-out items (``synth_eval``), and a checkpoint (``step_N``, ``last`` and
``best`` by validation NLL). Output, never committed: adapter folders, ``log.csv``,
``evals.jsonl``.
"""

from __future__ import annotations

import argparse
import collections
import csv
import json
import sys
import time
from collections.abc import Sequence
from pathlib import Path
from typing import Any

import numpy as np
import synth_adapter
import synth_eval
import synth_train_data as std

GRAD_WINDOW = 100
CLIP_QUANTILE = 0.8


def to_notes(batch: list[list[dict[str, Any]]]) -> list[list[Any]]:
    from transkun.Data import Note

    return [
        [
            Note(n["start"], n["end"], n["pitch"], n["velocity"], n["has_onset"], n["has_offset"])
            for n in notes
        ]
        for notes in batch
    ]


def clip_value(history: collections.deque) -> float:
    """Adaptive gradient-clip threshold: quantile of recent norms, infinite while warming up."""
    if len(history) < 10:
        return float("inf")
    return float(np.quantile(np.array(history), CLIP_QUANTILE))


def nll(model: Any, audio: np.ndarray, notes: list[list[dict[str, Any]]], device: str) -> Any:
    import torch

    x = torch.from_numpy(np.ascontiguousarray(audio)).to(device)
    return -model.log_prob(x, to_notes(notes)).sum(-1).mean()


def validation_nll(model: Any, segments: list, device: str) -> float:
    import torch

    model.eval()
    total = 0.0
    with torch.no_grad():
        for audio, notes in segments:
            total += float(nll(model, audio[None, :, None], [notes], device))
    model.train()
    return total / max(1, len(segments))


def val_scores(model: Any, root: Path, entries: list, device: str, tol: float) -> dict[str, Any]:
    import torch

    model.eval()
    est = {}
    with torch.no_grad():
        for e in entries:
            est[e["id"]] = synth_eval.transcribe_file(model, root / e["audio"], device)
    model.train()
    return synth_eval.score(entries, est, tol)["all"]


def run_eval(step: int, model: Any, peft_model: Any, ctx: dict, best: float) -> float:
    """Validation NLL + onset F1 / velocity MAE, checkpoint ``last`` and ``best``; returns best."""
    v = validation_nll(model, ctx["val_segments"], ctx["device"])
    scores = val_scores(model, ctx["root"], ctx["f1_entries"], ctx["device"], 0.05)
    rec = {
        "step": step,
        "val_nll": round(v, 2),
        "seconds": round(time.time() - ctx["t0"]),
        **scores,
    }
    with (ctx["out"] / "evals.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec) + chr(10))
    print("EVAL", rec, flush=True)
    synth_adapter.save_adapter(peft_model, ctx["out"] / "last")
    if v < best:
        synth_adapter.save_adapter(peft_model, ctx["out"] / "best")
        return v
    return best


def train(args: argparse.Namespace) -> None:
    import torch

    torch.manual_seed(args.seed)
    device = "cuda"
    root, out = Path(args.root), Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    model = synth_adapter.load_base(device)
    peft_model = synth_adapter.attach(model, args.arm, args.r, args.factor)
    n_train = synth_adapter.count_trainable(peft_model)
    print(f"arm={args.arm} trainable parameters: {n_train}", flush=True)
    train_s = std.SegmentSampler(root, std.load_entries(root, "train"), args.seg, args.seed)
    val_entries = std.load_entries(root, "val")
    val_segments = std.SegmentSampler(root, val_entries, args.seg, 0).fixed(args.val_segments)
    f1_entries = val_entries[:: max(1, len(val_entries) // args.eval_items)][: args.eval_items]
    params = [p for p in peft_model.parameters() if p.requires_grad]
    opt = torch.optim.AdamW(params, lr=args.lr, weight_decay=1e-4)
    sched = torch.optim.lr_scheduler.OneCycleLR(
        opt,
        args.lr,
        total_steps=args.steps,
        pct_start=0.05,
        div_factor=20,
        final_div_factor=2,
        cycle_momentum=False,
    )
    history: collections.deque = collections.deque(maxlen=GRAD_WINDOW)
    best, t0 = float("inf"), time.time()
    log = csv.writer((out / "log.csv").open("w", newline=""))
    log.writerow(["step", "loss", "grad_norm", "clip", "lr", "seconds"])
    ctx = {
        "root": root,
        "out": out,
        "val_segments": val_segments,
        "f1_entries": f1_entries,
        "device": device,
        "t0": t0,
    }
    best = run_eval(0, model, peft_model, ctx, best)  # baseline of the untrained adapter
    model.train()
    for step in range(1, args.steps + 1):
        audio, notes = train_s.batch(args.bs)
        opt.zero_grad(set_to_none=True)
        with torch.autocast("cuda", dtype=torch.bfloat16, enabled=args.bf16):
            loss = nll(model, audio, notes, device)
        loss.backward()
        clip = clip_value(history)
        gnorm = float(torch.nn.utils.clip_grad_norm_(params, clip))
        history.append(gnorm)
        opt.step()
        sched.step()
        if not np.isfinite(float(loss)):
            raise RuntimeError(f"non-finite loss at step {step}")
        if step % args.log_every == 0 or step == 1:
            row = [
                step,
                round(float(loss), 2),
                round(gnorm, 2),
                round(clip, 2),
                f"{sched.get_last_lr()[0]:.3g}",
                round(time.time() - t0),
            ]
            log.writerow(row)
            print(*row, flush=True)
        if step % args.eval_every == 0 or step == args.steps:
            best = run_eval(step, model, peft_model, ctx, best)


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("root", help="Dataset folder (dataset.json, audio/).")
    p.add_argument("--arm", choices=synth_adapter.ARMS, default="lokr")
    p.add_argument("--r", type=int)
    p.add_argument("--factor", type=int, help="LoKr decompose_factor (default 4).")
    p.add_argument("--steps", type=int, default=8000)
    p.add_argument("--bs", type=int, default=4)
    p.add_argument("--seg", type=float, default=std.SEG_S)
    p.add_argument("--lr", type=float, default=2e-4)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--bf16", action="store_true")
    p.add_argument("--log-every", type=int, default=20)
    p.add_argument("--eval-every", type=int, default=1000)
    p.add_argument("--eval-items", type=int, default=24, help="Held-out items for F1 per eval.")
    p.add_argument("--val-segments", type=int, default=24, help="Held-out segments for NLL.")
    p.add_argument("--out", required=True, help="Run folder (outside the repo).")
    return p


def main(argv: Sequence[str] | None = None) -> int:
    train(build_parser().parse_args(argv))
    return 0


if __name__ == "__main__":
    sys.exit(main())
