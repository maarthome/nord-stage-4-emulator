"""Unit tests for tools/fx_reverb_names.py (synthetic image, no firmware bytes)."""

from __future__ import annotations

import struct

import fx_reverb_names as rn


def synthetic_image() -> bytes:
    """Strings first, then a pointer table over them, as the ARM image lays them out."""
    names = [*rn.EXPECTED_TYPES, "Normal", "Dark", "Bright"]
    blob = bytearray(b"\0" * 16)
    offsets = []
    for n in names:
        offsets.append(len(blob))
        blob += n.encode() + b"\0"
    while len(blob) % 4:
        blob += b"\0"
    blob += struct.pack(f"<{len(offsets)}I", *[rn.ARM_BASE + o for o in offsets])
    return bytes(blob)


def test_finds_the_table_and_the_names_in_order() -> None:
    img = synthetic_image()
    addr, names = rn.find_names(img)
    assert names[:12] == rn.EXPECTED_TYPES
    assert names[12:] == ["Normal", "Dark", "Bright"]
    assert addr - rn.ARM_BASE == len(img) - 15 * 4


def test_type_index_is_model_and_variation() -> None:
    # model = index // 2 (Spring, Booth, Room, Stage, Hall, Cathedral), variation = index & 1
    models = [n.split()[0].rstrip("2") for n in rn.EXPECTED_TYPES[::2]]
    assert models == ["Spring", "Booth", "Room", "Stage", "Hall", "Cathedral"]
    for i, name in enumerate(rn.EXPECTED_TYPES):
        assert name.split()[0].rstrip("2") == models[i // 2]


def test_missing_table_is_reported() -> None:
    assert rn.find_names(b"no names here") == (0, [])
    assert rn.main.__name__ == "main"


def test_c_string_outside_the_image_is_empty() -> None:
    assert rn.c_string(b"abc\0", rn.ARM_BASE + 100) == ""
    assert rn.c_string(b"abc\0", rn.ARM_BASE) == "abc"
