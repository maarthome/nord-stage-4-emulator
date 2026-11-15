"""Tests for tools/muscriptor_events.py (pure conversion; no torch, no model)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import muscriptor_events as me  # noqa: E402


def fixture() -> list[dict]:
    return [
        {"type": "start", "pitch": 60, "start_time": 1.0, "index": 0, "instrument": "synth_lead"},
        {"type": "start", "pitch": 36, "start_time": 0.5, "index": 1, "instrument": "drums"},
        {"type": "end", "end_time": 1.5, "start_event_index": 0},
        {"type": "end", "end_time": 0.6, "start_event_index": 1},
        {"type": "start", "pitch": 64, "start_time": 2.0, "index": 2, "instrument": "synth_lead"},
    ]


def test_pair_notes_joins_and_sorts() -> None:
    notes = me.pair_notes(fixture())
    assert [(n["midi"], n["onset_s"], n["offset_s"]) for n in notes] == [
        (36, 0.5, 0.6),
        (60, 1.0, 1.5),
        (64, 2.0, 2.02),  # no end event: minimum length
    ]
    assert "velocity" not in notes[0]


def test_to_doc_offset_drop_velocity() -> None:
    doc = me.to_doc(fixture(), "t", offset_s=10.0, drop=["drums"], velocity=70)
    assert [e["midi"] for e in doc["events"]] == [60, 64]
    assert doc["events"][0]["onset_s"] == 11.0 and doc["events"][0]["velocity"] == 70
    assert doc["pedal"] == [] and doc["source"] == "t"


def test_to_doc_keep_and_no_velocity() -> None:
    doc = me.to_doc(fixture(), "t", keep=["drums"])
    assert [e["midi"] for e in doc["events"]] == [36]
    assert "velocity" not in doc["events"][0]


def test_instrument_counts_sorted() -> None:
    assert me.instrument_counts(me.to_doc(fixture(), "t")) == {"synth_lead": 2, "drums": 1}


def test_convert_cli_roundtrip(tmp_path: Path) -> None:
    src, out = tmp_path / "in.json", tmp_path / "out.json"
    src.write_text(json.dumps(fixture()), encoding="utf-8")
    rc = me.main(
        ["convert", str(src), "--out", str(out), "--offset", "5", "--drop-instruments", "drums"]
    )
    assert rc == 0
    doc = json.loads(out.read_text(encoding="utf-8"))
    assert [e["onset_s"] for e in doc["events"]] == [6.0, 7.0]


def test_run_uses_cached_files(tmp_path: Path) -> None:
    (tmp_path / "c.wav").write_bytes(b"")
    (tmp_path / "c.medium.json").write_text(json.dumps(fixture()), encoding="utf-8")
    out = tmp_path / "ev.json"
    rc = me.main(
        ["run", "--media", "m.webm", "--name", "c", "--workdir", str(tmp_path),
         "--events-out", str(out), "--at", "100", "--drop-instruments", "drums"]
    )  # fmt: skip
    assert rc == 0
    doc = json.loads(out.read_text(encoding="utf-8"))
    assert doc["source"] == "muscriptor-medium" and doc["events"][0]["onset_s"] == 101.0


def test_stream_slices_for_emu_vs_hw(tmp_path: Path) -> None:
    """A velocity-free stream slices through ``emu_events.segment_events`` unchanged."""
    import emu_events

    stream = tmp_path / "s.json"
    doc = me.to_doc(fixture(), "t", offset_s=100.0, drop=["drums"])
    stream.write_text(json.dumps(doc), encoding="utf-8")
    out = emu_events.segment_events(
        stream, {"at_s": 100.5, "duration_s": 1.0}, tmp_path / "seg.json"
    )
    ev = json.loads(out.read_text(encoding="utf-8"))["events"]
    assert [(e["midi"], e["onset_s"]) for e in ev] == [(60, 0.5)]
    assert "velocity" not in ev[0]
