"""Unit tests for tools/ns4_arm_init_script.py (synthetic words, no firmware needed)."""

from __future__ import annotations

import ns4_arm_init_script as s


def test_parse_records_reads_type_4_blocks_and_skips_strays() -> None:
    words = [0x93DCF, 4, 2, 0x93E0C, 0xFFFFFFFF, 7, 1, 1, 0x3D, 4, 1, 0x95C69, 0x92426]
    assert s.parse_records(words) == [(0x93E0C, [0xFFFFFFFF, 7]), (0x95C69, [0x92426])]


def test_parse_records_rejects_addresses_outside_dm() -> None:
    assert s.parse_records([4, 1, 0x100, 5, 0, 0]) == []


def test_poke_lines_one_per_word() -> None:
    assert s.poke_lines([(0x95C7B, [0x95C6D, 0x1C12DD])]) == [
        "poke 0x95C7B 0x95C6D",
        "poke 0x95C7C 0x1C12DD",
    ]


def test_sy_records_from_a_synthetic_image() -> None:
    import struct

    size = s.SY_SPAN[1] - s.LOAD_BASE
    code = bytearray(size)
    struct.pack_into("<4I", code, s.SY_SPAN[0] - s.LOAD_BASE, 4, 1, 0x95C7B, 0x95C6D)
    assert s.sy_records(bytes(code)) == [(0x95C7B, [0x95C6D])]
