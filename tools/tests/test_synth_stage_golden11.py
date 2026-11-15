"""Tests of the verified-words unison golden generator (no firmware needed)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_stage_golden11 as g  # noqa: E402
import synth_wave_model as m  # noqa: E402


def test_words_come_from_the_committed_capture() -> None:
    w = g.load_words()
    assert len(w) == 74
    t = g.type_of(6, 0)
    assert t == 22
    assert abs(float(m.ld(w[t][16])) - 0.0883883461) < 1e-9
    assert abs(float(m.ld(w[t][-32])) - 0.997116029) < 1e-9


def test_rows_cover_every_case_with_finite_outputs() -> None:
    rows = g.rows()
    assert len(rows) == len(g.CASES) * len(g.PITCHES) * len(g.SPREADS)
    for _, _, _, out in rows:
        assert len(out) == 4
        assert all(abs(float(m.ld(w & 0xFFFFFFFF))) < 4.0 for w in out)
    assert rows == g.rows(), "deterministic"


def test_rust_text_declares_the_tables() -> None:
    text = g.rust()
    for name in ("CASE", "DT", "S5", "OUT"):
        assert f"pub(super) const {name}:" in text
    assert "golden_tables_have_the_documented_shape" in text
