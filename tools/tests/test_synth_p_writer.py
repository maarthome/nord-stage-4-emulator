"""Tests for the P block writer capture helpers and the committed capture (no firmware needed)."""

from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_p_writer as pw  # noqa: E402

DATA = Path(__file__).resolve().parents[2] / "docs" / "research" / "data" / "synth-p-words.csv"


def msg(chip: str, core: int, dm: int, word: int) -> SimpleNamespace:
    return SimpleNamespace(chip=chip, core=core, dm_word=dm, word2=word)


def test_p_index_covers_the_slot_block_and_the_multi_window() -> None:
    assert pw.p_index(0xC169A) == 0
    assert pw.p_index(0xC167A) == -32
    assert pw.p_index(0xC16AA) == 16
    assert pw.p_index(0xC1710) is None, "slot 1 starts at P[-32 + 150]"
    assert pw.p_index(0xC1596) == 0xC1596 - 0xC169A
    assert pw.p_index(0xC1000) is None


def test_slot_one_block_is_150_words_after_slot_zero() -> None:
    assert pw.p_index(0xC167A + pw.P_SLOT_WORDS, slot=1) == -32
    assert pw.p_index(0xC167A + pw.P_SLOT_WORDS) is None


def test_fold_keeps_the_last_org_core2_word_only() -> None:
    got = pw.fold_core2(
        [
            msg("ORG", 2, 0xC16AA, 1),
            msg("ORG", 2, 0xC16AA, 2),
            msg("ORG", 1, 0xC16AB, 3),
            msg("PNO", 2, 0xC16AC, 4),
        ]
    )
    assert got == {0xC16AA: 2}


def test_f32_and_rows_round_trip() -> None:
    assert pw.f32(0x3E800000) == 0.25
    rows = pw.rows_for(2, 0, 0, {16: 0x3E800000, -32: 0x3F800000})
    assert [r["p_index"] for r in rows] == ["-32", "16"]
    assert rows[1]["dm"] == "0xc16aa" and rows[1]["value"] == "0.25"


def test_delta_keeps_changed_and_new_words() -> None:
    assert pw.delta({1: 5, 2: 6, 3: 7}, {1: 5, 2: 9}) == {2: 6, 3: 7}


def test_committed_capture_fingerprint() -> None:
    table = pw.load_csv(DATA)
    base = {t: w for (t, a, b), w in table.items() if (a, b) == (0, 0)}
    assert sorted(base) == list(range(pw.TYPES))
    saw = base[2]
    # VERIFIED (run): saw = one oscillator, ratio 1, gain P[16] 0.25, own weight P[-16] 1.0
    assert (pw.f32(saw[-32]), pw.f32(saw[-30]), pw.f32(saw[16]), pw.f32(saw[-16])) == (
        1.0,
        1.0,
        0.25,
        1.0,
    )
    assert pw.f32(saw[-12]) == 0.0
    # pulse types 3, 4, 5 differ in the gain only
    assert [round(pw.f32(base[t][16]), 4) for t in (3, 4, 5)] == [0.175, 0.2, 0.25]
    # MULTI wave 0: second oscillator ratio 1.0087 weight 0.85, gain 0.25 / sqrt 2
    multi = base[16]
    assert round(pw.f32(multi[-30]), 4) == 1.0087
    assert round(pw.f32(multi[-12]), 2) == 0.85
    assert round(pw.f32(multi[16]), 5) == 0.17678
    # SUPER 0 and 1..4 share the sixteen unison ratios, equal weights 1 / sqrt 2
    ratios = [pw.f32(base[22][i]) for i in range(-32, -16)]
    assert ratios[:4] == [pw.f32(base[24][i]) for i in range(-32, -28)]
    assert round(ratios[0], 5) == 0.99712 and round(ratios[3], 5) == 0.99137
    assert round(pw.f32(base[22][-16]), 5) == 0.70711
    assert round(pw.f32(base[22][16]), 5) == 0.08839
    assert round(pw.f32(base[24][16]), 3) == 0.125
