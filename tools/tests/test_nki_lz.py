import os
import sys

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import nki_lz as z  # noqa: E402


def lit(b: bytes) -> bytes:
    assert 1 <= len(b) <= 32
    return bytes([len(b) - 1]) + b


def short_copy(length: int, offset: int) -> bytes:
    assert 3 <= length <= 8 and 1 <= offset <= 8191
    o = offset - 1
    return bytes([0x20 + ((length - 3) << 5) + (o >> 8), o & 0xFF])


def long_copy(length: int, offset: int) -> bytes:
    assert length >= 9 and offset <= 8191
    o = offset - 1
    ext = bytearray()
    rest = length - 9
    while rest >= 255:
        ext.append(0xFF)
        rest -= 255
    ext.append(rest)
    return bytes([0xE0 + (o >> 8)]) + bytes(ext) + bytes([o & 0xFF])


def far_copy(length: int, offset: int) -> bytes:
    """Copy with offset >= 8192: base offset byte pair encodes 8192, then 2 more bytes."""
    extra = offset - 8192
    return bytes([0x20 + ((length - 3) << 5) + 0x1F, 0xFF, extra >> 8, extra & 0xFF])


def test_control_byte_classes():
    assert z.copy_length_class(0x00) == 1 and z.copy_length_class(0x1F) == 1
    assert [z.copy_length_class(c) for c in (0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0)] == [
        3,
        4,
        5,
        6,
        7,
        8,
    ]
    assert z.copy_length_class(0xE0) == 9 and z.copy_length_class(0xFF) == 9


def test_literals_and_short_copy():
    # "abcabcabc": literal abc then copy 6 from offset 3 (overlapping)
    data = lit(b"abc") + short_copy(6, 3)
    assert z.decompress(data) == b"abcabcabc"


def test_reference_vectors_from_ni_file_notes():
    # ni-file unit tests: 0x02 literal of 3, [0x20,0x0E] len 3 off 15, [0x60,0x00] len 5 off 1
    assert z.decompress(bytes([0x02]) + b"xyz") == b"xyz"
    base = lit(b"0123456789abcdef")
    assert z.decompress(base + bytes([0x20, 0x0E])) == b"0123456789abcdef123"
    assert z.decompress(lit(b"q") + bytes([0x60, 0x00])) == b"qqqqqq"


def test_extended_length_with_ff_continuation():
    data = lit(b"xy") + long_copy(300, 2)  # needs one 0xFF extension byte (9+255+36)
    assert z.decompress(data) == (b"xy" * 151)[:302]
    assert len(z.decompress(lit(b"z") + long_copy(9 + 255 + 255 + 3, 1))) == 1 + 9 + 513


def test_far_offset_escape():
    filler = bytes((i * 7) % 251 for i in range(9000))
    data = b"".join(lit(filler[i : i + 32]) for i in range(0, len(filler), 32))
    data += far_copy(5, 8192 + 100)
    out = z.decompress(data)
    assert out[:9000] == filler
    assert out[9000:] == filler[9000 - (8192 + 100) : 9000 - (8192 + 100) + 5]


def test_end_must_match_and_footer_is_found():
    stream = lit(b"abc") + short_copy(3, 3)
    footer = bytes(8) + b"hsin" + bytes([1]) * 20
    blob = b"HEAD" + stream + footer
    end = z.find_stream_end(blob, 4)
    assert end == 4 + len(stream)
    assert z.decompress(blob, 4, end) == b"abcabc"
    with pytest.raises(z.NkiLzError):
        z.decompress(blob, 4, end - 1)  # ends mid-token
    assert z.find_stream_end(b"no footer", 0) == len(b"no footer")


def test_errors():
    with pytest.raises(z.NkiLzError):
        z.decompress(bytes([0x05]) + b"ab")  # literal overruns
    with pytest.raises(z.NkiLzError):
        z.decompress(lit(b"a") + short_copy(3, 5))  # offset before start
    with pytest.raises(z.NkiLzError):
        z.decompress(lit(b"a") + bytes([0x20]))  # truncated copy


def test_find_start_script_length_and_cli(tmp_path):
    script = b"on init\nend on\n"
    stream = lit(script) + lit(b"\x00\x01\x02")
    blob = b"\x00" * 50 + stream
    assert z.find_stream_start(blob) == 50
    with pytest.raises(z.NkiLzError):
        z.find_stream_start(b"no marker here")
    out = z.decompress(blob, 50)
    assert z.script_length(out) == len(script)
    src = tmp_path / "a.nki"
    src.write_bytes(blob)
    assert z.main([str(src), str(tmp_path / "o.bin"), "--script", str(tmp_path / "s.ksp")]) == 0
    assert (tmp_path / "s.ksp").read_bytes() == script
