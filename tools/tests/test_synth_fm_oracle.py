"""Tests for the FM stage oracle driver helpers (no firmware, no oracle run)."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_fm_oracle as fo  # noqa: E402
import synth_fm_tables as ft  # noqa: E402

DATA = Path(__file__).resolve().parents[2] / "docs" / "research" / "data"


def test_random_pokes_are_deterministic_and_carry_table_and_words() -> None:
    table = {0xE0900: 1}
    a = fo.random_pokes(3, table, {0xC1599: 7})
    b = fo.random_pokes(3, table, {0xC1599: 7})
    assert a == b
    assert a[0xE0900] == 1 and a[0xC1599] == 7
    assert fo.random_pokes(4, table, {}) != a


def test_flat_reader_prefers_the_overlay_and_reads_little_endian_words() -> None:
    img = struct.pack("<2I", 5, 6)
    read = fo.flat_reader(img, {1: 9})
    assert (read(0), read(1)) == (5, 9)


def test_model_runs_a_listing_and_reports_written_words() -> None:
    img = struct.pack("<4I", 0, 0, 0, 0) * 0x40
    lines = [(0, "R0 = 0x3f800000"), (4, "DM(8, I2) = R0"), (8, "RTS")]
    got = fo.model(lines, img, {})
    assert got == {fo.REGS[2] + 8: 0x3F800000, fo.REGS[2] + 9: 0x3F800000}


def test_window_words_cover_the_table_and_the_committed_p_words_exist() -> None:
    words = set(fo.window_words())
    assert set(range(ft.TABLE_LO, ft.TABLE_HI + 1)) <= words
    assert fo.wo_words(49), "synth-p-words.csv has the type 49 words"
    assert (DATA / "synth-fm-tables.csv").exists()
