#!/usr/bin/env python3
"""Piano-likeness and transcription-confidence flags for a full-length note stream.

A transcriber such as Transkun reports notes for any audio, so organ, synth, speech and noise
passages produce plausible-looking junk. Two cheap checks mark the stretches:

1. ``program_class``: a name heuristic for the program shown in the video (ASSUMED: derived from
   the program name only, not from the decoded engine mix; correct by hand where it is wrong).
2. ``f0_snr_db``: for every transcribed note, the spectral peak at its fundamental (+-40 cents)
   against the median magnitude in ``[f0 / 1.5, 2 f0]`` over 250 ms after the onset. A note the
   audio does not contain scores about 0 dB; a real pitched note scores well above ``SNR_OK_DB``.

Windows (1 s) combine both: ``piano_like`` is true when the program class is piano and the
window's notes are mostly supported by the audio (or it has no notes).
"""

from __future__ import annotations

import math
from typing import Any, Sequence

import numpy as np

WINDOW_S = 1.0
ANALYSIS_S = 0.25
ANALYSIS_DELAY_S = 0.03  # skip the click of the onset
NFFT = 16384
SNR_OK_DB = 8.0  # EMPIRICAL: below this the fundamental is not distinguishable from the floor
SUPPORT_MIN = 0.5  # fraction of window notes above SNR_OK_DB for the window to be "supported"

PIANO_WORDS = ("grand", "upright", "felt", "imperial", "piano")
EP_WORDS = ("wurly", "tines", "mk i", "mk ii", "spin ep", "rhodes", "digi keys")
ORGAN_WORDS = ("b3", "whl", "j3", "vox", "gospel", "farfisa")


def program_class(name: str) -> str:
    """``piano`` / ``ep`` / ``organ`` / ``other`` from a program name (heuristic)."""
    low = name.lower()
    for cls, words in (("piano", PIANO_WORDS), ("ep", EP_WORDS), ("organ", ORGAN_WORDS)):
        if any(w in low for w in words):
            return cls
    return "other"


def note_f0_snr_db(mono: np.ndarray, sr: int, midi: int, onset_s: float) -> float | None:
    """Fundamental-to-floor ratio (dB) of one note, or None when the clip is too short."""
    a = int((onset_s + ANALYSIS_DELAY_S) * sr)
    seg = mono[a : a + int(ANALYSIS_S * sr)]
    if len(seg) < int(0.05 * sr):
        return None
    spec = np.abs(np.fft.rfft(seg * np.hanning(len(seg)), NFFT))
    freqs = np.fft.rfftfreq(NFFT, 1.0 / sr)
    f0 = 440.0 * 2.0 ** ((midi - 69) / 12.0)
    near = (freqs >= f0 * 2 ** (-40 / 1200)) & (freqs <= f0 * 2 ** (40 / 1200))
    floor = (freqs >= f0 / 1.5) & (freqs <= f0 * 2.0)
    if not near.any() or not floor.any():
        return None
    ref = float(np.median(spec[floor])) + 1e-12
    return round(20 * math.log10(float(spec[near].max()) / ref + 1e-12), 1)


def program_at(programs: Sequence[dict[str, Any]], t: float) -> dict[str, Any] | None:
    """Program shown at time ``t`` (the last one whose ``first_seen_s`` is <= ``t``)."""
    cur = None
    for p in sorted(programs, key=lambda p: p["first_seen_s"]):
        if p["first_seen_s"] <= t:
            cur = p
    return cur


def piano_like(cls: str, silent: bool, supported: float | None) -> bool:
    """Window flag: a piano-class program with audio-supported notes (or none played).

    Without a program list (``unknown``, e.g. a tutorial video) only the audio support counts
    and at least one note must exist.
    """
    if silent:
        return False
    if cls == "piano":
        return supported is None or supported >= SUPPORT_MIN
    return cls == "unknown" and supported is not None and supported >= SUPPORT_MIN


def window_rows(
    mono: np.ndarray,
    sr: int,
    notes: Sequence[dict[str, Any]],
    programs: Sequence[dict[str, Any]] = (),
    t0: float = 0.0,
) -> list[dict[str, Any]]:
    """One row per ``WINDOW_S``: level, note count, supported fraction, program and flag.

    ``t0`` is the media time of the clip start (rows and program lookup use media time).
    Mutates each note by adding ``snr_db`` (None when it cannot be measured).
    """
    for n in notes:
        n["snr_db"] = note_f0_snr_db(mono, sr, n["midi"], n["onset_s"])
    dur = len(mono) / sr
    by_win: dict[int, list[dict[str, Any]]] = {}
    for n in notes:
        by_win.setdefault(int(n["onset_s"] // WINDOW_S), []).append(n)
    rows = []
    for w in range(int(math.ceil(dur / WINDOW_S))):
        seg = mono[int(w * WINDOW_S * sr) : int((w + 1) * WINDOW_S * sr)]
        rms = float(np.sqrt(np.mean(seg.astype(np.float64) ** 2))) if len(seg) else 0.0
        ns = by_win.get(w, [])
        good = [n for n in ns if n["snr_db"] is not None and n["snr_db"] >= SNR_OK_DB]
        supported = len(good) / len(ns) if ns else None
        prog = program_at(programs, t0 + w * WINDOW_S)
        cls = program_class(prog["name"]) if prog else "unknown"
        silent = rms < 1e-4
        rows.append(
            {
                "t": round(t0 + w * WINDOW_S, 3),
                "program": prog["program"] if prog else None,
                "class": cls,
                "rms_dbfs": round(20 * math.log10(max(rms, 1e-6)), 1),
                "notes": len(ns),
                "supported": None if supported is None else round(supported, 2),
                "piano_like": piano_like(cls, silent, supported),
            }
        )
    return rows


def program_ranges(rows: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Merges consecutive window rows of one program into ``[start_s, end_s)`` ranges."""
    out: list[dict[str, Any]] = []
    for r in rows:
        if out and out[-1]["program"] == r["program"]:
            out[-1]["end_s"] = r["t"] + WINDOW_S
            out[-1]["notes"] += r["notes"]
            out[-1]["piano_like_s"] += WINDOW_S if r["piano_like"] else 0.0
        else:
            out.append(
                {
                    "program": r["program"],
                    "class": r["class"],
                    "start_s": r["t"],
                    "end_s": r["t"] + WINDOW_S,
                    "notes": r["notes"],
                    "piano_like_s": WINDOW_S if r["piano_like"] else 0.0,
                }
            )
    return out
