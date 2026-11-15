"""Tests for tools/transcribe_agree.py."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import transcribe_agree as ta  # noqa: E402


def n(midi: int, t: float, **kw):
    return {"midi": midi, "onset_s": t, **kw}


def test_match_exact_octave_and_misses():
    truth = [n(60, 1.0), n(64, 2.0), n(67, 3.0), n(72, 4.0)]
    est = [n(60, 1.03), n(52, 2.0), n(67, 3.2), n(72, 4.0), n(72, 4.01)]
    r = ta.match_notes(truth, est)
    assert r["matched_exact"] == 2  # 60 and the first 72; 67 is 0.2 s late
    assert r["matched_octave"] == 1  # 52 vs 64: pitch class E
    assert r["recall"] == 0.5 and r["recall_pitch_class"] == 0.75
    assert r["precision_lower_bound"] == 0.4


def test_empty_inputs():
    r = ta.match_notes([], [])
    assert r["recall"] is None and r["precision_lower_bound"] is None


def test_agreement_and_cli(tmp_path, capsys):
    vid = tmp_path / "v.json"
    vid.write_text(
        json.dumps({"events": [n(60, 1.0, confirmed=True), n(62, 2.0, confirmed=False)]})
    )
    stream = tmp_path / "s.json"
    stream.write_text(json.dumps({"events": [n(60, 11.02), n(99, 50.0)]}))
    assert ta.main([str(vid), str(stream), "--at", "10", "--duration", "5"]) == 0
    out = json.loads(capsys.readouterr().out)
    assert out["video_confirmed"] == 1 and out["video_detected"] == 2
    assert out["estimated"] == 1 and out["recall"] == 1.0
