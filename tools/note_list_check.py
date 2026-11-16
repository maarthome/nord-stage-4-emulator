#!/usr/bin/env python3
"""Plausibility of a note list against the hardware audio of its clip (chroma agreement).

``transcribe_agree.py`` scores a list against the video-confirmed notes, which cover only the
visible keys and recall little on synth clips. This tool needs no truth list: it compares the
pitch-class energy of the hardware audio (50 ms frames, STFT magnitude folded to 12 classes) with
the pitch classes of the notes the list holds sounding in each frame (cosine similarity), and
repeats the comparison with the list shifted in time. A list that follows the performance scores
high at shift 0 and falls to the null level (mean over shifts of 3 s or more); the lift
``cos(0) - null`` is the evidence. First and last 3 s are reported apart (clip edge effects).
EMPIRICAL heuristic: overtones and reverb tails of the audio blur the classes, so only the
comparison between lists of the same clip is meaningful, never the absolute value.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

import numpy as np
import video_ref

FRAME_S = 0.05
WIN = 4096
F_LO, F_HI = 55.0, 4200.0
EDGE_S = 3.0
SHIFTS_S = (-8.0, -6.0, -4.0, -3.0, 3.0, 4.0, 6.0, 8.0)
SILENCE_DB = -50.0


def hw_chroma(mono: np.ndarray, sr: int) -> np.ndarray:
    """``(frames, 12)`` sqrt-compressed pitch-class energy of the audio, hop ``FRAME_S``."""
    hop = int(round(FRAME_S * sr))
    n = max(0, (len(mono) - WIN) // hop + 1)
    freqs = np.fft.rfftfreq(WIN, 1.0 / sr)
    sel = (freqs >= F_LO) & (freqs <= F_HI)
    pc = (np.round(12 * np.log2(freqs[sel] / 440.0)).astype(int) + 9) % 12
    win = np.hanning(WIN)
    out = np.zeros((n, 12))
    for i in range(n):
        mag = np.abs(np.fft.rfft(mono[i * hop : i * hop + WIN] * win))[sel]
        np.add.at(out[i], pc, mag)
    return np.sqrt(out)


def list_chroma(events: Sequence[dict[str, Any]], frames: int, shift_s: float = 0.0) -> np.ndarray:
    """``(frames, 12)`` 0/1 pitch classes sounding per frame; the list rolled by ``shift_s``.

    The roll wraps around the clip, so a shifted list keeps its density at the clip edges (a
    plain shift would empty them and inflate the edge lift).
    """
    out = np.zeros((frames, 12))
    for e in events:
        a = int(round(e["onset_s"] / FRAME_S))
        b = int(round(e["offset_s"] / FRAME_S))
        a, b = max(a, 0), min(max(b, a + 1), frames)
        if a < frames and b > 0:
            out[a:b, int(e["midi"]) % 12] = 1.0
    return np.roll(out, int(round(shift_s / FRAME_S)), axis=0) if frames else out


def frame_cosine(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """Per-frame cosine similarity; frames where either side is all zero score 0."""
    num = (a * b).sum(axis=1)
    den = np.linalg.norm(a, axis=1) * np.linalg.norm(b, axis=1)
    return np.where(den > 0, num / np.maximum(den, 1e-12), 0.0)


def active_mask(chroma: np.ndarray) -> np.ndarray:
    """Frames whose energy is within ``-SILENCE_DB`` dB of the loudest frame."""
    e = chroma.sum(axis=1) ** 2
    return e > e.max() * 10 ** (SILENCE_DB / 10) if len(e) else e > 0


def score_list(chroma: np.ndarray, events: Sequence[dict[str, Any]]) -> dict[str, float]:
    """Mean cosine at shift 0 over active frames, the null over ``SHIFTS_S``, edges apart."""
    frames = len(chroma)
    act = active_mask(chroma)

    def mean_at(shift: float, mask: np.ndarray) -> float:
        c = frame_cosine(chroma, list_chroma(events, frames, shift))
        m = mask & act
        return float(c[m].mean()) if m.any() else 0.0

    allm = np.ones(frames, dtype=bool)
    t = np.arange(frames) * FRAME_S
    head, tail = t < EDGE_S, t > t[-1] - EDGE_S if frames else allm
    mid = ~(head | tail)
    cos0 = mean_at(0.0, allm)
    null = float(np.mean([mean_at(s, allm) for s in SHIFTS_S]))
    return {
        "cos0": round(cos0, 3),
        "null": round(null, 3),
        "lift": round(cos0 - null, 3),
        "lift_head": round(
            mean_at(0.0, head) - float(np.mean([mean_at(s, head) for s in SHIFTS_S])), 3
        ),
        "lift_mid": round(
            mean_at(0.0, mid) - float(np.mean([mean_at(s, mid) for s in SHIFTS_S])), 3
        ),
        "lift_tail": round(
            mean_at(0.0, tail) - float(np.mean([mean_at(s, tail) for s in SHIFTS_S])), 3
        ),
        "notes": len(events),
    }


def load_list(path: Path, confirmed_only: bool = False) -> list[dict[str, Any]]:
    ev = json.loads(path.read_text(encoding="utf-8"))["events"]
    return [e for e in ev if e.get("confirmed", True) or not confirmed_only]


def main(argv: Sequence[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("wav", help="hardware clip WAV (local, never committed)")
    p.add_argument("lists", nargs="+", help="NAME=events.json[@confirmed]")
    args = p.parse_args(argv)
    sr, left, right = video_ref.read_wav_stereo(Path(args.wav))
    chroma = hw_chroma(0.5 * (np.asarray(left) + np.asarray(right)), sr)
    res = {}
    for spec in args.lists:
        name, _, rest = spec.partition("=")
        path, _, flag = rest.partition("@")
        res[name] = score_list(chroma, load_list(Path(path), flag == "confirmed"))
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
