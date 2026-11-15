from __future__ import annotations

import ns4_arp_player as p

STRAIGHT = [k * (1 << 18) for k in range(8)]
SHUFFLE = [0, 0x55555, 0x80000, 0xD5555, 0x100000, 0x155555, 0x180000, 0x1D5555]


def test_cycle_is_four_firmware_beats_and_grid_spans_it():
    assert p.CYCLE == 4 * (1 << 19)
    assert len(STRAIGHT) * (1 << 18) == p.CYCLE


def test_slot_events_play_one_record_per_grid_point_and_skip_the_start():
    events = p.slot_events(STRAIGHT, slots=4, cycles=1)
    # The point at phase 0 of the first cycle is never crossed.
    assert events[0] == (1 << 18, 1)
    assert [slot for _phase, slot in events] == [1, 2, 3, 0, 1, 2, 3]


def test_slot_index_continues_across_cycles_for_patterns_of_any_length():
    # 3 steps = 6 records over 8 points per cycle: the second cycle starts at record 8 % 6 = 2.
    events = p.slot_events(STRAIGHT, slots=6, cycles=2)
    second = [slot for phase, slot in events if phase >= p.CYCLE]
    assert second[:3] == [2, 3, 4]


def test_modelled_events_for_a_short_note_are_on_at_even_and_off_at_odd_points():
    # E E = records 0 3 0 3 (documented compiler output).
    types = [0, 3, 0, 3]
    events = p.modelled_events(types, STRAIGHT, cycles=1)
    assert events[:3] == [(1 << 18, "off"), (2 << 18, "on"), (3 << 18, "off")]


def test_modelled_events_type_one_sends_the_pseudo_off_first():
    # q E = 0 2 1 3: the second note starts without a note-off record before it.
    events = p.modelled_events([0, 2, 1, 3], STRAIGHT, cycles=1)
    assert [(ph >> 18, kind) for ph, kind in events if 2 <= ph >> 18 <= 3] == [
        (2, "off"),
        (2, "on"),
        (3, "off"),
    ]


def test_modelled_events_hold_records_send_nothing():
    # Only record 0 (a note-on) is played, at grid point 4; the three hold records are silent.
    events = p.modelled_events([0, 2, 2, 2], STRAIGHT, cycles=1)
    assert events == [(4 << 18, "on")]


def test_step_is_two_grid_points_and_a_firmware_beat():
    # R = 2400 (dial 127): 60 / 2400 s per firmware beat = one step = 25 ms.
    assert p.step_seconds(2400) == 0.025
    # R = 240 (dial 54, 120 BPM): a step is a 1/8 note, 250 ms; a grid point (a slot) is 125 ms.
    assert abs(p.step_seconds(240) - 0.25) < 1e-12
    assert abs(p.phase_seconds(1 << 18, 240) - 0.125) < 1e-12


def test_step_seconds_is_thirty_over_the_displayed_bpm():
    for table_bpm in (32, 240, 900, 2400):
        assert abs(p.step_seconds(table_bpm) - 30.0 / (table_bpm / 2)) < 1e-12


def test_shuffle_grid_puts_the_second_record_at_two_thirds_of_a_step():
    step = 1 << 19
    assert SHUFFLE[1] / step == 0x55555 / step
    assert abs(SHUFFLE[1] / step - 2 / 3) < 1e-5
    assert SHUFFLE[2] == step


def test_check_player_accepts_matching_events_and_reports_others(monkeypatch):
    types = [0, 3]
    want = p.modelled_events(types, STRAIGHT, cycles=1)
    good = [(phase + 5, kind, 0) for phase, kind in want]
    monkeypatch.setattr(p, "replay", lambda *_a, **_k: (types, good))
    assert p.check_player(object(), 0, STRAIGHT, "E", cycles=1) == []
    late = [(good[0][0] + 0x20000, good[0][1], 0), *good[1:]]
    monkeypatch.setattr(p, "replay", lambda *_a, **_k: (types, late))
    (problem,) = p.check_player(object(), 0, STRAIGHT, "E", cycles=1)
    assert "call phase" in problem
    monkeypatch.setattr(p, "replay", lambda *_a, **_k: (types, [(0x40000, "on", 0)]))
    (problem,) = p.check_player(object(), 0, STRAIGHT, "E", cycles=1)
    assert "event kinds differ" in problem
