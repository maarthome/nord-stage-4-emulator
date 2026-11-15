"""Tests for `tools/fx_mod2_dense.py` (pure helpers; the capture needs the firmware)."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import fx_mod2_dense as d


def fw(x: float) -> int:
    return struct.unpack("<I", struct.pack("<f", x))[0]


def test_dense_grid_clips_and_lists_small_ranges_whole() -> None:
    assert d.dense_grid(0, 127)[0] == 0 and d.dense_grid(0, 127)[-1] == 127
    assert set(d.SAMPLES) <= set(d.dense_grid(0, 127))
    assert set(range(60, 81)) <= set(d.dense_grid(0, 127))
    assert d.dense_grid(0, 3) == [0, 1, 2, 3]
    assert max(d.dense_grid(0, 100)) <= 100


def test_interpolation_is_exact_for_a_linear_series() -> None:
    ser = {v: fw(2.0 * v + 1.0) for v in d.dense_grid(0, 127)}
    err, _at, is_float = d.interpolation_error(ser)
    assert is_float and err < 1e-6


def test_interpolation_error_finds_a_knee_between_the_knots() -> None:
    ser = {v: fw(max(0.0, v - 70.0)) for v in d.dense_grid(0, 127)}
    err, at, _ = d.interpolation_error(ser)
    assert err > 0.02 and 63 < at < 79


def test_integer_words_are_signed() -> None:
    assert d.as_number(0xFFFFFFFF, False) == -1.0
    assert not d.looks_float(0x00043200)
    ser = {v: 12800 + 4096 * v for v in d.dense_grid(0, 127)}
    err, _at, is_float = d.interpolation_error(ser)
    assert not is_float and err == 0.0


def test_series_of_keeps_the_last_word_of_each_value() -> None:
    rows = [
        {
            "stage": "param",
            "kind": "piano",
            "type": "4",
            "id": "Edt_Effects_kFx2Amt",
            "dm_word": "0x1",
            "value": "5",
            "word": "0x00000001",
        },
        {
            "stage": "param",
            "kind": "piano",
            "type": "4",
            "id": "Edt_Effects_kFx2Amt",
            "dm_word": "0x1",
            "value": "5",
            "word": "0x00000002",
        },
        {
            "stage": "type",
            "kind": "piano",
            "type": "4",
            "id": "x",
            "dm_word": "0x1",
            "value": "5",
            "word": "0x3",
        },
    ]
    assert d.series_of(rows) == {("piano", "4", "Amt", "0x1"): {5: 2}}


def test_holdout_error_is_small_for_a_smooth_law_and_large_for_a_knee() -> None:
    smooth = {v: fw(0.01 * v * v) for v in d.dense_grid(0, 127)}
    knee = {v: fw(max(0.0, v - 70.0)) for v in d.dense_grid(0, 127)}
    assert d.holdout_error(smooth) < 0.01
    assert d.holdout_error(knee) > d.holdout_error(smooth)
