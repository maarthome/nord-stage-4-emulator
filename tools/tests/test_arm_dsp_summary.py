"""Unit tests for tools/arm_dsp_summary.py (program class, word comparison, `e` check)."""

from __future__ import annotations

import struct
from pathlib import Path

import arm_dsp_capture as cap
import arm_dsp_summary as ads
import arm_program_routing as apr
import pytest


def row(phase: str, chip: str, dm: int, w2: int, path: int = 2, core: int = 1, tick: int = 0):
    return {"phase": phase, "tick": tick, "chip": chip, "path": path, "core": core,
            "dm": dm, "w2": w2, "pc": 0}  # fmt: skip


def test_unpack_pack_round_trip_on_measured_entries() -> None:
    # VERIFIED values of sinc::step_join tests (e = 0x0800 .. 0x1800).
    for e, ratio in ((0x0800, 0.625), (0x0A00, 0.78125), (0x1000, 1.25), (0x1800, 2.5)):
        assert ads.unpack_ratio(e) == pytest.approx(ratio)
        assert ads.pack_ratio(ratio) == e
    assert ads.unpack_ratio(0) == 0.0
    assert ads.pack_ratio(0.3125) == 0x0400
    assert ads.pack_ratio(1e9) == 0xF7FF


def test_modelled_entry_from_pitch_bits() -> None:
    bits = struct.unpack("<I", struct.pack("<f", 0.0625))[0]  # ratio 1.25
    assert ads.modelled_entry(bits) == 0x1000


def test_program_class_zero_payload_is_all_sample_and_disabled() -> None:
    cls = ads.program_class(bytes(820))
    assert cls["sample_layers"] == [True, True, True]
    assert cls["sample_scene_1"] is False and cls["sample_scene_2"] is False


def test_program_class_active_when_section_and_layer_on() -> None:
    payload = bytearray(820)
    # The 3-bit section enable field of scene I, MSB first; `(sections >> SECTION_BIT) & 1`.
    shift = apr.SECTION_BIT["synth"]
    bit = apr.GLOBAL * 8 + apr.SECTION_ENABLE_I + (2 - shift)
    payload[bit // 8] |= 0x80 >> (bit % 8)
    cls = ads.program_class(bytes(payload))
    assert cls["sample_scene_1"] is True  # mask 0 plays layer A, which is SAMPLES here
    assert cls["sample_scene_2"] is False


def test_phase_group_and_target_name() -> None:
    assert [ads.phase_group(p) for p in ("load", "idle", "k60", "k60_off", "bind")] == [
        "load", "idle", "key", "key_off", "other",
    ]  # fmt: skip
    assert ads.target_name(row("load", "ORG", 1, 1, core=2)) == "ORG c2"
    assert ads.target_name(row("load", "SMP", 1, 1, path=1, core=0)) == "SMP p1"


def test_is_bulk_flags_stores_outside_the_core_window() -> None:
    assert not ads.is_bulk(row("load", "ORG", 0xC1DCC, 1))
    assert ads.is_bulk(row("load", "ORG", -0x7F000, 1))
    assert ads.is_bulk(row("load", "PNO", 0x400000, 1))
    assert not ads.is_bulk(row("load", "SMP", -5, 1, path=1, core=0))  # path 1 is a DM word


def test_counts_and_final_words() -> None:
    rows = [
        row("load", "ORG", 5, 1), row("load", "ORG", 5, 2), row("load", "ORG", 6, 3),
        row("k36", "SY", 9, 4), row("idle", "PNO", 1, 1),
    ]  # fmt: skip
    counts = ads.counts_by_group(rows)
    assert counts["ORG c1"] == {"load": 3}
    assert counts["SY c1"] == {"key": 1}
    assert ads.final_words(rows)["ORG c1"] == {5: 2, 6: 3}


def test_chip_total_sums_all_targets_of_a_chip() -> None:
    counts = {"SY p1": {"key": 5, "load": 1}, "SY c1": {"key": 2}, "SMP p1": {"load": 9}}
    assert ads.chip_total(counts, "SY") == 8
    assert ads.chip_total(counts, "FX") == 0
    assert ads.chip_total({"SY p1": {"other": 4, "key": 1}}, "SY") == 1  # bind rows do not count


def test_slim_replaces_ranges_by_their_count() -> None:
    out = ads.slim(ads.compare_words([{1: 1, 2: 1}, {1: 2, 2: 1}]))
    assert "varying_ranges" not in out and out["varying_range_count"] == 1


def test_ranges() -> None:
    assert ads.ranges([5, 1, 2, 3, 9, 10, 3]) == [(1, 3), (5, 5), (9, 10)]
    assert ads.ranges([]) == []


def test_compare_words_classifies() -> None:
    per = [{1: 10, 2: 5, 3: 7}, {1: 10, 2: 6}, {1: 10, 2: 5}]
    out = ads.compare_words(per)
    assert out["distinct_words"] == 3
    assert out["in_every_program"] == 2
    assert out["constant"] == 1  # word 1
    assert out["varying"] == 1  # word 2
    assert out["varying_ranges"] == [(2, 2)]
    assert out["program_specific"] == 1  # word 3 only in one program


def test_page_histogram_counts_distinct_words_per_page() -> None:
    assert ads.page_histogram([0x1000, 0x1001, 0x1001, 0x2FFF, 0x3000], page=0x1000) == {
        "0x1000": 2,
        "0x2000": 1,
        "0x3000": 1,
    }
    assert ads.compare_words([{0x9354A: 1}])["pages"] == {"0x90000": 1}


def test_target_stats_min_median_max() -> None:
    counts = [{"A": {"load": 4}}, {"A": {"load": 10}}, {"B": {"load": 1}}]
    stats = ads.target_stats(counts, "load")
    assert stats["A"] == {"min": 0, "median": 4, "max": 10}
    assert stats["B"]["max"] == 1


def test_e_window_hits() -> None:
    rows = [
        row("k60", "SY", 0x9354A, 1),
        row("k60", "SY", 0x92414, 1),
        row("k60", "SMP", 0x935A0, 1),
    ]
    assert [hex(r["dm"]) for r in ads.e_window_hits(rows)] == ["0x9354a"]


def test_e_shaped_hits_match_model_only_in_key_phase() -> None:
    bits = struct.unpack("<I", struct.pack("<f", 0.0625))[0]  # model e = 0x1000
    rows = [
        row("k60", "SY", ads.PITCH_WORD_DM, bits),
        row("k60", "SY", 0x92500, 0x00001001),  # low half within slack
        row("k60", "SY", 0x92501, 0x10000000),  # high half 0x1000
        row("k60", "SY", 0x92502, 0x00002000),  # no
        row("k36", "SY", 0x92503, 0x00001000),  # phase k36 has no pitch word: skipped
    ]
    hits = ads.e_shaped_hits(rows)
    assert sorted(hex(h["dm"]) for h in hits) == ["0x92500", "0x92501"]
    assert all(h["model_e"] == 0x1000 for h in hits)


def test_sy_words_groups_by_phase() -> None:
    rows = [
        row("k60", "SY", 0x92414, 1), row("k60", "SY", 0x92414, 2),
        row("k84", "SY", 0x9243B, 3), row("k60", "ORG", 0x1, 9), row("load", "SY", 0x5, 1),
    ]  # fmt: skip
    out = ads.sy_words(rows)
    assert out["k60"] == {0x92414: [1, 2]}
    assert out["k84"] == {0x9243B: [3]}
    assert out["k36"] == {}


def test_key_word_table_flags_key_and_program_dependence() -> None:
    def prog(pitch: tuple[int, int, int], const: int, other: int) -> dict:
        return {
            ph: {0x921A6: [pitch[i]], 0x92414: [1, const], 0x92420: [other]}
            for i, ph in enumerate(ads.KEY_PHASES)
        }

    table = {t["dm"]: t for t in ads.key_word_table([prog((1, 2, 3), 7, 5), prog((1, 2, 4), 7, 6)])}
    assert table["0x921a6"]["varies_with_key"] and table["0x921a6"]["varies_with_program"]
    assert not table["0x92414"]["varies_with_key"] and not table["0x92414"]["varies_with_program"]
    assert table["0x92420"]["varies_with_program"] and not table["0x92420"]["varies_with_key"]
    assert table["0x92414"]["values"]["k36"] == ["0x7"]  # last value per program


def test_captured_lists_stems(tmp_path: Path) -> None:
    cap.write_rows(tmp_path / "Bank_A__One.txt.gz", [])
    cap.write_rows(tmp_path / "Bank_A__Two.txt.gz", [])
    assert list(ads.captured(tmp_path)) == ["Bank_A__One", "Bank_A__Two"]
