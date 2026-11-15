"""Tests for `tools/arm_smp_slew_init.py` (no firmware or emulator needed)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import arm_smp_slew_init as m  # noqa: E402


def test_decode_matches_the_record_layout():
    # VERIFIED against the ARM note-on list: DM 0x93927 is F-1 of record 80, 0x9393B is F+19.
    assert m.decode(0x93927) == (80, -1)
    assert m.decode(0x9393B) == (80, 19)
    assert m.decode(0x9395D) == (81, -1)
    assert m.decode(0x92848) == (0, 0)
    assert m.decode(0x92526) is None
    assert m.decode(0xC0500) is None


def test_addr_is_the_inverse_of_decode():
    for k in (0, 17, 81, 113):
        for f in m.FIELDS:
            assert m.decode(m.addr(k, f)) == (k, f)


def test_replay_scenario_batches_pairs_and_captures_every_field():
    msgs = [[3, 1, 0x90000000, 5]] + [[3, 1, m.addr(81, 3), i] for i in range(90)] + [[3, 2, 1, 1]]
    text = m.replay_scenario(msgs, [81], 12)
    lines = text.splitlines()
    assert lines.count("irq 13") == 1 + 3  # header reset + one per batch of 40 (91 pairs)
    assert lines[len(m.HEADER)] == "irq 13"
    assert "msg op=9 0x0 0x5" in lines
    assert sum(1 for line in lines if line.startswith("msg ")) == 91  # path 2 is skipped
    assert len([line for line in lines if line.startswith("capture ")]) == 2 * len(m.FIELDS)
    assert "loop 12" in lines and lines[-1] == "end"


def test_read_words_joins_halves_and_slew_updates_takes_every_second_row(tmp_path):
    csv_path = tmp_path / "x.csv"
    csv_path.write_text(
        "# comment\nindex,r81_f0_lo,r81_f0_hi\n0,0xde,3\n1,1,3\n2,2,-1\n".replace("0xde", "222")
    )
    words = m.read_words(csv_path, 81, 0)
    assert words == [(3 << 16) | 222, (3 << 16) | 1, 0xFFFF0002]
    assert m.slew_updates(words) == [(3 << 16) | 222, 0xFFFF0002 - (1 << 32)]
