"""Unit tests for tools/arm_touch_curves.py on a synthetic firmware image (no real firmware)."""

from __future__ import annotations

import struct

import arm_touch_curves as tc


def build_image() -> bytes:
    image = bytearray(0x600000)

    def put(address: int, data: bytes) -> None:
        image[address - tc.LOAD_BASE : address - tc.LOAD_BASE + len(data)] = data

    put(tc.TOUCH_TABLE, struct.pack("<3I", 10, 11, 13))
    # r8(v) = 4 * v on the default curve (so r8 stays under 512).
    for v in range(0, 128):
        put(
            tc.R8_TABLE + tc.R8_OFFSET + 2 * (v + 128 * tc.DEFAULT_R8_CURVE),
            struct.pack("<h", 4 * v),
        )
    # T[curveId][r8] = (curveId * 7 + r8 // 4) % 251.
    for cid in (10, 11, 13):
        base = tc.T_TABLE + (cid << 10)
        for r8 in range(512):
            image[base - tc.LOAD_BASE + r8] = (cid * 7 + r8 // 4) % 251
    return bytes(image)


def test_curve_ids_and_r8():
    code = build_image()
    assert tc.curve_ids(code) == [10, 11, 13]
    assert tc.r8_of(code, 1) == 4
    assert tc.r8_of(code, 127) == 508


def test_table_composes_the_two_firmware_tables():
    code = build_image()
    t = tc.table(code, 11)
    assert len(t) == 127
    assert t[0] == (77 + 1) % 251
    assert t[126] == (77 + 127) % 251
    assert tc.t_of(code, 13, 64) == (91 + 64) % 251


def test_fingerprint_changes_with_any_byte_and_rust_array_wraps():
    a = [list(range(127)), list(range(127)), list(range(127))]
    b = [list(x) for x in a]
    b[2][100] ^= 1
    assert tc.fingerprint(a) != tc.fingerprint(b)
    text = tc.rust_array("T_X", list(range(127)))
    assert text.startswith("const T_X: [u8; 127] = [")
    assert text.endswith("];")
    assert all(len(line) <= 92 for line in text.splitlines())


def test_extract_reads_a_file(tmp_path):
    path = tmp_path / "code.bin"
    path.write_bytes(build_image())
    ids, tables = tc.extract(path)
    assert ids == [10, 11, 13]
    assert [len(t) for t in tables] == [127, 127, 127]
