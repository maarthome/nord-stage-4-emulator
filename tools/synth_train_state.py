#!/usr/bin/env python3
"""Exact-resume state for ``tools/synth_train.py``.

The adapter folders (``last``, ``step_N``) hold only weights. Next to the weights the trainer
writes ``train_state.pt`` with everything else a continued run needs: optimizer moments,
scheduler position, the gradient-norm history behind the adaptive clip, the best validation NLL,
the step number, elapsed seconds, the sampler RNG and the torch RNGs. With it
``synth_train.py --resume <folder>`` continues as if the run had never stopped (the same batches
and, on the same GPU and drivers, the same numbers up to non-deterministic CUDA kernels).

Size: about two floats of Adam state per trainable parameter, 8 MB for the 1.04M-parameter LoKr arm.
Torch is imported inside the functions, so the module imports without it.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

STATE_FILE = "train_state.pt"


def save_state(
    folder: Path,
    *,
    step: int,
    total_steps: int,
    opt: Any,
    sched: Any,
    history: Any,
    best: float,
    elapsed: float,
    sampler_rng: Any,
) -> None:
    import torch

    torch.save(
        {
            "step": step,
            "total_steps": total_steps,
            "opt": opt.state_dict(),
            "sched": sched.state_dict(),
            "history": list(history),
            "best": best,
            "elapsed": elapsed,
            "sampler_rng": sampler_rng.getstate(),
            "torch_rng": torch.get_rng_state(),
            "cuda_rng": torch.cuda.get_rng_state_all() if torch.cuda.is_available() else None,
        },
        Path(folder) / STATE_FILE,
    )


def load_state(folder: Path) -> dict[str, Any]:
    import torch

    path = Path(folder) / STATE_FILE
    if not path.exists():
        raise FileNotFoundError(f"{path} missing: this folder was saved without resume state")
    return torch.load(path, map_location="cpu", weights_only=False)


def apply_state(
    state: dict[str, Any], *, total_steps: int, opt: Any, sched: Any, history: Any, sampler_rng: Any
) -> tuple[int, float, float]:
    """Restores optimizer, scheduler, history, RNGs; returns ``(step, best, elapsed)``."""
    import torch

    if state["total_steps"] != total_steps:
        raise ValueError(
            f"run was saved for {state['total_steps']} steps, got --steps {total_steps}: "
            "the learning-rate schedule would differ"
        )
    opt.load_state_dict(state["opt"])
    sched.load_state_dict(state["sched"])
    history.clear()
    history.extend(state["history"])
    sampler_rng.setstate(state["sampler_rng"])
    torch.set_rng_state(state["torch_rng"])
    if state["cuda_rng"] is not None and torch.cuda.is_available():
        torch.cuda.set_rng_state_all(state["cuda_rng"])
    return state["step"], state["best"], state["elapsed"]
