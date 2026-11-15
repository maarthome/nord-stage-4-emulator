"""Tests for tools/note_env.py on synthetic exponential-decay tones."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import note_env as ee  # noqa: E402

SR = 8000


def note_audio(onsets: list[tuple[float, float, float]], dur: float) -> np.ndarray:
    """Tone with 20 ms linear attack, 30 dB/s decay while held, 60 dB/s after the release."""
    t = np.arange(int(dur * SR)) / SR
    x = np.zeros_like(t)
    for on, off, amp in onsets:
        rel = np.clip((t - on) / 0.02, 0, 1)
        held = np.minimum(t, off) - on
        db = -30.0 * np.clip(held, 0, None) - 60.0 * np.clip(t - off, 0, None)
        env = np.where(t >= on, rel * amp * 10 ** (db / 20), 0.0)
        x += env * np.sin(2 * np.pi * 440 * t)
    return x


def test_isolated_filters_close_onsets():
    notes = [{"onset_s": 0.0}, {"onset_s": 0.1}, {"onset_s": 2.0}]
    assert [n["onset_s"] for n in ee.isolated(notes)] == [0.1, 2.0]


def test_envelope_metrics_recover_known_times():
    notes = [{"onset_s": 0.5, "offset_s": 1.5}]
    m = ee.envelope_metrics(note_audio([(0.5, 1.5, 0.5)], 4.0), SR, notes)
    assert 8 <= m["attack_ms"] <= 25
    assert 120 <= m["hold_ms"] <= 280  # -6 dB at 30 dB/s is 0.2 s after the peak
    assert 250 <= m["release_ms"] <= 420  # -20 dB at 60 dB/s is 0.33 s
    assert m["n_attack"] == 1 and m["n_release"] == 1


def test_silent_audio_gives_none():
    m = ee.envelope_metrics(np.zeros(SR * 3), SR, [{"onset_s": 0.5, "offset_s": 1.0}])
    assert m["attack_ms"] is None and m["release_ms"] is None


def test_velocity_slope_positive_for_louder_notes():
    spec = [(0.5 + i, 0.9 + i, 0.02 * (1 + i)) for i in range(6)]
    notes = [
        {"onset_s": a, "offset_s": b, "velocity": 30 + 15 * i} for i, (a, b, _) in enumerate(spec)
    ]
    slope, n = ee.velocity_slope_db(note_audio(spec, 8.0), SR, notes)
    assert n == 6 and slope is not None and slope > 0
    assert ee.velocity_slope_db(np.zeros(SR), SR, notes[:2]) == (None, 0)
