import csv
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import nki_zones as nz  # noqa: E402


def make_zone(zid, vlo, vhi, klo, khi, root, fidx, frames, rate=44100, size=226):
    r = bytearray(b"\x00" * size)
    r[: len(nz.ZONE_SIG)] = nz.ZONE_SIG
    struct.pack_into("<I", r, 44, zid)
    struct.pack_into("<4H", r, 64, vlo, vhi, klo, khi)
    struct.pack_into("<H", r, 80, root)
    struct.pack_into("<I", r, 100, fidx)
    struct.pack_into("<I", r, 108, rate)
    struct.pack_into("<I", r, 113, frames)
    return bytes(r)


def name_entry(name):
    return b"\x04" + struct.pack("<I", len(name)) + name.encode("utf-16le") + b"\x00\x00"


def make_blob():
    zones = [
        make_zone(10, 1, 43, 21, 22, 21, 7, 661500),
        make_zone(11, 44, 52, 21, 22, 21, 8, 600000, size=254),
        make_zone(12, 1, 127, 23, 24, 23, 9, 100),
    ]
    names = [
        "Astoria Grand DN 021 043.wav",
        "Astoria Grand DN 021 052.wav",
        "Pad 023.wav",
        "notes.txt",
    ]
    return b"HEAD" + b"".join(zones) + b"\xaa\xaa" + b"".join(name_entry(n) for n in names)


def test_parse_zones_fields():
    z = nz.parse_zones(make_blob())
    assert [(x.zone_id, x.vel_lo, x.vel_hi, x.key_lo, x.key_hi, x.root) for x in z] == [
        (10, 1, 43, 21, 22, 21),
        (11, 44, 52, 21, 22, 21),
        (12, 1, 127, 23, 24, 23),
    ]
    assert z[0].frames == 661500 and z[0].rate == 44100 and z[0].file_index == 7


def test_names_skip_non_wav_and_base_attach():
    blob = make_blob()
    names = nz.parse_names(blob)
    assert names == ["Astoria Grand DN 021 043.wav", "Astoria Grand DN 021 052.wav", "Pad 023.wav"]
    z = nz.parse_zones(blob)
    assert nz.attach_names(z, names) == 7
    assert [x.name for x in z] == names
    assert nz.attach_names(z, names, base=8) == 8 and z[0].name == ""


def test_check_names_counts_match_mismatch_and_other():
    z = nz.parse_zones(make_blob())
    nz.attach_names(z, nz.parse_names(make_blob()))
    assert nz.check_names(z) == (2, 0, 1)  # third name has no velocity
    z[0].vel_hi = 99
    assert nz.check_names(z) == (1, 1, 1)


def test_check_frames_against_catalog():
    z = nz.parse_zones(make_blob())
    nz.attach_names(z, nz.parse_names(make_blob()))
    cat = {"Astoria Grand DN 021 043": 661500, "Astoria Grand DN 021 052": 1}
    assert nz.check_frames(z, cat) == (1, 1)


def test_csv_and_cli(tmp_path, capsys):
    src = tmp_path / "dec.bin"
    src.write_bytes(make_blob())
    cat = tmp_path / "cat.csv"
    cat.write_text("set,note,velocity,frames\nAstoria Grand DN,21,43,661500\n", encoding="utf-8")
    out = tmp_path / "z.csv"
    assert nz.main([str(src), str(out), "--catalog", str(cat)]) == 0
    text = capsys.readouterr().out
    assert "3 zones (file base 7)" in text and "1 equal, 0 different" in text
    rows = list(csv.DictReader(out.open(encoding="utf-8")))
    assert rows[0]["name"] == "Astoria Grand DN 021 043.wav" and rows[0]["key_hi"] == "22"
