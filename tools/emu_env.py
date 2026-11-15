"""Per-note envelope metrics of a mix for ``emu_vs_hw.py``: hold decay and release drop.

The demo clips are dense polyphony, so no note decays or releases in isolation and the T60 of
``emu_vs_hw.tail_t60_s`` finds nothing. These two metrics compare the same windows of the mix
around every played note in the hardware clip and in the emulator render (same notes, same
polyphony), so the other notes cancel to first order in the emulator-minus-hardware delta.
EMPIRICAL heuristics: only the medians over many notes are reported.
"""

from __future__ import annotations

import statistics
from typing import Sequence

import numpy as np

EARLY_S = (0.05, 0.20)  # window after the onset: the note near its peak
LATE_S = (0.45, 0.60)  # window after the onset: the held note
MIN_HOLD_S = 0.65  # a note qualifies for the decay when it is held at least this long
PRE_S = (-0.15, -0.01)  # window before the release
POST_S = (0.10, 0.25)  # window after the release
FLOOR_DB = -80.0


def window_db(mono: np.ndarray, sr: int, t0: float, span: tuple[float, float]) -> float | None:
    """Mean-square level (dB re 1.0) of ``mono`` in ``t0 + span``, or None outside the signal."""
    a = int(round((t0 + span[0]) * sr))
    b = int(round((t0 + span[1]) * sr))
    if a < 0 or b > len(mono) or b - a < 8:
        return None
    ms = float(np.mean(np.square(mono[a:b], dtype=np.float64)))
    return 10.0 * float(np.log10(max(ms, 1e-12)))


def hold_decay_db(
    mono: np.ndarray, sr: int, intervals: Sequence[tuple[float, float]]
) -> float | None:
    """Median level change (dB) from the early to the late window of notes held past ``MIN_HOLD_S``.

    More negative means a faster decay. Notes whose early window is below ``FLOOR_DB`` are skipped.
    """
    out: list[float] = []
    for on, off in intervals:
        if off - on < MIN_HOLD_S:
            continue
        e = window_db(mono, sr, on, EARLY_S)
        la = window_db(mono, sr, on, LATE_S)
        if e is None or la is None or e < FLOOR_DB:
            continue
        out.append(la - e)
    return round(statistics.median(out), 2) if out else None


def release_drop_db(
    mono: np.ndarray, sr: int, intervals: Sequence[tuple[float, float]]
) -> float | None:
    """Median level change (dB) from just before to just after a note release.

    A release counts when no other note starts between 50 ms before the release and the end of
    the post window, and the note was held at least 0.2 s. More negative means a faster release.
    """
    onsets = sorted(a for a, _ in intervals)
    out: list[float] = []
    for on, off in intervals:
        if off - on < 0.2 or any(
            a != on and off - 0.05 < a < off + POST_S[1] for a in onsets
        ):
            continue
        pre = window_db(mono, sr, off, PRE_S)
        post = window_db(mono, sr, off, POST_S)
        if pre is None or post is None or pre < FLOOR_DB:
            continue
        out.append(post - pre)
    return round(statistics.median(out), 2) if out else None
