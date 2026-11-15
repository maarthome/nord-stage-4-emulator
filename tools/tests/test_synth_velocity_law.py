"""Unit tests for tools/synth_velocity_law.py (model vs the committed oracle CSV, no firmware)."""

from __future__ import annotations

import synth_velocity_law as law


def test_model_matches_the_committed_oracle_series_within_one_ulp():
    rows = law.read_csv(law.CSV_PATH)
    assert len(rows) == 768
    assert law.compare(rows, law.model_rows()) <= 1


def test_amp_law_endpoints_and_flat_setting_zero():
    assert all(law.amp_word(0, g) == law.f2u(1.0) for g in range(128))
    for s in (1, 2, 3):
        lo, hi = law.u2f(law.amp_word(s, 0)), law.u2f(law.amp_word(s, 127))
        assert abs(lo / 10 ** (-0.3 * s) - 1) < 1e-3
        assert abs(hi / 10 ** (0.1 * s) - 1) < 1e-3


def test_amp_law_is_monotonic_and_g127_acts_as_128():
    for s in (1, 2, 3):
        words = [law.amp_word(s, g) for g in range(128)]
        assert words == sorted(words)
    assert law.effective_g(127) == 128 and law.effective_g(126) == 126


def test_filter_factor_is_g_over_128_when_on_and_one_when_off():
    assert law.u2f(law.filter_word(True, 64)) == 0.5
    assert law.u2f(law.filter_word(False, 3)) == 1.0


def test_csv_round_trip(tmp_path):
    path = tmp_path / "x.csv"
    rows = law.model_rows()
    law.write_csv(path, rows)
    assert law.read_csv(path) == rows
