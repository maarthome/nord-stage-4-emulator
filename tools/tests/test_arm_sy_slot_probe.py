"""Unit tests for tools/arm_sy_slot_probe.py (pure word helpers, no firmware needed)."""

from __future__ import annotations

import arm_sy_slot_probe as probe


def test_word0_low_half_matches_the_arm_orr_constant() -> None:
    # `orr r4, r4, #0x4000000` at 0xC01E2B0C: slot 0 -> 0x04000000
    assert probe.word0(0) == 0x0400_0000
    assert probe.word0(5) == 0x0400_0005


def test_word0_high_half_matches_the_second_orr_constant() -> None:
    # `orr r4, r4, #0x4100000` at 0xC01E2C74: spec 0x0410
    assert probe.word0(3, high=True) == 0x0410_0003


def test_unused_lane_is_all_ones_as_the_harness_captured() -> None:
    # the captured 0x9219E message was 0xFFFFFFFF (slot -1 ORed with the spec)
    assert probe.word0(-1) == 0xFFFF_FFFF
    assert probe.decode_word0(0xFFFF_FFFF)[0] == 0xFFFF


def test_decode_word0_splits_index_and_field_spec() -> None:
    assert probe.decode_word0(0x0400_0036) == (0x36, 0, 16)
    assert probe.decode_word0(0x0410_0000) == (0, 16, 16)


def test_entry_address_for_the_neutral_voice_reset_index() -> None:
    # the DSP voice reset 0x1C0794 stores 0x36 | 0x4000000: DM(0x93580), the 0x80008000 word
    assert probe.entry_address(0x36) == 0x93580
    assert probe.entry_address(0) == 0x9354A


def test_window_bounds() -> None:
    assert probe.in_window(0x9354A)
    assert not probe.in_window(0x935A0)
    assert not probe.in_window(0x9219E)
