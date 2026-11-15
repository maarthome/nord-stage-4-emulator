import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import emu_env  # noqa: E402

SR = 8000


def _tone(seconds: float, decay_db_s: float) -> np.ndarray:
    t = np.arange(int(seconds * SR)) / SR
    return (10 ** (decay_db_s * t / 20)) * np.sin(2 * np.pi * 220 * t) * 0.5


def test_window_db_outside_signal_is_none() -> None:
    x = np.ones(SR)
    assert emu_env.window_db(x, SR, 5.0, (0.0, 0.1)) is None
    assert abs(emu_env.window_db(x, SR, 0.0, (0.1, 0.2)) - 0.0) < 1e-6


def test_hold_decay_follows_the_decay_rate() -> None:
    x = _tone(2.0, -20.0)  # -20 dB/s amplitude
    d = emu_env.hold_decay_db(x, SR, [(0.0, 1.5)])
    # centre of the late window is 0.4 s after the centre of the early one
    assert d is not None and abs(d - (-20.0 * 0.4)) < 0.6


def test_hold_decay_skips_short_notes() -> None:
    assert emu_env.hold_decay_db(_tone(2.0, -10.0), SR, [(0.0, 0.3)]) is None


def test_release_drop_sees_the_cut() -> None:
    x = _tone(2.0, 0.0)
    x[int(1.0 * SR) :] = 0.0
    d = emu_env.release_drop_db(x, SR, [(0.0, 1.0)])
    assert d is not None and d < -60.0


def test_release_drop_skips_overlapping_onsets() -> None:
    x = _tone(2.0, 0.0)
    assert emu_env.release_drop_db(x, SR, [(0.0, 1.0), (1.1, 1.15)]) is None
