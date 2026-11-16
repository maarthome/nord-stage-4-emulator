"""Tests for tools/note_list_check.py."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import note_list_check as nlc  # noqa: E402

SR = 16000


def tone(seq):
    """seq: list of (midi, seconds); returns a mono signal of sine notes."""
    out = []
    for midi, dur in seq:
        t = np.arange(int(dur * SR)) / SR
        out.append(0.5 * np.sin(2 * np.pi * 440.0 * 2 ** ((midi - 69) / 12) * t))
    return np.concatenate(out)


def events(seq):
    t, ev = 0.0, []
    for midi, dur in seq:
        ev.append({"midi": midi, "onset_s": t, "offset_s": t + dur})
        t += dur
    return ev


SEQ = [(60, 1.0), (64, 1.0), (67, 1.0), (62, 1.0), (65, 1.0), (69, 1.0)] * 3


def test_list_chroma_marks_pitch_classes_in_time():
    c = nlc.list_chroma([{"midi": 61, "onset_s": 0.5, "offset_s": 1.0}], 40)
    assert c[10:20, 1].all() and c.sum() == 10
    assert nlc.list_chroma([{"midi": 61, "onset_s": 0.5, "offset_s": 1.0}], 40, 0.5)[20:30, 1].all()
    wrapped = nlc.list_chroma([{"midi": 61, "onset_s": 1.5, "offset_s": 2.0}], 40, 0.5)
    assert wrapped[0:10, 1].all() and wrapped.sum() == 10  # rolled past the end, density kept


def test_true_list_beats_a_wrong_one_and_the_null():
    chroma = nlc.hw_chroma(tone(SEQ), SR)
    good = nlc.score_list(chroma, events(SEQ))
    wrong = nlc.score_list(chroma, events([(m + 1, d) for m, d in SEQ]))
    assert good["cos0"] > 0.8 and good["lift"] > 0.3
    assert wrong["cos0"] < 0.2
    assert good["lift_mid"] > 0.3


def test_frame_cosine_and_silence_mask():
    a = np.array([[1.0, 0, 0], [0, 0, 0]])
    assert list(nlc.frame_cosine(a, a)) == [1.0, 0.0]
    assert list(nlc.active_mask(np.array([[1.0, 0], [0.0, 0.0]]))) == [True, False]
