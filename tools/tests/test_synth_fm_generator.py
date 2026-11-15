"""Tests for the ported FM table generator against the committed ARM captures (no firmware)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_fm_generator as g  # noqa: E402

DATA = Path(__file__).resolve().parents[2] / "docs" / "research" / "data"


def test_default_tables_equal_the_committed_capture() -> None:
    ref: dict[int, list[int]] = {}
    for line in (DATA / "synth-fm-tables.csv").read_text().splitlines()[1:]:
        t, dm, word, _ = line.split(",")
        ref.setdefault(int(t), [0] * g.TABLE_WORDS)[int(dm, 16) - g.TABLE_BASE] = int(word, 16)
    for ty, want in ref.items():
        assert g.table(ty) == want


def test_variant_cases_equal_the_arm_run() -> None:
    lines = (DATA / "synth-fm-generator-cases.csv").read_text().splitlines()[1:]
    assert len(lines) == 15
    for line in lines:
        ty, a4, a8, words = line.split(",", 3)
        want = [int(w, 16) for w in words.split(" ")]
        assert g.table(int(ty), int(a4), int(a8, 16)) == want


def test_scale_laws() -> None:
    assert float(g.scale(0, 0x80000)) == 8.0
    assert abs(float(g.scale(0, 0x40000)) - 4.0) < 1e-6
    assert abs(float(g.scale(1, 0x80000)) - 2 ** (-4 / 12)) < 1e-6


def test_fade_clips_and_blends() -> None:
    assert float(g.fade(g.F32(1.0))) == 1.0
    assert float(g.fade(g.F32(-1.0))) == 0.0
    assert 0.0 < float(g.fade(g.F32(0.0))) < 1.0
