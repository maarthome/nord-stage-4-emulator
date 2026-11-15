#!/usr/bin/env python3
"""Per-octave partial decay (T60) of piano notes in a hardware clip and an emulator render.

Both signals share one note list (``tools/emu_vs_hw.py --events-from transkun``), so the sustain
pedal and the polyphony are the same. For every isolated note the level of the fundamental
(Hann-windowed DFT at ``f0``, best of +-25 cents) is tracked in dB from ``FIT_START_S`` after the
onset until the same pitch is struck again or ``FIT_END_S``; the slope of a line fit gives
``T60 = -60 / slope``. Medians per octave are compared. EMPIRICAL heuristic: the clips are lossy and
reverberant, so only the emulator-minus-hardware delta carries information.

Usage: ``python tools/piano_decay.py --hw hw.wav --emu emu.wav --events events.json [--emu ...]``.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path
from typing import Any, Sequence

import numpy as np

HOP_S = 0.02
FIT_START_S = 0.25  # skip the attack and the hammer noise
FIT_END_S = 3.0
MIN_SPAN_S = 0.8  # shortest fit window
MIN_POINTS = 8
DYN_RANGE_DB = 35.0  # only points within this range under the peak
CLEAR_SEMITONES = (-1, 1, -12, 12, 19, -19)  # neighbours whose partials land on the fundamental
CENTS_SEARCH = (-25.0, -12.0, 0.0, 12.0, 25.0)
OCTAVES = ((21, 47), (48, 59), (60, 71), (72, 83), (84, 108))
MAX_T60_S = 30.0


def octave_label(midi: int) -> str:
    for lo, hi in OCTAVES:
        if lo <= midi <= hi:
            return f"{lo}-{hi}"
    return "other"


def window_s(midi: int) -> float:
    """Analysis window: long enough to resolve a semitone at the fundamental."""
    return 0.25 if midi < 48 else 0.12


def f0_track_db(
    mono: np.ndarray, sr: int, midi: int, t0: float, t1: float
) -> tuple[np.ndarray, np.ndarray]:
    """Times and dB level of the fundamental of ``midi`` in ``[t0, t1]`` (hops of ``HOP_S``)."""
    win = int(round(window_s(midi) * sr))
    hop = max(1, int(round(HOP_S * sr)))
    a = int(round(t0 * sr))
    starts = np.arange(a, int(round(t1 * sr)) - win, hop)
    if len(starts) == 0 or starts[-1] + win > len(mono):
        starts = starts[starts + win <= len(mono)]
    if len(starts) == 0:
        return np.zeros(0), np.zeros(0)
    hann = np.hanning(win)
    n = np.arange(win)
    f0 = 440.0 * 2.0 ** ((midi - 69) / 12.0)
    best = np.zeros(len(starts))
    frames = np.stack([mono[s : s + win] * hann for s in starts])
    for cents in CENTS_SEARCH:
        f = f0 * 2.0 ** (cents / 1200.0)
        basis = np.exp(-2j * np.pi * f * n / sr)
        best = np.maximum(best, np.abs(frames @ basis))
    db = 20.0 * np.log10(np.maximum(best / (win / 2.0), 1e-9))
    return (starts + win / 2.0) / sr, db


def isolated(notes: Sequence[dict[str, Any]], idx: int, span_s: float) -> bool:
    """No other note near ``midi`` (or its octave / twelfth) sounds in the fit window."""
    me = notes[idx]
    for j, o in enumerate(notes):
        if j == idx:
            continue
        if (o["midi"] - me["midi"]) in CLEAR_SEMITONES and me["onset_s"] - 0.3 < o["onset_s"] < me[
            "onset_s"
        ] + span_s:
            return False
        if (
            o["midi"] == me["midi"]
            and me["onset_s"] < o["onset_s"] < me["onset_s"] + MIN_SPAN_S + FIT_START_S
        ):
            return False
    return True


def fit_end(notes: Sequence[dict[str, Any]], idx: int) -> float:
    """End of the fit window: the next onset of the same pitch or ``FIT_END_S``."""
    me = notes[idx]
    nxt = [o["onset_s"] for o in notes if o["midi"] == me["midi"] and o["onset_s"] > me["onset_s"]]
    end = me["onset_s"] + FIT_END_S
    return min([end] + [t - 0.02 for t in nxt])


def note_slope_db_s(
    mono: np.ndarray, sr: int, notes: Sequence[dict[str, Any]], idx: int
) -> float | None:
    """Decay slope (dB/s) of one note's fundamental, or None when it does not qualify."""
    me = notes[idx]
    end = fit_end(notes, idx)
    t0 = me["onset_s"] + FIT_START_S
    if end - t0 < MIN_SPAN_S or not isolated(notes, idx, end - me["onset_s"]):
        return None
    t, db = f0_track_db(mono, sr, me["midi"], me["onset_s"], end)
    sel = t >= t0
    t, db = t[sel], db[sel]
    if len(t) < MIN_POINTS:
        return None
    keep = db >= db.max() - DYN_RANGE_DB
    if keep.sum() < MIN_POINTS:
        return None
    return float(np.polyfit(t[keep], db[keep], 1)[0])


def t60_by_octave(
    mono: np.ndarray, sr: int, notes: Sequence[dict[str, Any]]
) -> dict[str, dict[str, float]]:
    """Per octave: median slope (dB/s), T60 (s) and note count, over qualifying notes."""
    groups: dict[str, list[float]] = {}
    for i, n in enumerate(notes):
        s = note_slope_db_s(mono, sr, notes, i)
        if s is not None:
            groups.setdefault(octave_label(n["midi"]), []).append(s)
    out: dict[str, dict[str, float]] = {}
    for label, slopes in groups.items():
        med = statistics.median(slopes)
        t60 = MAX_T60_S if med > -60.0 / MAX_T60_S else -60.0 / med
        out[label] = {"slope_db_s": round(med, 2), "t60_s": round(t60, 2), "n": len(slopes)}
    return out


def read_mono(path: Path) -> tuple[int, np.ndarray]:
    import video_ref

    sr, left, right = video_ref.read_wav_stereo(path)
    return sr, 0.5 * (np.asarray(left) + np.asarray(right))


def compare(
    hw: dict[str, dict[str, float]], emu: dict[str, dict[str, float]]
) -> list[dict[str, Any]]:
    """Rows per octave present in both: hardware and emulator T60 and the slope delta."""
    rows = []
    for label in sorted(set(hw) & set(emu), key=lambda s: int(s.split("-")[0])):
        rows.append(
            {
                "octave": label,
                "n_hw": hw[label]["n"],
                "n_emu": emu[label]["n"],
                "t60_hw_s": hw[label]["t60_s"],
                "t60_emu_s": emu[label]["t60_s"],
                "slope_delta_db_s": round(emu[label]["slope_db_s"] - hw[label]["slope_db_s"], 2),
            }
        )
    return rows


def main(argv: Sequence[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--hw", required=True)
    p.add_argument("--emu", required=True, nargs="+")
    p.add_argument("--events", required=True, help="Transcribed events JSON of the segment.")
    p.add_argument("--json-out", help="Write {render: rows} (derived numbers only) here.")
    args = p.parse_args(argv)
    notes = json.loads(Path(args.events).read_text(encoding="utf-8"))["events"]
    sr, hw = read_mono(Path(args.hw))
    hw_t = t60_by_octave(hw, sr, notes)
    doc: dict[str, list[dict[str, Any]]] = {}
    print(
        "| render | octave | n hw/emu | T60 hw s | T60 emu s | slope delta dB/s |\n"
        "|---|---|---|---|---|---|"
    )
    for path in args.emu:
        sr_e, emu = read_mono(Path(path))
        rows = compare(hw_t, t60_by_octave(emu, sr_e, notes))
        doc[Path(path).stem] = rows
        for r in rows:
            print(
                f"| {Path(path).stem} | {r['octave']} | {r['n_hw']}/{r['n_emu']} | {r['t60_hw_s']} "
                f"| {r['t60_emu_s']} | {r['slope_delta_db_s']:+} |"
            )
    if args.json_out:
        Path(args.json_out).write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
