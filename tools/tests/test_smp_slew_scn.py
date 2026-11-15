"""Unit tests for tools/smp_slew_scn.py (scenario text only, no firmware needed)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import smp_slew_scn as s  # noqa: E402


def test_field_addresses_follow_the_record_geometry() -> None:
    assert s.addr(0, 0) == 0x92848
    assert s.addr(1, -32) == 0x92848 + 54 - 32
    assert s.addr(113, 21) == 0x92848 + 54 * 113 + 21
    assert s.fbits(1.0) == 0x3F800000
    assert s.fbits(0.5) == 0x3F000000


def test_random_scenario_pokes_records_and_runs_one_interrupt() -> None:
    text = s.random_scenario(10, 12, seed=1)
    lines = text.splitlines()
    assert lines[0] == "meta rate 48000"
    assert sum(1 for x in lines if x == "irq 17") == 1
    assert text.count("poke 0x") == 3 + 2 * (len(s.FLOAT_FIELDS) + 6)
    assert text == s.random_scenario(10, 12, seed=1)
    assert text != s.random_scenario(10, 12, seed=2)
    assert lines.index("irq 17") > max(i for i, x in enumerate(lines) if x.startswith("poke"))


def test_trajectory_scenario_loops_two_interrupts_per_update() -> None:
    lines = s.trajectory_scenario().splitlines()
    i = lines.index("loop 60")
    assert lines[i + 1 : i + 3] == ["irq 17", "irq 17"]
    assert lines[-1] == "end"
    assert any(x.startswith("capture f0_lo ") for x in lines)
    assert "poke 0x%X 0x7FFFFFFF" % s.addr(20, 19) in lines


def test_edges_scenario_sets_the_loop_words_and_captures_three_interrupts() -> None:
    text = s.edges_scenario(130)
    assert text.count("irq 17") == 3
    assert "poke 0x%X 0x64" % s.addr(0, -8) in text
    assert "poke 0x%X 0x82" % s.addr(0, -30) in text
    assert "poke 0x%X 0x83" % s.addr(1, -30) in text


def test_main_writes_the_file(tmp_path: Path) -> None:
    out = tmp_path / "t.scn"
    assert s.main(["trajectory", str(out)]) == 0
    assert out.read_text(encoding="utf-8").startswith("meta rate")
