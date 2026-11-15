"""Unit tests for tools/synth_sustped.py (synthetic payloads and a synthetic backup zip)."""

from __future__ import annotations

import zipfile

import synth_sustped as ss


def payload_with(bits: tuple[int, int, int]) -> bytes:
    buf = bytearray(820)
    for block, bit in zip(ss.LAYER_BLOCKS, bits):
        if bit:
            pos = ss.SYNTH_SECTION * 8 + block + ss.SUSTAIN_FIELD
            buf[pos // 8] |= 0x80 >> (pos % 8)
    return bytes(buf)


def test_bits_are_read_per_layer():
    assert ss.synth_sustain_bits(payload_with((1, 0, 1))) == (1, 0, 1)
    assert ss.synth_sustain_bits(bytes(820)) == (0, 0, 0)


def test_scan_and_summary_over_a_synthetic_backup(tmp_path):
    archive = tmp_path / "b.ns4b"
    with zipfile.ZipFile(archive, "w") as z:
        z.writestr("Program/Bank A/One.ns4p", bytes(48) + payload_with((1, 1, 1)))
        z.writestr("Program/Bank A/Two.ns4p", bytes(48) + payload_with((0, 1, 1)))
        z.writestr("Sound/skip.ns4s", b"x")
    rows = ss.scan(archive)
    assert [r["program"] for r in rows] == ["Program/Bank A/One.ns4p", "Program/Bank A/Two.ns4p"]
    assert ss.summary(rows) == {
        "programs": 2,
        "set": {"synth_a": 1, "synth_b": 2, "synth_c": 2},
        "off_default": 1,
    }


def test_main_reports_a_missing_backup(tmp_path, capsys):
    assert ss.main([str(tmp_path / "none.ns4b")]) == 2
    assert "missing backup" in capsys.readouterr().err


def test_main_writes_the_csv(tmp_path):
    archive = tmp_path / "b.ns4b"
    with zipfile.ZipFile(archive, "w") as z:
        z.writestr("Program/Bank A/One.ns4p", bytes(48) + payload_with((1, 0, 1)))
    out = tmp_path / "o.csv"
    assert ss.main([str(archive), "--csv", str(out)]) == 0
    assert out.read_text(encoding="utf-8").splitlines()[1] == "Program/Bank A/One.ns4p,1,0,1"
