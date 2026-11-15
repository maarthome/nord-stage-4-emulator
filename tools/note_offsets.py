#!/usr/bin/env python3
"""Sustained-note offsets for the Transkun stream, from the hardware audio itself.

Transkun is a piano transcriber: its offsets are string decays (median note 44 ms on the B3 clip),
while an organ key sounds until it is released. This tool replaces each offset of a stream
(``tools/transcribe_clip.py``) inside one clip window by an audio estimate, so the Leslie /
presence comparison (``docs/research/organ-preamp-search.md`` s4.2) no longer depends on the
held-length sweep.

Method (EMPIRICAL heuristic, validated on synthetic tones in ``tools/tests/test_note_offsets.py``):
Hann STFT; the level of a note is the summed power of the bins around its partials
``1..PARTIALS``; the reference is the maximum over the first ``ATTACK_S`` after the onset; the
estimated offset is the first frame after that where the level stays more than ``DROP_DB`` under
the reference for ``HOLD_FRAMES`` frames, capped at the next onset of the same key and at
``MAX_HOLD_S``. A partial shared with a still-sounding neighbour (octave, fifth) keeps the level
up, so the estimate is an **upper bound**; the transcribed offset is the lower bound.

    python tools/note_offsets.py STREAM.json HW.wav --at 238 --duration 25 --out OUT.json
        [--merge-gap 0.3]
"""

from __future__ import annotations

import argparse
import json
import wave
from pathlib import Path
from typing import Any

import numpy as np

NFFT = 8192
HOP_S = 0.010
PARTIALS = 3
ATTACK_S = 0.15
DROP_DB = 12.0
HOLD_FRAMES = 3
MAX_HOLD_S = 8.0


def midi_to_hz(midi: float) -> float:
    return 440.0 * 2.0 ** ((midi - 69.0) / 12.0)


def read_wav_mono(path: Path) -> tuple[int, np.ndarray]:
    """16-bit PCM WAV as ``(sample_rate, mono float32)``."""
    with wave.open(str(path), "rb") as w:
        sr, ch = w.getframerate(), w.getnchannels()
        raw = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2")
    return sr, raw.reshape(-1, ch).mean(axis=1).astype(np.float32) / 32768.0


def stft_power(mono: np.ndarray, sr: int) -> tuple[np.ndarray, float]:
    """Power spectrogram ``[frame, bin]`` (Hann, ``NFFT``) and the frame hop in seconds."""
    hop = max(1, int(round(HOP_S * sr)))
    win = np.hanning(NFFT).astype(np.float32)
    pad = np.concatenate([np.zeros(NFFT // 2, np.float32), mono, np.zeros(NFFT, np.float32)])
    n = max(1, (len(mono) + hop - 1) // hop)
    idx = np.arange(NFFT)[None, :] + hop * np.arange(n)[:, None]
    spec = np.fft.rfft(pad[idx] * win, axis=1)
    return (spec.real**2 + spec.imag**2).astype(np.float32), hop / sr


def partial_bins(midi: int, sr: int, partials: int = PARTIALS) -> np.ndarray:
    """FFT bins within one bin of each of the first ``partials`` partials (below Nyquist)."""
    out: list[int] = []
    for h in range(1, partials + 1):
        b = int(round(midi_to_hz(midi) * h * NFFT / sr))
        if b + 1 < NFFT // 2:
            out += [b - 1, b, b + 1]
    return np.array(sorted(set(out)), dtype=int)


def estimate_offset(
    power: np.ndarray,
    hop_s: float,
    midi: int,
    sr: int,
    onset_s: float,
    cap_s: float,
) -> float | None:
    """Audio offset of one note (seconds, clip time) or ``None`` when it has no energy."""
    bins = partial_bins(midi, sr)
    if bins.size == 0:
        return None
    level = 10.0 * np.log10(power[:, bins].sum(axis=1) + 1e-20)
    a = max(0, int(round(onset_s / hop_s)))
    b = min(len(level), int(round(cap_s / hop_s)))
    att = max(1, int(round(ATTACK_S / hop_s)))
    if b - a <= att:
        return None
    ref = float(level[a : a + att].max())
    below = level[a + att : b] < ref - DROP_DB
    for k in range(len(below) - HOLD_FRAMES + 1):
        if below[k : k + HOLD_FRAMES].all():
            return (a + att + k) * hop_s
    return cap_s


def merge_retriggers(events: list[dict[str, Any]], gap_s: float) -> list[dict[str, Any]]:
    """Joins same-key notes whose gap to the previous offset is at most ``gap_s``.

    A Leslie (or any amplitude modulation) makes a piano transcriber re-trigger one held organ
    key many times; the merged note spans the first onset to the last offset and keeps the first
    velocity. EMPIRICAL: on the B3 clip a 0.3 s gap turns 525 notes into 96.
    """
    out: list[dict[str, Any]] = []
    last: dict[int, dict[str, Any]] = {}
    for e in sorted(events, key=lambda x: x["onset_s"]):
        prev = last.get(e["midi"])
        if prev is not None and e["onset_s"] - prev["offset_s"] <= gap_s:
            prev["offset_s"] = max(prev["offset_s"], e["offset_s"])
        else:
            last[e["midi"]] = dict(e)
            out.append(last[e["midi"]])
    return out


def audio_offsets(
    events: list[dict[str, Any]], mono: np.ndarray, sr: int, at_s: float, duration_s: float
) -> list[dict[str, Any]]:
    """Copy of ``events`` (stream time) with audio offsets for the notes inside the clip window.

    The new offset is never earlier than the transcribed one and never past the next onset of
    the same key, the clip end or ``MAX_HOLD_S``; notes without energy keep their offset.
    """
    power, hop_s = stft_power(mono, sr)
    end = at_s + duration_s
    onsets: dict[int, list[float]] = {}
    for e in events:
        onsets.setdefault(e["midi"], []).append(e["onset_s"])
    out: list[dict[str, Any]] = []
    for e in events:
        ev = dict(e)
        if at_s <= e["onset_s"] < end:
            later = [t for t in onsets[e["midi"]] if t > e["onset_s"]]
            cap = min([end, e["onset_s"] + MAX_HOLD_S, *later]) - at_s
            est = estimate_offset(power, hop_s, e["midi"], sr, e["onset_s"] - at_s, cap)
            if est is not None:
                ev["offset_s"] = round(max(e["offset_s"], at_s + est), 4)
        out.append(ev)
    return out


def hold_stats(events: list[dict[str, Any]], at_s: float, duration_s: float) -> dict[str, float]:
    """Median and mean hold (seconds) of the notes inside the window."""
    h = [e["offset_s"] - e["onset_s"] for e in events if at_s <= e["onset_s"] < at_s + duration_s]
    if not h:
        return {"notes": 0, "median_s": 0.0, "mean_s": 0.0}
    return {
        "notes": len(h),
        "median_s": round(float(np.median(h)), 3),
        "mean_s": round(float(np.mean(h)), 3),
    }


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("stream", type=Path)
    ap.add_argument("wav", type=Path)
    ap.add_argument("--at", type=float, required=True)
    ap.add_argument("--duration", type=float, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--merge-gap", type=float, default=0.0, help="join same-key re-triggers")
    ns = ap.parse_args(argv)
    doc = json.loads(ns.stream.read_text(encoding="utf-8"))
    events = doc["events"]
    if ns.merge_gap > 0:
        events = merge_retriggers(events, ns.merge_gap)
    sr, mono = read_wav_mono(ns.wav)
    new = audio_offsets(events, mono, sr, ns.at, ns.duration)
    print("transcribed", hold_stats(events, ns.at, ns.duration))
    print("audio", hold_stats(new, ns.at, ns.duration))
    ns.out.write_text(json.dumps({**doc, "events": new}), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
