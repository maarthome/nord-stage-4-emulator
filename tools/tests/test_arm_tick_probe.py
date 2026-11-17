"""Unit tests for tools/arm_tick_probe.py (pure helpers; the Unicorn runs need local firmware)."""

from __future__ import annotations

import arm_tick_probe as tp
import pytest


def test_prd_hz_divides_the_24_mhz_clock() -> None:
    assert tp.prd_hz(1000) == pytest.approx(24_000.0)  # PRD34 the firmware writes (0x3E8)
    assert tp.prd_hz(250) == pytest.approx(96_000.0)  # PRD12 (0xFA)
    assert tp.prd_hz(0) == 0.0


def test_the_set_up_routine_arithmetic_gives_the_prd() -> None:
    # `0xC005019C(dev, 1, 0x5DC0)`: PRD = 0x016E3600 / 24000 = 1000, an interrupt rate in Hz.
    assert 0x016E3600 // 0x5DC0 == 1000
    assert tp.TIMER_CLOCK_HZ == 0x016E3600


def test_decode_tgcr_of_the_final_write() -> None:
    f = tp.decode_tgcr(0x7)
    assert f["timmode"] == "dual 32-bit unchained"
    assert (f["tim12_released"], f["tim34_released"]) == (1, 1)
    assert tp.decode_tgcr(0x6)["tim12_released"] == 0


def test_decode_tcr_of_the_final_write() -> None:
    f = tp.decode_tcr(0x800080)
    assert f == {"enamode12": "continuous", "enamode34": "continuous"}
    assert tp.decode_tcr(0x800000)["enamode12"] == "disabled"


def test_timer_summary_takes_the_last_write_of_each_register() -> None:
    writes = [
        (0xC0050298, 0x1C, 0x3E8),
        (0xC0069518, 0x18, 0xFA),
        (0xC0024000, 0x20, 0x800000),
        (0xC0024004, 0x20, 0x800080),
        (0xC0050208, 0x24, 0x6),
        (0xC0050209, 0x24, 0x7),
    ]
    s = tp.timer_summary(writes)
    assert s["TIM34"]["hz"] == pytest.approx(24_000.0)  # type: ignore[index]
    assert s["TIM12"]["hz"] == pytest.approx(96_000.0)  # type: ignore[index]
    assert s["TIM34"]["period_us"] == pytest.approx(1e6 / 24_000)  # type: ignore[index]
    assert s["tcr"]["enamode12"] == "continuous"  # type: ignore[index]
    assert s["tgcr"]["tim12_released"] == 1  # type: ignore[index]


def test_wheel_rates_one_slot_per_tick() -> None:
    slots = [tp.DEFAULT_ENTRY] * 512
    for i in range(2, 512, 4):
        slots[i] = 0xC009B8A8  # every 4th slot
    for i in range(4, 512, 8):
        slots[i] = 0xC009B864  # every 8th slot
    rows = tp.wheel_rates(slots, 24_000.0)
    by_fn = {r["fn"]: r for r in rows}
    assert by_fn[0xC009B8A8]["hz"] == pytest.approx(6000.0)
    assert by_fn[0xC009B8A8]["gaps"] == [4]
    assert by_fn[0xC009B864]["hz"] == pytest.approx(3000.0)
    assert by_fn[0xC009B864]["slots"] == 64
    assert by_fn[tp.DEFAULT_ENTRY]["idle"] is True
    assert rows[-1]["fn"] == tp.DEFAULT_ENTRY  # idle slots sort last


def test_periodic_rows_convert_ticks_to_hz() -> None:
    # words: id, _, period, _, due, _, object, fn, mode, _
    words = [
        [8, 0, 0x78, 0, 0, 0, 0x90039D30, 0xC009F13C, 1, 0],
        [0x13, 0, 0xF0, 0, 0, 0, 0x9115D2B0, 0xC02090F4, 1, 0],
        [2, 0, 0x5DC0, 0, 0, 0, 0x90168A80, 0xC004C8AC, 1, 0],
    ]
    rows = tp.periodic_rows(words, 24_000.0)
    assert [r["hz"] for r in rows] == pytest.approx([200.0, 100.0, 1.0])
    assert rows[1]["period_ms"] == pytest.approx(10.0)  # the Leslie rotor update (rotor_arm.rs)
    assert rows[1]["fn"] == 0xC02090F4


def test_calls_per_second() -> None:
    assert tp.calls_per_second(100, 24_000, 24_000.0) == pytest.approx(100.0)
    assert tp.calls_per_second(5, 0, 24_000.0) == 0.0


def test_harness_scale_is_sixteen_times_the_real_budget() -> None:
    s = tp.harness_scale(200_000, 24_000.0)
    assert s["real_insns_per_tick"] == pytest.approx(12_500.0)
    assert s["harness_over_real"] == pytest.approx(16.0)
    assert tp.harness_scale(200_000, 1_000.0)["harness_over_real"] == pytest.approx(
        200_000 / 300_000
    )


def test_message_ticks_are_the_distinct_ticks_with_messages() -> None:
    assert tp.message_ticks([(0, 0), (3, 2), (3, 1), (17, 4), (18, 0)]) == [3, 17]


def test_delay_model_timer_driven_has_constant_ticks() -> None:
    assert tp.delay_model([200_000, 100_000, 50_000], [3, 3, 3]) == "timer"


def test_delay_model_instruction_driven_has_constant_instructions() -> None:
    assert tp.delay_model([200_000, 100_000, 50_000], [3, 6, 12]) == "instructions"


def test_delay_model_mixed() -> None:
    assert tp.delay_model([200_000, 100_000, 50_000], [3, 4, 6]) == "mixed"


def test_pump_increment_is_exact_at_the_samples_and_linear_between() -> None:
    assert tp.pump_increment(63) == 0xFF200
    assert tp.pump_increment(79) == 0x13F200
    assert tp.pump_increment(71) == pytest.approx((0xFF200 + 0x13F200) / 2)
    assert tp.pump_increment(500) == 0x1FF200  # clamped to 127


def test_pump_free_run_reproduces_6_11_hz_and_the_beat_is_2_1_hz() -> None:
    # The free-run figure of the `--firmware-fx` render of A:25 (synth-super-lp24-clips.md s3b)
    # at rate value 73 (0.575 x 127); the hardware modulates at the beat, 126 BPM = 2.10 Hz.
    assert tp.pump_free_run_hz(73) == pytest.approx(6.11, abs=0.01)
    assert 126 / 60 == pytest.approx(2.10)
    assert tp.pump_free_run_hz(73) / (126 / 60) == pytest.approx(2.91, abs=0.01)


def test_pump_free_run_scales_with_frames_per_call() -> None:
    assert tp.pump_free_run_hz(73, frames_per_call=4) == pytest.approx(tp.pump_free_run_hz(73) / 2)


def test_spacing_stats() -> None:
    s = tp.spacing_stats([0, 10, 20, 35])
    assert (s["count"], s["min_gap"], s["max_gap"]) == (4.0, 10.0, 15.0)
    assert s["mean_gap"] == pytest.approx(35 / 3)
    assert tp.spacing_stats([5])["mean_gap"] == 0.0


def test_find_stores_decodes_str_immediate_offset() -> None:
    # str r1, [r5, #0xc] = e585100c ; str r2, [r0, #0xc] = e580200c ; ldr r1, [r5, #0xc] = e595100c
    # str r1, [r5, #0x10] = e5851010 ; strb r3, [r4, #0xc] = e5c4300c
    words = [0xE585100C, 0xE580200C, 0xE595100C, 0xE5851010, 0xE5C4300C]
    code = b"".join(w.to_bytes(4, "little") for w in words)
    rows = tp.find_stores(code, 0xC0000000, 0xC0000000, 0xC0000000 + len(code), 0xC)
    assert rows == [
        (0xC0000000, "str r1, [r5, #0xc]"),
        (0xC0000004, "str r2, [r0, #0xc]"),
        (0xC0000010, "strb r3, [r4, #0xc]"),
    ]
    only_r5 = tp.find_stores(code, 0xC0000000, 0xC0000000, 0xC0000014, 0xC, reg_filter=5)
    assert only_r5 == [(0xC0000000, "str r1, [r5, #0xc]")]
