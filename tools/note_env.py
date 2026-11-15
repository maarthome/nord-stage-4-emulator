#!/usr/bin/env python3
"""Per-note envelope metrics (attack / hold / release) and level-versus-velocity slope.

Used by ``tools/emu_vs_hw.py`` style comparisons of a hardware clip and an emulator render that
share one note list. Only isolated notes count (no other onset within the analysis window), and
only medians are reported: overlapping notes and the pedal make single values noisy.

* attack: median 10 % to 90 % rise time of the 2 ms smoothed envelope after the onset (ms).
* hold: median time from the envelope peak until it falls 6 dB (ms).
* release: median time for the 10 ms smoothed level to fall 20 dB below its value at the key
  release (ms); notes whose release is followed by another onset within 1 s are skipped.
* velocity slope: least-squares dB of the note's 300 ms peak level per MIDI velocity unit.

EMPIRICAL heuristics, not firmware facts: they compare two recordings of the same notes.
"""

from __future__ import annotations

import math
import statistics
from typing import Any, Sequence

import numpy as np

PEAK_WINDOW_S = 0.3
HOLD_DROP_DB = 6.0
RELEASE_DROP_DB = 20.0
RELEASE_MAX_S = 2.0
ISOLATION_S = 0.35  # no other onset within this time after the note's own onset
MIN_PEAK = 1e-3  # about -60 dBFS


def smooth_abs(mono: np.ndarray, sr: int, seconds: float) -> np.ndarray:
    n = max(1, int(round(seconds * sr)))
    return np.convolve(np.abs(mono), np.ones(n) / n, mode="same")


def isolated(notes: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Notes with no other onset within ``ISOLATION_S`` after their own."""
    onsets = sorted(n["onset_s"] for n in notes)
    out = []
    for n in notes:
        t = n["onset_s"]
        if not any(t < o < t + ISOLATION_S for o in onsets):
            out.append(n)
    return out


def _first_at_or_above(w: np.ndarray, level: float) -> int:
    return int(np.argmax(w >= level))


def note_peak_db(mono: np.ndarray, sr: int, onset_s: float) -> float | None:
    """Peak of the 2 ms envelope in the 300 ms after the onset (dBFS), or None if silent."""
    env = smooth_abs(mono, sr, 0.002)
    a = int(onset_s * sr)
    w = env[a : a + int(PEAK_WINDOW_S * sr)]
    if len(w) < 8 or float(w.max()) < MIN_PEAK:
        return None
    return 20 * math.log10(float(w.max()))


def envelope_metrics(mono: np.ndarray, sr: int, notes: Sequence[dict[str, Any]]) -> dict[str, Any]:
    """Median attack / hold / release (ms) over the isolated ``notes``."""
    fast, slow = smooth_abs(mono, sr, 0.002), smooth_abs(mono, sr, 0.010)
    att: list[float] = []
    hold: list[float] = []
    rel: list[float] = []
    for n in isolated(notes):
        a = max(0, int((n["onset_s"] - 0.02) * sr))
        w = fast[a : int((n["onset_s"] + PEAK_WINDOW_S) * sr)]
        if len(w) < 8 or float(w.max()) < MIN_PEAK:
            continue
        peak = float(w.max())
        ip = int(np.argmax(w))
        att.append(max(0, _first_at_or_above(w, 0.9 * peak) - _first_at_or_above(w, 0.1 * peak)))
        tail = fast[a + ip : a + ip + int(2.0 * sr)]
        drop = np.nonzero(tail <= peak * 10 ** (-HOLD_DROP_DB / 20))[0]
        if len(drop):
            hold.append(float(drop[0]))
        later = [o["onset_s"] for o in notes if o["onset_s"] > n["offset_s"]]
        if later and min(later) < n["offset_s"] + 1.0:
            continue
        b = int(n["offset_s"] * sr)
        seg = slow[b : b + int(RELEASE_MAX_S * sr)]
        if len(seg) < 8 or float(seg[0]) < MIN_PEAK:
            continue
        under = np.nonzero(seg <= seg[0] * 10 ** (-RELEASE_DROP_DB / 20))[0]
        if len(under):
            rel.append(float(under[0]))

    def med(xs: list[float]) -> float | None:
        return round(statistics.median(xs) * 1000.0 / sr, 1) if xs else None

    return {
        "attack_ms": med(att),
        "hold_ms": med(hold),
        "release_ms": med(rel),
        "n_attack": len(att),
        "n_release": len(rel),
    }


def velocity_slope_db(
    mono: np.ndarray, sr: int, notes: Sequence[dict[str, Any]]
) -> tuple[float | None, int]:
    """Least-squares slope (dB per velocity unit) of note peak level on velocity, and the count."""
    xs: list[float] = []
    ys: list[float] = []
    for n in isolated(notes):
        db = note_peak_db(mono, sr, n["onset_s"])
        if db is not None and "velocity" in n:
            xs.append(float(n["velocity"]))
            ys.append(db)
    if len(xs) < 5 or max(xs) - min(xs) < 10:
        return None, len(xs)
    return round(float(np.polyfit(xs, ys, 1)[0]), 3), len(xs)
