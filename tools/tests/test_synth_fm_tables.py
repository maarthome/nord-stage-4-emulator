"""Tests for the FM band table capture helpers and the committed capture (no firmware needed)."""

from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_fm_tables as ft  # noqa: E402

DATA = Path(__file__).resolve().parents[2] / "docs" / "research" / "data" / "synth-fm-tables.csv"


def msg(chip: str, core: int, dm: int, word: int) -> SimpleNamespace:
    return SimpleNamespace(chip=chip, core=core, dm_word=dm, word2=word)


def test_window_keeps_core2_words_inside_the_table_only() -> None:
    got = ft.window(
        [
            msg("ORG", 2, 0xE0900, 5),
            msg("ORG", 2, 0xE0900, 7),
            msg("ORG", 1, 0xE0901, 1),
            msg("PNO", 2, 0xE0902, 1),
            msg("ORG", 2, 0xE08FF, 1),
            msg("ORG", 2, 0xE0A69, 1),
            msg("ORG", 2, 0xE0A68, 9),
        ]
    )
    assert got == {0xE0900: 7, 0xE0A68: 9}


def test_rows_and_table_round_trip() -> None:
    rows = ft.rows_for(49, {0xE0900: 0x3F800000})
    assert rows == [{"type": "49", "dm": "0xe0900", "word": "0x3f800000", "value": "1"}]
    assert ft.table({0xE0900: 0x3F800000})[:2] == [1.0, 0.0]
    assert len(ft.table({})) == ft.TABLE_HI - ft.TABLE_LO + 1 == 361


def test_base_pointer_sits_inside_the_window_with_three_rows() -> None:
    assert ft.TABLE_LO < ft.BASE < ft.TABLE_HI
    assert ft.BASE + 2 * ft.ROW_STRIDE + 127 <= ft.TABLE_HI + 0x40


def test_committed_capture_has_five_types_of_361_words() -> None:
    tabs = ft.load_csv(DATA)
    assert sorted(tabs) == list(ft.TYPES)
    assert all(len(t) == 361 for t in tabs.values())


def test_committed_capture_fingerprint() -> None:
    tabs = ft.load_csv(DATA)
    h = hashlib.sha256()
    for t in ft.TYPES:
        for dm in sorted(tabs[t]):
            h.update(struct.pack("<II", dm, tabs[t][dm]))
    assert h.hexdigest()[:16] == "13a83db262affa8a"


def test_ramp_segment_is_a_linear_fade_of_70_steps_per_type() -> None:
    """EMPIRICAL (capture): `0xE0900 + k = (70 - k) / n` within 2e-6 relative (the ARM chain
    is a float recurrence, not one division), n by type; zero from k = 70."""
    tabs = ft.load_csv(DATA)
    for t, n in zip(ft.TYPES, (240, 30, 30, 51, 30), strict=True):
        for k in range(70):
            assert abs(ft.f32(tabs[t][0xE0900 + k]) - (70 - k) / n) <= 2e-6 * (70 - k) / n, (t, k)
        assert all(tabs[t][0xE0900 + k] == 0 for k in range(70, 100))


def test_decay_segments_start_at_the_documented_addresses() -> None:
    tabs = ft.load_csv(DATA)
    assert all(tabs[t][0xE0964 + 2] == 0 and tabs[t][0xE0967] != 0 for t in ft.TYPES)
    assert all(tabs[t][0xE09E7] == 0 and tabs[t][0xE09E8] != 0 for t in ft.TYPES)


def test_changed_offsets_lists_only_pokes_that_change_the_table() -> None:
    base = {0xE0900: 1}
    runs = {(0x98, 0): dict(base), (0xA8, 1): {0xE0900: 2}, (0xA8, 2): {0xE0900: 3}}
    assert ft.changed_offsets(base, runs) == {0xA8: [1, 2]}
