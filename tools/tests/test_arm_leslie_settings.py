"""Tests for tools/arm_leslie_settings.py: the literal / STRD evaluator on synthetic ARM words."""

import struct

import arm_leslie_settings as als


def _image(words: list[int]) -> bytes:
    return struct.pack(f"<{len(words)}I", *words)


def test_decode_double_reads_the_two_literal_words():
    assert als.decode_double(0x9999999A, 0x3FE99999) == 0.8
    assert als.decode_double(0, 0x401C0000) == 7.0


def test_evaluate_follows_ldr_literal_mov_strd_and_branch():
    # 0: ldr r2, [pc, #0x14] -> word 6   4: ldr r3, [pc, #0x14] -> word 7
    # 8: mov r8, #0           c: b +0 (to 0x14)   14: strd r2, r3, [r0, #0x48]; 18: bx lr
    words = [
        0xE59F2010,
        0xE59F3010,
        0xE3A08000,
        0xEA000000,
        0xE1C024F8,
        0xE12FFF1E,
        0x9999999A,
        0x3FE99999,
    ]
    # the branch at 0xc jumps to 0xc + 8 + 0 = 0x14; the strd sits at 0x10, so it is skipped
    assert als.evaluate(_image(words), als.LOAD_BASE) == {}
    words[3] = 0xEAFFFFFF  # b -1 word: target 0xc + 8 - 4 = 0x10, the strd
    assert als.evaluate(_image(words), als.LOAD_BASE) == {0x48: 0.8}


def test_evaluate_stops_at_the_first_unsupported_instruction():
    words = [0xE59F2008, 0xE59F3008, 0xE12FFF1E, 0xE1C024F8, 0x0, 0x3FF00000]
    assert als.evaluate(_image(words), als.LOAD_BASE) == {}


def test_handler_tables_cover_every_choice_of_every_setting():
    for setting, blocks in als.BLOCKS.items():
        assert sorted(blocks) == [0, 1, 2], setting
        assert len(als.FIELDS[setting]) == 2, setting
