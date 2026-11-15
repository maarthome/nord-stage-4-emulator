"""Unit tests for tools/fx_rvb_colors.py."""

import fx_rvb_colors as rc


def rows(t, c, words):
    return [
        {"type": str(t), "color": str(c), "dm_word": hex(a), "word": hex(w)}
        for a, w in words.items()
    ]


def test_delta_keeps_only_words_that_differ_from_the_type_block():
    r = rows(2, 0, {0x100: 1, 0x101: 2}) + rows(2, 1, {0x100: 1, 0x101: 9, 0x102: 3})
    assert rc.reduce_rows(r) == [(2, 1, 0x101, 9), (2, 1, 0x102, 3)]


def test_a_colour_without_a_type_block_is_an_error():
    try:
        rc.reduce_rows(rows(2, 1, {0x100: 1}))
    except ValueError:
        return
    raise AssertionError("expected ValueError")


def test_windows_cover_the_instance_the_ring_and_the_type_table_only():
    assert rc.in_window(0xC1A3D) and rc.in_window(0xE0C46) and rc.in_window(0x9438E)
    assert not rc.in_window(0xB2F25)


def test_committed_csv_regenerates_the_committed_rust_file():
    assert rc.RUST.read_text(encoding="utf-8") == rc.rust_file(rc.read_csv(rc.DATA))
