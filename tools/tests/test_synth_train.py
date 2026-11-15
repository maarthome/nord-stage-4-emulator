"""Tests for tools/synth_train.py (helpers; the training loop is exercised by a real run)."""

from __future__ import annotations

import collections
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import synth_train as st  # noqa: E402


def test_clip_value_warms_up_then_uses_quantile():
    h: collections.deque = collections.deque(maxlen=st.GRAD_WINDOW)
    assert st.clip_value(h) == float("inf")
    h.extend(range(1, 11))  # 1..10
    assert st.clip_value(h) == pytest.approx(8.2)  # 0.8 quantile


def test_to_notes_builds_transkun_notes():
    pytest.importorskip("transkun")
    seg = [
        [
            {
                "start": 0.5,
                "end": 1.0,
                "pitch": 60,
                "velocity": 70,
                "has_onset": True,
                "has_offset": False,
            }
        ]
    ]
    (n,) = st.to_notes(seg)[0]
    assert (n.start, n.end, n.pitch, n.velocity) == (0.5, 1.0, 60, 70)
    assert n.hasOnset is True and n.hasOffset is False


def test_parser_defaults_follow_the_plan():
    a = st.build_parser().parse_args(["data", "--out", "run"])
    assert a.arm == "lokr" and a.bs == 4 and a.seg == 16.0 and a.lr == 2e-4 and not a.bf16
    b = st.build_parser().parse_args(["data", "--out", "run", "--arm", "lora", "--r", "32"])
    assert b.arm == "lora" and b.r == 32
