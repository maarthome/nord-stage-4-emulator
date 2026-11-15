#!/usr/bin/env python3
"""Standard MIDI file to ``ns4 render-patch --events`` JSON (notes, velocities, sustain pedal).

Pure standard library so the conversion is testable without torch. Input is the MIDI that a
piano transcriber (``tools/transcribe_clip.py``, Transkun) writes; output is the event format
read by ``parse_note_events`` in ``crates/nord-core/src/render.rs``::

    {"source": ..., "events": [{"midi", "onset_s", "offset_s", "velocity"}],
     "pedal": [{"t", "value"}]}

Only numbers are produced (no audio). Notes still held at the end of the file are closed there.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path
from typing import Any, Callable, Sequence

DEFAULT_TEMPO_US = 500_000  # 120 bpm, the SMF default until a tempo meta event
SUSTAIN_CC = 64
MIN_NOTE_S = 0.02  # zero-length notes are not renderable (`offset_s > onset_s`)

Row = tuple[int, str, tuple[int, ...]]


def read_vlq(data: bytes, pos: int) -> tuple[int, int]:
    """Reads a MIDI variable-length quantity; returns ``(value, next_pos)``."""
    value = 0
    while True:
        b = data[pos]
        pos += 1
        value = (value << 7) | (b & 0x7F)
        if not b & 0x80:
            return value, pos


def parse_track(data: bytes, pos: int, end: int) -> list[Row]:
    """Parses one ``MTrk`` body into ``(abs_tick, kind, args)`` rows.

    Kinds: ``on`` (note, velocity), ``off`` (note,), ``cc`` (controller, value),
    ``tempo`` (microseconds per quarter note,).
    """
    rows: list[Row] = []
    tick, status = 0, 0
    while pos < end:
        delta, pos = read_vlq(data, pos)
        tick += delta
        b = data[pos]
        if b == 0xFF:  # meta
            kind = data[pos + 1]
            length, p = read_vlq(data, pos + 2)
            if kind == 0x51 and length == 3:
                rows.append((tick, "tempo", (int.from_bytes(data[p : p + 3], "big"),)))
            pos = p + length
            continue
        if b in (0xF0, 0xF7):  # sysex
            length, pos = read_vlq(data, pos + 1)
            pos += length
            continue
        if b & 0x80:
            status = b
            pos += 1
        high = status & 0xF0
        n_args = 1 if high in (0xC0, 0xD0) else 2
        args = tuple(data[pos : pos + n_args])
        pos += n_args
        if high == 0x90 and args[1] > 0:
            rows.append((tick, "on", args))
        elif high in (0x80, 0x90):
            rows.append((tick, "off", args[:1]))
        elif high == 0xB0:
            rows.append((tick, "cc", args))
    return rows


def tick_seconds(tempos: Sequence[tuple[int, int]], division: int) -> Callable[[int], float]:
    """Returns ``f(tick) -> seconds`` for a ``(tick, us_per_quarter)`` tempo map."""
    segs: list[tuple[int, float, int]] = []  # (start_tick, start_s, tempo)
    t_prev, s_prev, tempo = 0, 0.0, DEFAULT_TEMPO_US
    for tk, us in sorted(tempos):
        s_prev += (tk - t_prev) * tempo / division / 1e6
        t_prev, tempo = tk, us
        segs.append((tk, s_prev, us))

    def f(tick: int) -> float:
        start, s0, us = 0, 0.0, DEFAULT_TEMPO_US
        for sg in segs:
            if sg[0] <= tick:
                start, s0, us = sg
        return s0 + (tick - start) * us / division / 1e6

    return f


def _close(
    notes: list[dict[str, Any]], held: dict[int, tuple[float, int]], n: int, s: float
) -> None:
    on, vel = held.pop(n)
    notes.append(
        {
            "midi": n,
            "onset_s": round(on, 4),
            "offset_s": round(max(s, on + MIN_NOTE_S), 4),
            "velocity": vel,
        }
    )


def parse_midi(data: bytes) -> dict[str, Any]:
    """Parses SMF bytes into ``{"notes": [...], "pedal": [...]}`` in seconds."""
    if data[:4] != b"MThd":
        raise ValueError("not a standard MIDI file (no MThd)")
    _fmt, ntrks, division = struct.unpack(">HHH", data[8:14])
    if division & 0x8000:
        raise ValueError("SMPTE time division is not supported")
    pos = 8 + struct.unpack(">I", data[4:8])[0]
    rows: list[Row] = []
    for _ in range(ntrks):
        if data[pos : pos + 4] != b"MTrk":
            raise ValueError("expected MTrk chunk")
        size = struct.unpack(">I", data[pos + 4 : pos + 8])[0]
        rows += parse_track(data, pos + 8, pos + 8 + size)
        pos += 8 + size
    to_s = tick_seconds([(t, a[0]) for t, k, a in rows if k == "tempo"], division)
    rows.sort(key=lambda r: r[0])
    held: dict[int, tuple[float, int]] = {}
    notes: list[dict[str, Any]] = []
    pedal: list[dict[str, Any]] = []
    for tick, kind, a in rows:
        s = to_s(tick)
        if kind == "on":
            if a[0] in held:  # re-strike without off: close the previous note
                _close(notes, held, a[0], s)
            held[a[0]] = (s, a[1])
        elif kind == "off" and a[0] in held:
            _close(notes, held, a[0], s)
        elif kind == "cc" and a[0] == SUSTAIN_CC:
            if not pedal or pedal[-1]["value"] != a[1]:
                pedal.append({"t": round(s, 4), "value": a[1]})
    end = max((to_s(r[0]) for r in rows), default=0.0)
    for note in list(held):
        _close(notes, held, note, end)
    notes.sort(key=lambda n: (n["onset_s"], n["midi"]))
    return {"notes": notes, "pedal": pedal}


def to_events(parsed: dict[str, Any], source: str, offset_s: float = 0.0) -> dict[str, Any]:
    """Builds the ``--events`` document; ``offset_s`` shifts times (clip start in the video)."""
    ev = [
        {
            **n,
            "onset_s": round(n["onset_s"] + offset_s, 4),
            "offset_s": round(n["offset_s"] + offset_s, 4),
        }
        for n in parsed["notes"]
    ]
    ped = [{"t": round(p["t"] + offset_s, 4), "value": p["value"]} for p in parsed["pedal"]]
    return {"source": source, "events": ev, "pedal": ped}


def slice_events(doc: dict[str, Any], start_s: float, duration_s: float) -> dict[str, Any]:
    """Cuts ``[start_s, start_s + duration_s)`` and re-bases times to 0 (for a segment replay).

    Notes starting inside the window are kept (offsets clipped to the window end). The pedal
    state in force at ``start_s`` is carried as a ``t = 0`` event.
    """
    end = start_s + duration_s
    ev = [
        {
            **e,
            "onset_s": round(e["onset_s"] - start_s, 4),
            "offset_s": round(min(e["offset_s"], end) - start_s, 4),
        }
        for e in doc["events"]
        if start_s <= e["onset_s"] < end
    ]
    state = 0
    ped: list[dict[str, Any]] = []
    for p in doc.get("pedal", []):
        if p["t"] <= start_s:
            state = p["value"]
        elif p["t"] < end:
            ped.append({"t": round(p["t"] - start_s, 4), "value": p["value"]})
    ped.insert(0, {"t": 0.0, "value": state})
    rest = {k: v for k, v in doc.items() if k not in ("events", "pedal")}
    return {**rest, "events": ev, "pedal": ped}


def write_csv(doc: dict[str, Any], path: Path) -> None:
    """Writes notes and pedal rows as ``kind,time_s,end_s,midi,velocity,value``."""
    lines = ["kind,time_s,end_s,midi,velocity,value"]
    for e in doc["events"]:
        lines.append(f"note,{e['onset_s']},{e['offset_s']},{e['midi']},{e['velocity']},")
    for p in doc.get("pedal", []):
        lines.append(f"pedal,{p['t']},,,,{p['value']}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("midi", type=Path)
    ap.add_argument("--out", type=Path, required=True, help="Events JSON to write.")
    ap.add_argument("--csv", type=Path, help="Also write a CSV.")
    ap.add_argument("--source", default="transkun")
    ap.add_argument("--offset", type=float, default=0.0, help="Add seconds to all times.")
    args = ap.parse_args(argv)
    doc = to_events(parse_midi(args.midi.read_bytes()), args.source, args.offset)
    args.out.write_text(json.dumps(doc), encoding="utf-8")
    if args.csv:
        write_csv(doc, args.csv)
    print(f"{len(doc['events'])} notes, {len(doc['pedal'])} pedal events")
    return 0


if __name__ == "__main__":
    sys.exit(main())
