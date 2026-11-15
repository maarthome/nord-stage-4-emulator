#!/usr/bin/env python3
"""Seeded note phrases for the synth fine-tuning dataset (``tools/synth_dataset.py``).

Output is the ``ns4 render-patch --events`` note list: ``{"midi", "onset_s", "offset_s",
"velocity"}``. Rules that Transkun's training code needs (``transkun.Data.validateNotes``,
``prepareIntervals``): no two notes of one pitch overlap, and consecutive same-pitch notes are
separated by at least ``MIN_GAP_S`` so they stay distinct after the 1024-sample (23 ms) frame
quantisation.

Provenance (``.agents/rules/provenance-in-code.md``):

- ASSUMED: the velocity law ``1 + 126 * Beta(1.6, 2.6)`` (median about 45). The reference clips
  are played soft (transcribed median 36 to 65, ``docs/research/piano-transcription.md``
  section 3); confirm by comparing the dataset histogram to the transcribed streams.
- ASSUMED: note lengths, gaps and the phrase mix are plausible playing, not measured.
"""

from __future__ import annotations

import random
from typing import Any

KINDS = ("melody", "chords", "arpeggio", "repeats", "mixed")
MIN_GAP_S = 0.06  # more than 2 transcriber frames (2 * 1024 / 44100 = 46 ms) between same pitch
LOW, HIGH = 36, 96  # main playing range; ``wide`` phrases use 28..100
WIDE_LOW, WIDE_HIGH = 28, 100
MIN_NOTE_S = 0.08


def draw_velocity(rng: random.Random) -> int:
    """Skewed to soft playing, 1..127."""
    return max(1, min(127, 1 + round(126 * rng.betavariate(1.6, 2.6))))


class _Track:
    """Collects notes while enforcing the per-pitch no-overlap rule."""

    def __init__(self, duration_s: float):
        self.duration_s = duration_s
        self.notes: list[dict[str, Any]] = []
        self._free_at: dict[int, float] = {}

    def add(self, midi: int, onset: float, length: float, velocity: int) -> bool:
        onset = max(0.0, onset)
        length = max(MIN_NOTE_S, length)
        end = min(onset + length, self.duration_s)
        if end - onset < MIN_NOTE_S or onset < self._free_at.get(midi, 0.0):
            return False
        self._free_at[midi] = end + MIN_GAP_S
        self.notes.append(
            {
                "midi": int(midi),
                "onset_s": round(onset, 4),
                "offset_s": round(end, 4),
                "velocity": int(velocity),
            }
        )
        return True


def _pitch(rng: random.Random, prev: int | None, lo: int, hi: int) -> int:
    if prev is None or rng.random() < 0.15:
        return rng.randint(lo, hi)  # leap
    return max(lo, min(hi, prev + rng.choice((-7, -5, -4, -3, -2, -1, 1, 2, 3, 4, 5, 7))))


def _chord(rng: random.Random, lo: int, hi: int) -> list[int]:
    root = rng.randint(lo, max(lo, hi - 14))
    shape = rng.choice(((0, 4, 7), (0, 3, 7), (0, 5, 7), (0, 4, 7, 11), (0, 3, 7, 10), (0, 7, 12)))
    return [p for p in (root + s for s in shape) if p <= hi]


def _melody(rng, trk, lo, hi, t0, t1):
    t, prev = t0, None
    while t < t1:
        prev = _pitch(rng, prev, lo, hi)
        length = rng.choice((0.15, 0.25, 0.4, 0.6, 1.0, 1.6)) * rng.uniform(0.7, 1.2)
        trk.add(prev, t, length, draw_velocity(rng))
        t += max(0.1, length * rng.uniform(0.6, 1.1))


def _chords(rng, trk, lo, hi, t0, t1):
    t = t0
    while t < t1:
        length = rng.choice((0.4, 0.8, 1.5, 2.5))
        vel = draw_velocity(rng)
        for p in _chord(rng, lo, hi):
            trk.add(p, t + rng.uniform(0, 0.03), length * rng.uniform(0.9, 1.0), vel)
        t += length * rng.uniform(0.9, 1.2) + rng.uniform(0, 0.3)


def _arpeggio(rng, trk, lo, hi, t0, t1):
    t = t0
    while t < t1:
        tones = _chord(rng, lo, hi)
        tones = tones + [p + 12 for p in tones if p + 12 <= hi]
        step = rng.choice((0.12, 0.18, 0.25, 0.35))
        order = tones if rng.random() < 0.5 else tones[::-1]
        for p in order * rng.randint(1, 2):
            trk.add(p, t, step * rng.uniform(1.0, 1.8), draw_velocity(rng))
            t += step
        t += rng.uniform(0.1, 0.5)


def _repeats(rng, trk, lo, hi, t0, t1):
    t = t0
    while t < t1:
        p, step = rng.randint(lo, hi), rng.choice((0.12, 0.2, 0.3, 0.45))
        base = draw_velocity(rng)
        for _ in range(rng.randint(3, 8)):
            vel = max(1, min(127, base + rng.randint(-8, 8)))
            trk.add(p, t, step * 0.7, vel)
            t += step
        t += rng.uniform(0.2, 0.8)


def make_phrase(
    seed: int, kind: str | None = None, duration_s: float = 60.0, wide: bool = False
) -> tuple[str, list[dict[str, Any]]]:
    """One deterministic phrase ``(kind, notes)`` sorted by onset then pitch."""
    rng = random.Random(seed)
    kind = kind or rng.choice(KINDS)
    if kind not in KINDS:
        raise ValueError(f"unknown phrase kind {kind!r}")
    lo, hi = (WIDE_LOW, WIDE_HIGH) if wide else (LOW, HIGH)
    trk = _Track(duration_s)
    gens = {"melody": _melody, "chords": _chords, "arpeggio": _arpeggio, "repeats": _repeats}
    t0, t1 = rng.uniform(0.3, 1.0), duration_s - 1.0
    if kind == "mixed":
        cuts = sorted(rng.uniform(t0, t1) for _ in range(3))
        edges = [t0, *cuts, t1]
        for a, b in zip(edges, edges[1:]):
            gens[rng.choice(tuple(gens))](rng, trk, lo, hi, a, b)
    else:
        gens[kind](rng, trk, lo, hi, t0, t1)
    return kind, sorted(trk.notes, key=lambda n: (n["onset_s"], n["midi"]))
