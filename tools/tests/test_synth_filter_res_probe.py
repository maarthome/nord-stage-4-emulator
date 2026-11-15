"""Tests for tools/synth_filter_res_probe.py (pure helpers; the oracle run needs the image)."""

import struct

import synth_filter_res_probe as p


def test_res_word_is_the_arm_float() -> None:
    assert struct.unpack("<f", struct.pack("<I", p.res_word(0)))[0] == -0.5
    assert abs(struct.unpack("<f", struct.pack("<I", p.res_word(127)))[0] - 0.5) < 1e-6


def test_position_splits_index_and_fraction() -> None:
    assert p.position(0x40000000) == 64.0
    assert abs(p.position(0x40218223) - 64.131) < 1e-3


def test_model_matches_the_recorded_oracle_values() -> None:
    # recorded oracle run (60 passes, note 60): W28 words of knob 0, 64, 127
    for knob, word in ((0, 0x2F6070C0), (64, 0x40218223), (127, 0x509F8F40)):
        assert abs(p.model_position(knob) - p.position(word)) < 0.01
