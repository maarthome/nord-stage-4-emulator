import csv
import os
import struct
import sys
import zlib

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import nkr_extract as nx  # noqa: E402


def png(width, height):
    def chunk(tag, body):
        crc = zlib.crc32(tag + body)
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    raw = zlib.compress(b"\x00" + b"\x00\x00\x00" * width)
    return nx.PNG_SIG + chunk(b"IHDR", ihdr) + chunk(b"IDAT", raw) + chunk(b"IEND", b"")


def node(name, offset, kind):
    body = name.encode("utf-16le") + b"\x00\x00"
    size = 2 + 4 + 2 + len(body)
    return struct.pack("<HIH", size, offset, kind) + body


def block(payload):
    head = nx.MAGIC + struct.pack("<H", 0x0111) + bytes(4) + b"\xff\x00\x00\x00"
    head += struct.pack("<I", len(payload)) + bytes(4)
    assert len(head) == nx.HEADER
    return head + payload


def make_container(files):
    """Folder node plus file nodes; data blocks follow the directory."""
    names = [n for n, _ in files]
    dir_len = len(node("pictures", 0, nx.TYPE_FOLDER)) + sum(
        len(node(n, 0, nx.TYPE_FILE)) for n in names
    )
    off = dir_len
    entries = node("pictures", 0, nx.TYPE_FOLDER)
    blocks = b""
    for name, payload in files:
        entries += node(name, off + len(blocks), nx.TYPE_FILE)
        blocks += block(payload)
    return entries + blocks


FILES = [
    ("up2_a.txt", b"Number of Animations: 3\r\nResizable: no"),
    ("up2_a.PNG", png(5, 4)),
    ("up2_b#.PNG", png(7, 2)),
]


def test_parse_entries_lists_folder_and_files():
    ents = nx.parse_entries(make_container(FILES))
    assert [(e.name, e.type) for e in ents] == [
        ("pictures", nx.TYPE_FOLDER),
        ("up2_a.txt", nx.TYPE_FILE),
        ("up2_a.PNG", nx.TYPE_FILE),
        ("up2_b#.PNG", nx.TYPE_FILE),
    ]


def test_read_file_returns_exact_payload_and_png_size():
    data = make_container(FILES)
    by_name = {e.name: e for e in nx.parse_entries(data)}
    payload = nx.read_file(data, by_name["up2_a.PNG"])
    assert payload == FILES[1][1]
    assert nx.png_size(payload) == (5, 4)
    assert nx.png_size(b"not a png") is None


def test_blocks_are_contiguous():
    data = make_container(FILES)
    files = [e for e in nx.parse_entries(data) if e.type == nx.TYPE_FILE]
    for a, b in zip(files, files[1:]):
        assert b.offset == a.offset + nx.HEADER + len(nx.read_file(data, a))


def test_bad_magic_and_overrun_raise():
    data = bytearray(make_container(FILES))
    ent = [e for e in nx.parse_entries(bytes(data)) if e.type == nx.TYPE_FILE][0]
    bad = bytearray(data)
    bad[ent.offset] ^= 0xFF
    with pytest.raises(nx.NkrError):
        nx.read_file(bytes(bad), ent)
    over = bytearray(data)
    struct.pack_into("<I", over, ent.offset + nx.SIZE_AT, 1 << 30)
    with pytest.raises(nx.NkrError):
        nx.read_file(bytes(over), ent)


def test_no_files_gives_empty_list():
    assert nx.parse_entries(node("pictures", 0, nx.TYPE_FOLDER)) == []


def test_safe_name_replaces_reserved_characters():
    assert nx.safe_name('a<b>c:d"e/f\\g|h?i*j') == "a_b_c_d_e_f_g_h_i_j"
    assert nx.safe_name("up2_b#.PNG") == "up2_b#.PNG"


def test_extract_and_cli(tmp_path, capsys):
    src = tmp_path / "x.nkr"
    src.write_bytes(make_container(FILES))
    out = tmp_path / "out"
    assert nx.main([str(src), str(out)]) == 0
    assert "3 files, 2 PNG" in capsys.readouterr().out
    assert (out / "up2_a.PNG").read_bytes() == FILES[1][1]
    rows = {r["name"]: r for r in csv.DictReader((out / "manifest.csv").open(encoding="utf-8"))}
    assert rows["up2_b#.PNG"]["width"] == "7" and rows["up2_b#.PNG"]["height"] == "2"
    assert rows["up2_a.txt"]["text"] == "Number of Animations: 3; Resizable: no"
