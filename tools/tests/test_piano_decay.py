"""Tests for tools/piano_decay.py on synthetic decaying partials."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import piano_decay as pdk  # noqa: E402

SR = 8000


def tone(midi: int, t60: float, dur: float, onset: float = 0.1) -> np.ndarray:
    """Exponentially decaying sine of the fundamental of ``midi`` (T60 in seconds)."""
    t = np.arange(int(dur * SR)) / SR
    f0 = 440.0 * 2.0 ** ((midi - 69) / 12.0)
    env = np.where(t >= onset, 10.0 ** (-3.0 * (t - onset) / t60), 0.0)
    return 0.5 * env * np.sin(2 * np.pi * f0 * t)


def test_octave_label() -> None:
    assert pdk.octave_label(60) == "60-71"
    assert pdk.octave_label(40) == "21-47"
    assert pdk.octave_label(10) == "other"


def test_slope_recovers_t60() -> None:
    notes = [{"midi": 60, "onset_s": 0.1, "offset_s": 1.0}]
    x = tone(60, 2.0, 4.0)
    slope = pdk.note_slope_db_s(x, SR, notes, 0)
    assert slope is not None
    assert abs(-60.0 / slope - 2.0) < 0.1


def test_fast_and_slow_decay_ordered() -> None:
    notes = [{"midi": 64, "onset_s": 0.1, "offset_s": 1.0}]
    fast = pdk.t60_by_octave(tone(64, 1.5, 4.0), SR, notes)["60-71"]["t60_s"]
    slow = pdk.t60_by_octave(tone(64, 4.0, 4.0), SR, notes)["60-71"]["t60_s"]
    assert fast < slow


def test_neighbour_note_disqualifies() -> None:
    notes = [
        {"midi": 60, "onset_s": 0.1, "offset_s": 1.0},
        {"midi": 61, "onset_s": 0.5, "offset_s": 1.0},
    ]
    assert pdk.note_slope_db_s(tone(60, 2.0, 4.0), SR, notes, 0) is None


def test_repeated_pitch_ends_fit_window() -> None:
    notes = [
        {"midi": 60, "onset_s": 0.1, "offset_s": 0.5},
        {"midi": 60, "onset_s": 0.6, "offset_s": 1.0},
    ]
    assert pdk.fit_end(notes, 0) < 0.6
    assert pdk.note_slope_db_s(tone(60, 2.0, 4.0), SR, notes, 0) is None


def test_compare_rows_and_json(tmp_path: Path) -> None:
    hw = {"60-71": {"slope_db_s": -20.0, "t60_s": 3.0, "n": 4}}
    emu = {
        "60-71": {"slope_db_s": -15.0, "t60_s": 4.0, "n": 5},
        "72-83": {"slope_db_s": -1.0, "t60_s": 9.0, "n": 1},
    }
    rows = pdk.compare(hw, emu)
    assert rows == [
        {
            "octave": "60-71",
            "n_hw": 4,
            "n_emu": 5,
            "t60_hw_s": 3.0,
            "t60_emu_s": 4.0,
            "slope_delta_db_s": 5.0,
        }
    ]
    json.dumps(rows)


def test_window_longer_for_low_notes() -> None:
    assert pdk.window_s(40) > pdk.window_s(60)


def test_flat_signal_caps_t60() -> None:
    notes = [{"midi": 60, "onset_s": 0.1, "offset_s": 1.0}]
    out = pdk.t60_by_octave(tone(60, 1e6, 4.0), SR, notes)
    assert out["60-71"]["t60_s"] == pdk.MAX_T60_S
