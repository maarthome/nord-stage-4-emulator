"""Tests for tools/ns4_pstick_range.py on a synthetic string blob (no firmware)."""

import ns4_pstick_range as pr


def _blob(values: list[int]) -> bytes:
    strings = b"".join(b"+2\xa3" + f"{v} semi".encode() + b"\x00" for v in values)
    return b"junk 7 semi\x00" + b"\x00" * 200 + strings + pr.LABEL + b"\x00\x00Synth"


def test_the_choices_before_the_label_are_read_in_menu_order():
    values = [1, 2, 3, 4, 5, 7, 10, 12, -12, -24]
    assert pr.pstick_range_list(_blob(values)) == values


def test_strings_outside_the_window_are_not_choices():
    assert 7 not in pr.pstick_range_list(_blob([1, 2]))


def test_missing_label_is_an_error():
    try:
        pr.pstick_range_list(b"no label here")
    except ValueError:
        return
    raise AssertionError("expected ValueError")
