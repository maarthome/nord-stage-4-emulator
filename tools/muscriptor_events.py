#!/usr/bin/env python3
"""MuScriptor note stream to ``ns4 render-patch --events`` JSON (``emu_vs_hw.py --events-from``).

MuScriptor (Kyutai / Mirelo, https://github.com/muscriptor/muscriptor; code MIT, weights
CC BY-NC 4.0, gated on Hugging Face) transcribes multi-instrument audio. ``muscriptor transcribe
-f json`` writes a flat list of ``start`` events (``pitch``, ``start_time``, ``instrument``,
``index``) and ``end`` events (``end_time``, ``start_event_index``). It produces **no velocity**,
so notes carry none and the replay uses ``--velocity`` (or ``--velocity N`` here to stamp one).
There is no sustain pedal either.

Needs the separate environment of the model (for example ``C:\\tmp\\ns4-muscriptor``); the
conversion here is pure Python and imports without torch.

Usage::

    python muscriptor_events.py run --media demo.webm --at 290 --duration 30 --workdir W \\
        --muscriptor C:/tmp/ns4-muscriptor/Scripts/muscriptor.exe --model medium \\
        --drop-instruments drums --events-out horizon_ms.json
    python muscriptor_events.py convert horizon.json --offset 290 --out stream.json
    python emu_vs_hw.py spec.json --events-from stream.json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable, Sequence

MIN_NOTE_S = 0.02


def pair_notes(events: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Joins ``start`` events with their ``end`` events (by ``start_event_index``).

    A note with no end event (cut at the clip end) closes at its own onset plus ``MIN_NOTE_S``.
    """
    ends = {e["start_event_index"]: e["end_time"] for e in events if e["type"] == "end"}
    notes = []
    for e in events:
        if e["type"] != "start":
            continue
        on = float(e["start_time"])
        off = max(float(ends.get(e["index"], on)), on + MIN_NOTE_S)
        notes.append(
            {
                "midi": int(e["pitch"]),
                "onset_s": on,
                "offset_s": off,
                "instrument": e.get("instrument", ""),
            }
        )
    notes.sort(key=lambda n: (n["onset_s"], n["midi"]))
    return notes


def to_doc(
    events: Sequence[dict[str, Any]],
    source: str,
    offset_s: float = 0.0,
    drop: Iterable[str] = (),
    keep: Iterable[str] | None = None,
    velocity: int | None = None,
) -> dict[str, Any]:
    """Builds the ``--events`` document; ``offset_s`` shifts clip time to media time.

    ``drop`` removes instruments (``drums`` is noise for pitched replay), ``keep`` (when given)
    keeps only those. ``velocity`` stamps every note; without it the notes carry none.
    """
    dropped, kept = set(drop), None if keep is None else set(keep)
    out = []
    for n in pair_notes(events):
        if n["instrument"] in dropped or (kept is not None and n["instrument"] not in kept):
            continue
        note = {
            **n,
            "onset_s": round(n["onset_s"] + offset_s, 4),
            "offset_s": round(n["offset_s"] + offset_s, 4),
        }
        if velocity is not None:
            note["velocity"] = velocity
        out.append(note)
    return {"source": source, "events": out, "pedal": []}


def instrument_counts(doc: dict[str, Any]) -> dict[str, int]:
    """Notes per instrument label of an events document."""
    counts: dict[str, int] = {}
    for e in doc["events"]:
        counts[e["instrument"]] = counts.get(e["instrument"], 0) + 1
    return dict(sorted(counts.items(), key=lambda kv: -kv[1]))


def _csv(text: str | None) -> list[str] | None:
    return None if text is None else [t for t in text.split(",") if t]


def cmd_convert(args: argparse.Namespace) -> int:
    events = json.loads(Path(args.input).read_text(encoding="utf-8"))
    doc = to_doc(
        events,
        args.source,
        args.offset,
        _csv(args.drop_instruments) or [],
        _csv(args.keep_instruments),
        args.velocity,
    )
    Path(args.out).write_text(json.dumps(doc, separators=(",", ":")), encoding="utf-8")
    print(f"{len(doc['events'])} notes {instrument_counts(doc)}")
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    import transcribe_clip

    work = Path(args.workdir)
    work.mkdir(parents=True, exist_ok=True)
    name = args.name or Path(args.media).stem
    wav, raw = work / f"{name}.wav", work / f"{name}.{args.model}.json"
    if not wav.exists():
        transcribe_clip.extract_audio(Path(args.media), wav, args.at, args.duration)
    if not raw.exists():
        cmd = [args.muscriptor, "transcribe", "--model", args.model, "-f", "json"]
        if args.instruments:
            cmd += ["--instruments", args.instruments]
        subprocess.run([*cmd, "-o", str(raw), str(wav)], check=True)
    ns = argparse.Namespace(**{**vars(args), "input": str(raw), "offset": args.at or 0.0})
    ns.source = f"muscriptor-{args.model}"
    ns.out = args.events_out
    return cmd_convert(ns)


def _add_filters(p: argparse.ArgumentParser) -> None:
    p.add_argument("--drop-instruments", help="Comma list, e.g. drums.")
    p.add_argument("--keep-instruments", help="Comma list; others are dropped.")
    p.add_argument("--velocity", type=int, help="Stamp this velocity (1..127) on every note.")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("convert", help="muscriptor -f json output to an events document.")
    c.add_argument("input")
    c.add_argument("--out", required=True)
    c.add_argument("--offset", type=float, default=0.0, help="Clip start (s) in media time.")
    c.add_argument("--source", default="muscriptor")
    _add_filters(c)
    c.set_defaults(fn=cmd_convert)
    r = sub.add_parser("run", help="Extract a clip, run the muscriptor CLI, convert.")
    r.add_argument("--media", required=True)
    r.add_argument("--workdir", required=True)
    r.add_argument("--events-out", required=True)
    r.add_argument("--muscriptor", default="muscriptor", help="Path of the muscriptor CLI.")
    r.add_argument("--model", default="medium", choices=["small", "medium", "large"])
    r.add_argument("--instruments", help="Instrument hint passed to muscriptor (comma list).")
    r.add_argument("--name")
    r.add_argument("--at", type=float)
    r.add_argument("--duration", type=float)
    _add_filters(r)
    r.set_defaults(fn=cmd_run)
    return p


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return int(args.fn(args))


if __name__ == "__main__":
    sys.exit(main())
