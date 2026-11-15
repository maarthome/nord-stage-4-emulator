"""Tests for tools/midi_events.py with a tiny hand-written SMF (no torch needed)."""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import midi_events as me  # noqa: E402


def vlq(n: int) -> bytes:
    out = [n & 0x7F]
    n >>= 7
    while n:
        out.append(0x80 | (n & 0x7F))
        n >>= 7
    return bytes(reversed(out))


def smf(track: bytes, division: int = 480) -> bytes:
    head = b"MThd" + struct.pack(">IHHH", 6, 0, 1, division)
    return head + b"MTrk" + struct.pack(">I", len(track)) + track


def fixture() -> bytes:
    """120 bpm, 480 tpq: 1 tick = 1/960 s. C4 v80 0..0.5 s, pedal down at 0.25 s, E4 v0-off."""
    t = vlq(0) + b"\xff\x51\x03" + (500_000).to_bytes(3, "big")
    t += vlq(0) + bytes([0x90, 60, 80])
    t += vlq(240) + bytes([0xB0, 64, 127])  # 0.25 s
    t += vlq(240) + bytes([0x80, 60, 0])  # 0.5 s
    t += vlq(0) + bytes([0x90, 64, 100])  # 0.5 s, then velocity-0 off via running status
    t += vlq(480) + bytes([64, 0])  # running status: note on, vel 0 = off at 1.0 s
    t += vlq(480) + bytes([0xB0, 64, 0])  # pedal up at 1.5 s
    t += vlq(0) + b"\xff\x2f\x00"
    return smf(t)


def test_parse_notes_velocity_and_pedal():
    p = me.parse_midi(fixture())
    assert p["notes"] == [
        {"midi": 60, "onset_s": 0.0, "offset_s": 0.5, "velocity": 80},
        {"midi": 64, "onset_s": 0.5, "offset_s": 1.0, "velocity": 100},
    ]
    assert p["pedal"] == [{"t": 0.25, "value": 127}, {"t": 1.5, "value": 0}]


def test_tempo_change_scales_time():
    t = vlq(0) + b"\xff\x51\x03" + (1_000_000).to_bytes(3, "big")  # 60 bpm: 1 s per quarter
    t += vlq(0) + bytes([0x90, 60, 64]) + vlq(480) + bytes([0x80, 60, 0])
    p = me.parse_midi(smf(t))
    assert p["notes"][0]["offset_s"] == 1.0


def test_held_note_closed_at_end_and_min_length():
    t = vlq(0) + bytes([0x90, 60, 64]) + vlq(0) + bytes([0x80, 60, 0])
    t += vlq(0) + bytes([0x90, 62, 64]) + vlq(480) + bytes([0xB0, 1, 5])
    notes = me.parse_midi(smf(t))["notes"]
    assert notes[0]["offset_s"] - notes[0]["onset_s"] >= me.MIN_NOTE_S
    assert notes[1]["offset_s"] == 0.5  # closed at the last event


def test_bad_header_and_smpte_rejected():
    for bad in (b"RIFF" + bytes(20), b"MThd" + struct.pack(">IHHH", 6, 0, 1, 0xE728)):
        try:
            me.parse_midi(bad)
        except ValueError:
            continue
        raise AssertionError("expected ValueError")


def test_to_events_offset_and_slice():
    doc = me.to_events(me.parse_midi(fixture()), "t", offset_s=10.0)
    assert doc["events"][0]["onset_s"] == 10.0 and doc["pedal"][0]["t"] == 10.25
    cut = me.slice_events(doc, 10.4, 0.8)  # keeps the E4 only; pedal down is in force at 0
    assert [e["midi"] for e in cut["events"]] == [64]
    assert cut["events"][0]["onset_s"] == 0.1
    assert cut["events"][0]["offset_s"] == 0.6
    assert cut["pedal"][0] == {"t": 0.0, "value": 127}
    assert cut["source"] == "t"


def test_csv_and_cli(tmp_path):
    mid = tmp_path / "a.mid"
    mid.write_bytes(fixture())
    out, csv = tmp_path / "e.json", tmp_path / "e.csv"
    assert me.main([str(mid), "--out", str(out), "--csv", str(csv)]) == 0
    assert json.loads(out.read_text())["events"][1]["velocity"] == 100
    lines = csv.read_text().splitlines()
    assert lines[0].startswith("kind,") and len(lines) == 5
    assert lines[1] == "note,0.0,0.5,60,80,"
