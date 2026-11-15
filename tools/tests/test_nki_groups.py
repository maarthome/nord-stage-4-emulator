import csv
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import nki_groups as ng  # noqa: E402


def utf16(name):
    return struct.pack("<I", len(name)) + name.encode("utf-16le") + b"\x00\x00"


def ident(text):
    return struct.pack("<I", len(text)) + text.encode("ascii")


def group_blob(name, volume, mods):
    body = ident("volume") + struct.pack("<f", 1.0) + ident("volume") + struct.pack("<f", 1.0)
    body += ident("volume") + struct.pack("<f", volume)
    for m in mods:
        body += b"\x00\x01" + ident(m) + b"\x00" * 6
    return utf16(name) + b"\x00" * 8 + body


def make_blob():
    return (
        b"SCRIPT\x00\x00"
        + group_blob(
            "astoria .grand #dn", 0.78125, ["ENV_AHDSR_VOLUME", "VEL_VOLUME", "KSP_CUTOFF"]
        )
        + group_blob("astoria .grand #ko", 0.5, ["ENV_AHDSR_VOLUME"])
        + b"ZONES"
    )


def test_group_names_found_and_non_groups_ignored():
    blob = make_blob() + utf16("not a group name") + utf16("pads .misc #up")
    names = [n for _, n in ng.find_group_names(blob, 0, len(blob))]
    assert names == ["astoria .grand #dn", "astoria .grand #ko", "pads .misc #up"]


def test_volume_and_modulators_per_group():
    blob = make_blob()
    gs = ng.parse_groups(blob, 8, len(blob))
    assert [g.name for g in gs] == ["astoria .grand #dn", "astoria .grand #ko"]
    assert gs[0].volume == 0.78125 and gs[1].volume == 0.5
    assert gs[0].modulators == ["ENV_AHDSR_VOLUME", "VEL_VOLUME", "KSP_CUTOFF"]
    assert gs[1].modulators == ["ENV_AHDSR_VOLUME"]


def test_group_without_volume_identifier():
    blob = b"xx" + utf16("empty .misc #up") + b"\x00" * 12
    (g,) = ng.parse_groups(blob, 0, len(blob))
    assert g.volume is None and g.modulators == []


def test_identifiers_ignore_short_or_truncated_text():
    rec = ident("pan") + b"\x05\x00\x00\x00ab" + ident("pitch")
    assert [t for _, t in ng.identifiers(rec)] == ["pan", "pitch"]


def test_csv_and_cli(tmp_path, capsys):
    zone_sig = b"\xff" * 7 + b"\x00\x01\x00\x00\x00"
    blob = make_blob() + zone_sig + b"\x00" * 20 + utf16("late .misc #up")
    src = tmp_path / "dec.bin"
    src.write_bytes(blob)
    out = tmp_path / "g.csv"
    assert ng.main([str(src), "8", str(out)]) == 0
    assert "2 groups, volume range 0.500..0.781" in capsys.readouterr().out
    rows = list(csv.DictReader(out.open(encoding="utf-8")))
    assert rows[0]["name"] == "astoria .grand #dn" and rows[0]["volume"] == "0.781250"
    assert rows[1]["modulators"] == "ENV_AHDSR_VOLUME"
