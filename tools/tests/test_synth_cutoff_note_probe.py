"""Unit tests for tools/synth_cutoff_note_probe.py (helpers, fake oracle, committed table)."""

from __future__ import annotations

import csv
import math
from pathlib import Path

import synth_adr_oracle as adr
import synth_cutoff_note_probe as p

DATA = Path(__file__).resolve().parents[2] / "docs/research/data/synth-cutoff-voice-law.csv"


def test_voice_word_matches_the_arm_key_word() -> None:
    # keydown-zones s7: note 21 -> byte 0xD9, key-down 0x0B, note 60 -> 0x00, layer B adds 0x20
    assert p.voice_word(21, g=0) == 0x0BD90000
    assert p.voice_word(60, g=0x80) == 0x0B008000
    assert p.voice_word(60, on=False, g=0) == 0x08000000
    assert p.voice_word(60, g=0, layer=1) >> 24 == 0x2B


def test_knob_word_is_the_arm_top_byte_law_and_the_steeper_table() -> None:
    assert p.knob_word(52) == 0
    assert p.knob_word(0) == 0xCC000000
    assert p.knob_word(117) == 0x41000000
    assert p.knob_word(127) == 0x6A000000  # from synth-filter-freq-law.csv


def test_natural_hz_and_semitone_offset() -> None:
    r = math.tan(math.pi * 140.0 / p.FRAME_RATE_HZ)
    assert abs(p.natural_hz(r) - 140.0) < 1e-6
    r2 = math.tan(math.pi * 280.0 / p.FRAME_RATE_HZ)
    assert abs(p.semitone_offset(r2, r) - 12.0) < 1e-9
    assert abs(p.table_index(0.36083)) < 1e-9


def test_probe_reads_r_of_the_last_pass(monkeypatch) -> None:  # noqa: ANN001
    seen = {}

    def fake(org, image, passes, **kw):  # noqa: ANN001, ANN003
        seen.update(kw, passes=passes)
        return {(passes - 1, 0): {1: adr.f2u(0.25)}}

    monkeypatch.setattr(adr, "run_script", fake)
    assert p.probe("o", "i", 52, 3, 72) == 0.25
    assert (0, 38, p.TRK_WORDS[3]) in seen["msgs"]
    assert seen["pre"] == {adr.CONTROLLER: p.voice_word(72)}
    assert seen["passes"] == p.PASSES
    p.probe("o", "i", 52, 0, None)
    assert seen["pre"] == {}


def test_committed_oracle_sweep_is_exponential_and_two_times_the_display() -> None:
    """Oracle result (sharc-org, voice word of note 60, key tracking 0), no oracle needed here."""
    rows = list(csv.DictReader(DATA.read_text().splitlines()))
    assert len(rows) == 128
    theta = [math.atan(float(r["R"])) for r in rows]
    for k in range(118):
        assert abs(12 * math.log2(theta[k] / theta[52]) - (k - 52)) < 0.01, k
    ratio = [float(r["display_over_natural"]) for r in rows[8:117]]
    assert 1.96 < sum(ratio) / len(ratio) < 1.98
    assert abs(float(rows[52]["natural_hz"]) - 140.6258) < 1e-3
