"""Unit tests for tools/arm_dsp_live.py (the pure parts; the firmware run needs local files)."""

from __future__ import annotations

import io

import arm_dsp_link as dl
import arm_dsp_live as live
import arm_emu as ae


class FakeEmu:
    """Only what `Feeder` touches: `pad` and the list of port stores."""

    def __init__(self) -> None:
        self.pad = (1, 1, 1, 0, 0)
        self.port_writes: list[tuple[int, int, int]] = []


class Proc:
    def __init__(self) -> None:
        self.stdin = io.StringIO()
        self.stdout = io.StringIO("READY\n")


def test_feeder_sends_only_the_chosen_chip_and_counts_the_rest() -> None:
    emu, link = FakeEmu(), dl.Link(Proc())
    f = live.Feeder(emu, link, "PNO", 1)
    pno, org = ae.PORT2_BASE + 0x10, ae.PORT2_BASE + 0x20  # comm 1 = PNO, comm 2 = ORG
    # PNO core 1: byte address 0x28000000 + 4 * 0xB0200, pad byte 0, value 0x1234
    emu.port_writes += [(pno, 0x28000000 + 4 * 0xB0200, 1), (pno, 0, 2), (pno, 0x1234, 3)]
    # PNO core 2 (0x28800000 base) and an ORG message: counted, not sent
    emu.port_writes += [(pno, 0x28800000 + 4 * 5, 4), (pno, 0, 5), (pno, 9, 6)]
    emu.port_writes += [(org, 0x28000000, 7), (org, 0, 8), (org, 1, 9)]
    emu.port_writes += [(pno, 0x28000000 + 4 * 6, 10)]  # partial group waits
    f.feed()
    assert link.log == [(0, 0xB0200, 0x1234)]
    assert f.seen == {"PNO1p2": 1, "PNO2p2": 1, "ORG1p2": 1}
    assert emu.port_writes == []
    emu.port_writes += [(pno, 0, 11), (pno, 0x55, 12)]
    f.feed()
    assert link.log[-1] == (0, 6, 0x55) and f.seen["PNO1p2"] == 2


def test_phase_delta_subtracts_the_additive_numbers_and_the_probe_hits() -> None:
    a = {"messages_sent": 5, "distinct_words": 4, "passes": 10, "idle_passes": 10,
         "probe_hits": {"0x1256ad": 1}}  # fmt: skip
    b = {"messages_sent": 9, "distinct_words": 6, "passes": 30, "idle_passes": 29,
         "probe_hits": {"0x1256ad": 4, "0x121e2b": 20}}  # fmt: skip
    d = live.phase_delta(a, b)
    assert d == {"messages_sent": 4, "distinct_words": 2, "passes": 20, "idle_passes": 19,
                 "probe_hits": {"0x1256ad": 3, "0x121e2b": 20}}  # fmt: skip


def test_snapshot_reports_link_stats() -> None:
    link = dl.Link(Proc())
    link.stats.add(dl.PassReport(idle=2, cnt=4, sumsq=1.0, peak=0.5, hits={0x1256AD: 1}), 0)
    link.send(3, 4)
    s = live.snapshot(link, 0.0)
    assert s["messages_sent"] == 1 and s["idle_passes"] == 2 and s["lane_peak"] == 0.5
    assert s["probe_hits"] == {"0x1256ad": 1} and s["probe_first_pass"] == {"0x1256ad": 0}
    assert s["lane_rms"] == 0.5


def test_feeder_skips_words_outside_the_core_window() -> None:
    emu, link = FakeEmu(), dl.Link(Proc())
    f = live.Feeder(emu, link, "PNO", 1)
    pno = ae.PORT2_BASE + 0x10
    # byte address below 0x28000000: dm_word would be negative (core 1 by the 0x28800000 split)
    emu.port_writes += [(pno, 0x27000000, 1), (pno, 0, 2), (pno, 5, 3)]
    f.feed()
    assert link.log == [] and f.skipped == 1


def test_return_observer_mirrors_messages_into_the_source() -> None:
    import arm_dsp_return as rt

    src = rt.ReturnSource()
    obs = live.return_observer(src)
    obs(ae.Message(1, 3, 0x92414, rt.STATE_PREPARE))  # SY path 1 state message
    obs(ae.Message(2, 1, 0x28800000 + 4 * 7, 99))  # PNO core 2 word 7
    assert src.read(3, rt.IDLE_MASK, 1) == [0xFFFFFFFE]
    assert src.word(1, 2, 7) == 99
    assert live.return_observer(None) is None


def test_return_report_counts_reads_layers_and_busy_voices() -> None:
    import arm_dsp_return as rt

    src, log = rt.ReturnSource(), rt.ReadLog()
    src.observe(3, 0, 1, 0x92414, rt.STATE_GO)
    src.read(3, rt.IDLE_MASK, 2)
    rep = live.return_report(src, log)
    assert rep["reads"] == {"SY:0:0x92196": 1, "SY:0:0x92197": 1}
    assert rep["answered_by"] == {"sy_mask": 2} and rep["busy_voices"] == 1
