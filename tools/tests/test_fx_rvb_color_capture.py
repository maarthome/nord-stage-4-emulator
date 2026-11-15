"""Unit tests for tools/fx_rvb_color_capture.py (no firmware)."""

from types import SimpleNamespace

import fx_rvb_color_capture as cc


def test_pre_type_always_differs_from_the_target_type():
    assert all(cc.pre_type(t) != t for t in range(2, 12))


def test_row_formats_a_message():
    m = SimpleNamespace(core=2, dm_word=0xE0C46, word2=0x4018A4A8, pc=0x1234)
    r = cc.row(4, 1, m)
    assert r == {
        "type": 4,
        "color": 1,
        "core": 2,
        "dm_word": "0xe0c46",
        "word": "0x4018a4a8",
        "pc": "0x1234",
    }
    assert set(r) == set(cc.FIELDS)
