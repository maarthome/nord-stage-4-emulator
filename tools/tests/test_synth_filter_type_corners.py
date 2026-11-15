"""Tests for tools/synth_filter_type_corners.py: oracle-match of the closed forms and the table."""

import cmath
import math
import struct

import sharc40 as s
import synth_filter_lp12 as lp12
import synth_filter_model as m
import synth_filter_type_corners as c

TABLES = c.load_tables()
N = 700
AMP = 1.0e-3
FREQS = (0.01, 0.03, 0.06, 0.12, 0.25)


def _f2u(x: float) -> int:
    return struct.unpack("<I", struct.pack("<f", x))[0]


def _impulse() -> list[int]:
    return [_f2u(AMP)] + [0] * (N - 1)


def _bit_exact(kind: str, r: float, idx: int) -> list[float]:
    w28 = idx << 24
    rw = _f2u(r)
    a = [_f2u(v) for v in TABLES["A"]]
    cc = [_f2u(v) for v in TABLES["C"]]
    b = [_f2u(v) for v in TABLES["B"]]
    d = [_f2u(v) for v in TABLES["D"]]
    xs = _impulse()
    if kind == "lp24":
        out = m.process(m.Lane(), m.coefficients(w28, rw, a, b), xs)
    elif kind == "bp4":
        out = m.process_bp(m.Lane(), m.coefficients(w28, rw, cc, b), xs)
    elif kind == "hp4":
        out = m.process_hp(m.Lane(), m.coefficients(w28, rw, a, b), xs)
    else:
        out = lp12.process(m.Lane(), m.coefficients(w28, rw, a, d), xs)
    floats = [float(s.ld(w)) for w in out]
    return [
        abs(sum(v * cmath.exp(-2j * math.pi * f * n) for n, v in enumerate(floats))) / AMP
        for f in FREQS
    ]


def test_closed_forms_match_the_bit_exact_models() -> None:
    # SHARC-verified Python models (equal to the oracle word for word) against the closed forms
    for kind in c.TYPES:
        want = _bit_exact(kind, 0.2, 64)
        for f, w in zip(FREQS, want):
            got = c.gain_at(TABLES, kind, 0.2, 64, f)
            assert abs(got - w) <= 3e-3 * max(abs(w), 0.05), (kind, f, got, w)


def test_nominal_cutoff_is_the_bilinear_one_for_every_type() -> None:
    row = c.corner_row(TABLES, "lp24", 0.1, 64)
    assert abs(float(row["nominal"]) - math.atan(0.1) / math.pi) < 1e-6
    # band pass peaks at the nominal cutoff, whatever the damping
    for idx in (16, 64, 127):
        assert abs(float(c.corner_row(TABLES, "bp4", 0.1, idx)["peak_over_nominal"]) - 1.0) < 0.01


def test_lp12_corner_is_above_lp24_at_equal_position() -> None:
    for idx in (47, 64, 81):
        a = float(c.corner_row(TABLES, "lp12", 0.1, idx)["f3_over_nominal"])
        b = float(c.corner_row(TABLES, "lp24", 0.1, idx)["f3_over_nominal"])
        assert 1.05 < a / b < 1.2


def test_committed_table_is_current() -> None:
    have = c.OUT_CSV.read_text(encoding="utf-8").replace("\r\n", "\n")
    assert have == c.table_text()
    assert c.main(["--check"]) == 0
