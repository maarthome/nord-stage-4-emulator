"""Tests for tools/clip_flags.py with synthetic audio (no torch)."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import clip_flags as cf  # noqa: E402

SR = 44100


def tone(midi: int, dur: float = 1.0) -> np.ndarray:
    t = np.arange(int(dur * SR)) / SR
    f0 = 440.0 * 2 ** ((midi - 69) / 12)
    return 0.3 * (np.sin(2 * np.pi * f0 * t) + 0.4 * np.sin(2 * np.pi * 2 * f0 * t))


def test_program_class():
    assert cf.program_class("Velvet Grand") == "piano"
    assert cf.program_class("Felt Upright") == "piano"
    assert cf.program_class("B3 Chords n Solo") == "organ"
    assert cf.program_class("Classic Wurly") == "ep"
    assert cf.program_class("Aeon Pad") == "other"


def test_snr_true_note_vs_wrong_note():
    x = tone(60) + 0.01 * np.random.default_rng(0).standard_normal(SR)
    good = cf.note_f0_snr_db(x, SR, 60, 0.2)
    bad = cf.note_f0_snr_db(x, SR, 66, 0.2)
    assert good is not None and bad is not None
    assert good > cf.SNR_OK_DB > bad
    assert cf.note_f0_snr_db(x, SR, 60, 0.99) is None  # too close to the clip end


def test_piano_like_rules():
    assert cf.piano_like("piano", False, None) and cf.piano_like("piano", False, 0.5)
    assert not cf.piano_like("piano", False, 0.2) and not cf.piano_like("piano", True, None)
    assert not cf.piano_like("organ", False, 1.0)
    assert cf.piano_like("unknown", False, 0.9) and not cf.piano_like("unknown", False, None)


def test_program_at_and_windows():
    progs = [
        {"first_seen_s": 0, "program": "A:61", "name": "Velvet Grand"},
        {"first_seen_s": 2, "program": "A:14", "name": "B3 Chords n Solo"},
    ]
    assert cf.program_at(progs, 1.5)["program"] == "A:61"
    assert cf.program_at(progs, 5)["program"] == "A:14"
    assert cf.program_at(progs, -1) is None
    x = np.concatenate([tone(60, 1.0), tone(64, 1.0), tone(60, 1.0), np.zeros(SR)])
    notes = [
        {"midi": 60, "onset_s": 0.0},
        {"midi": 71, "onset_s": 1.0},  # wrong pitch: junk transcription
        {"midi": 60, "onset_s": 2.0},
    ]
    rows = cf.window_rows(x, SR, notes, progs)
    assert [r["piano_like"] for r in rows] == [True, False, False, False]
    assert rows[1]["supported"] == 0.0 and rows[0]["supported"] == 1.0
    assert rows[2]["class"] == "organ" and rows[3]["rms_dbfs"] <= -100
    ranges = cf.program_ranges(rows)
    assert [(r["program"], r["start_s"], r["end_s"]) for r in ranges] == [
        ("A:61", 0.0, 2.0),
        ("A:14", 2.0, 4.0),
    ]
    assert ranges[0]["piano_like_s"] == 1.0 and ranges[0]["notes"] == 2
