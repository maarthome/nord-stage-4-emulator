"""Tests for `tools/fx_mod2_dense_gen.py` and `tools/fx_mod2_joint.py` (derived data only)."""

from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import fx_mod2_dense_gen as g
import fx_mod2_joint as j


def test_generated_rust_files_are_up_to_date() -> None:
    assert g.main(["laws", "--check"]) == 0
    assert g.main(["tests", "--check"]) == 0


def test_every_wired_law_has_the_knot_count_of_the_grid() -> None:
    ks = g.knots(g.read_dense())
    assert ks[0] == 0 and ks[-1] == 127 and len(ks) == 44
    for kinds in g.dense_laws().values():
        for laws in kinds.values():
            for _flt, words in laws.values():
                assert len(words) == len(ks)


def test_best_match_needs_most_samples_to_agree() -> None:
    full = {0x10: dict(zip(g.ml.SAMPLES, range(11), strict=True))}
    full[0x10].update({v: 0 for v in (3, 8)})
    words = list(range(11))
    assert g.best_match(full, words) == (0x10, full[0x10])
    assert g.best_match(full, [w + 1 for w in words]) is None


def test_joint_words_are_bilinear_in_the_rate_for_phas_0() -> None:
    joint = g.read_joint()
    grid = joint[(0, 0xB032C)]
    import struct

    f = lambda w: struct.unpack("<f", struct.pack("<I", w))[0]  # noqa: E731
    for a, col in grid.items():
        lin = f(col[0]) + (f(col[127]) - f(col[0])) * 64 / 127
        assert abs(lin - f(col[64])) <= 1e-5 * max(1.0, abs(f(col[64]))), a


def test_last_words_keeps_the_final_write_per_address() -> None:
    msgs = [SimpleNamespace(dm_word=1, word2=5), SimpleNamespace(dm_word=1, word2=6)]
    msgs.append(SimpleNamespace(dm_word=2, word2=7))
    assert j.last_words(msgs, {1}) == {1: 6}
