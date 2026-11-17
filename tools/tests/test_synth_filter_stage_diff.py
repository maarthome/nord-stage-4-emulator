"""Tests for tools/synth_filter_stage_diff.py (pure helpers; oracle runs need the local image)."""

import math

import numpy as np
import synth_filter_stage_diff as m


def test_float_words_round_trip() -> None:
    assert m.f2u(1.0) == 0x3F800000
    assert m.u2f(0xBF800000) == -1.0
    assert m.u2f(m.f2u(0.1)) == np.float32(0.1)


def test_stream_is_an_impulse_then_deterministic_noise() -> None:
    xs = m.stream()
    assert len(xs) == m.SAMPLES and xs[0] == 1.0
    assert all(x == 0.0 for x in xs[1 : m.IMPULSE_TAIL])
    noise = xs[m.IMPULSE_TAIL :]
    assert max(abs(x) for x in noise) <= m.NOISE_AMP and any(x != 0.0 for x in noise)
    assert xs == m.stream()


def test_resonance_word_and_settled_position() -> None:
    assert m.settled_position(0) == 0.0
    assert abs(m.settled_position(64) - 64.504) < 1e-3
    assert m.settled_position(127) == 127.999
    assert m.w28_word(64.5) == 0x40800000
    assert m.w28_word(-3.0) == 0 and m.w28_word(1e9) >> 24 == 127


def one_pole(a: float, n: int = 20000) -> np.ndarray:
    """Impulse response of `y = (1 - a) x + a y1`."""
    ir = np.zeros(n)
    y = 0.0
    for i in range(n):
        y = (1 - a) * (1.0 if i == 0 else 0.0) + a * y
        ir[i] = y
    return ir


def test_low_pass_corner_and_slope_of_a_one_pole() -> None:
    a = 0.9
    got = m.metrics(one_pole(a), "lp2")
    want = math.acos((4 * a - 1 - a * a) / (2 * a)) / (2 * math.pi)  # |H| = 1/sqrt2 of DC
    assert abs(got["corner"] - want) < 2e-4
    assert abs(got["peak_db"]) < 0.01
    assert -7.0 < got["slope_db_oct"] < -3.0


def test_high_pass_and_band_pass_corners_follow_the_one_pole() -> None:
    lp = one_pole(0.9)
    hp = -lp
    hp[0] += 1.0  # x - lowpass(x)
    c_lp = m.metrics(lp, "lp2")["corner"]
    assert abs(m.metrics(hp, "hp4")["corner"] - c_lp) < 5e-3
    band = lp - np.concatenate(([0.0], lp[:-1]))
    got = m.metrics(band, "bp4")
    assert 0.0 < got["corner"] < 0.5 and abs(got["peak_db"]) < 0.01


def test_diff_words_counts_equal_words_and_the_first_difference() -> None:
    a = [m.f2u(x) for x in (1.0, 0.5, -0.25, 0.0)]
    b = list(a)
    d = m.diff_words(a, b)
    assert (d["equal"], d["total"], d["first_diff"], d["max_abs"]) == (4, 4, -1, 0.0)
    b[2] = m.f2u(-0.25 + 2.0**-10)
    d = m.diff_words(a, b)
    assert (d["equal"], d["first_diff"]) == (3, 2)
    assert abs(d["max_abs"] - 2.0**-10) < 1e-12
    assert abs(d["max_rel_peak"] - 2.0**-10) < 1e-12


def test_write_csv_is_a_no_op_without_a_path(tmp_path) -> None:
    m.write_csv(None, ["a"], [[1]])
    path = tmp_path / "x.csv"
    m.write_csv(str(path), ["a", "b"], [[1, 2]])
    assert path.read_text(encoding="utf-8") == "a,b\n1,2\n"


def test_stage_table_is_the_identity_table() -> None:
    # docs/research/synth-filter-links.md s1: raw types 0, 1, 2, 4, 5 (3 is not modelled)
    assert m.STAGES == {
        "lp2": 0x1253BD,
        "lp4": 0x12566E,
        "ladder": 0x12549D,
        "bp4": 0x125913,
        "hp4": 0x125A50,
    }
    assert abs(m.UNIT_2F - 1.0147) < 1e-3


def test_ladder_position_inverts_row_two() -> None:
    import synth_filter_ladder as lad

    for p in (0.1, 0.36083, 0.5458, 0.8):
        i = int(p * 128)
        fr = p * 128 - i
        r2 = [m.u2f(w) for w in lad.ROWS[2]]
        c = r2[i] * (1 - fr) + r2[i + 1] * fr
        assert abs(m.table_position_of_c(c) - p) < 1e-4
    assert math.isnan(m.table_position_of_c(5.0))
