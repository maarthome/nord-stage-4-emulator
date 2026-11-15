"""Unit tests for tools/arm_key_timing.py on a synthetic firmware image (no real firmware)."""

from __future__ import annotations

import struct

import arm_key_timing as kt
import arm_xref as ax


def mov_imm(rd: int, value: int) -> int:
    """`MOV rd, #value` with the smallest rotation that fits."""
    for rot in range(16):
        imm = ((value << (rot * 2)) | (value >> (32 - rot * 2))) & 0xFFFFFFFF if rot else value
        if imm <= 0xFF:
            return 0xE3A00000 | (rd << 12) | (rot << 8) | imm
    raise ValueError(value)


def bl(address: int, target: int) -> int:
    return 0xEB000000 | (((target - (address + 8)) >> 2) & 0xFFFFFF)


def put(image: bytearray, address: int, words: list[int]) -> None:
    for i, w in enumerate(words):
        struct.pack_into("<I", image, address - ax.LOAD_BASE + 4 * i, w)


def build_image() -> bytes:
    image = bytearray(kt.INIT_END - ax.LOAD_BASE + 0x100)
    put(
        image,
        kt.DEFAULTS_ROUTINE,
        [mov_imm(2, 300), mov_imm(1, 80), 0xE5801004, 0xE5802000, 0xE5802008, 0xE12FFF1E],
    )
    a = kt.INIT_START
    words = [mov_imm(5, 0)]
    # key 81: hold 1000 via immediates, r3 copied from r5 (0)
    words += [mov_imm(1, 81), mov_imm(2, 250 * 4), 0xE1A03005]
    words += [0]  # placeholder for the bl, patched below
    # key 146: hold from a literal word, repeat 80
    words += [mov_imm(1, 146), 0xE59F2000 | 0x20, mov_imm(3, 80), 0]
    put(image, a, words)
    put(image, a + 4 * 4, [bl(a + 4 * 4, kt.SETTER)])
    put(image, a + 4 * 8, [bl(a + 4 * 8, kt.SETTER)])
    # literal at pc + 8 + 0x20 from the ldr at index 6
    struct.pack_into("<I", image, a + 4 * 6 + 8 + 0x20 - ax.LOAD_BASE, 350)
    return bytes(image)


def test_steps_is_the_umull_quotient_by_six() -> None:
    for ms in (0, 5, 6, 80, 300, 350, 500, 1000):
        assert kt.steps(ms) == ms // 6
    assert (kt.steps(500), kt.steps(1000), kt.steps(350), kt.steps(80)) == (83, 166, 58, 13)


def test_decode_defaults_reads_the_three_config_words() -> None:
    assert kt.decode_defaults(build_image()) == {
        "hold_ms": 300,
        "repeat_ms": 80,
        "double_tap_word": 300,
    }


def test_decode_table_tracks_immediates_register_moves_and_literals() -> None:
    table = kt.decode_table(build_image())
    assert table == [kt.KeyTiming(81, 1000, 0), kt.KeyTiming(146, 350, 80)]
    assert (table[0].hold_steps, table[1].repeat_steps) == (166, 13)


def test_rust_rows_formats_one_tuple_per_key() -> None:
    rows = kt.rust_rows([kt.KeyTiming(81, 1000, 0), kt.KeyTiming(146, 350, 80)])
    assert rows == "    (81, 1000, 0),\n    (146, 350, 80),"


def test_decode_mov_reg_only_matches_plain_register_moves() -> None:
    assert kt.decode_mov_reg(0xE1A03005) == (3, 5)
    assert kt.decode_mov_reg(mov_imm(3, 4)) is None
