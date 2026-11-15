"""Unit tests for tools/sharc_listing_vm.py and tools/organ_listing_check.py (no oracle needed)."""

from __future__ import annotations

import organ_listing_check as oc
import sharc_listing_vm as vm


def f(x: float) -> int:
    """40-bit pattern of an f32 value (extension byte zero)."""
    return vm.f32_bits_of(x) << 8


def test_f40_arithmetic_matches_single_precision_when_the_result_fits() -> None:
    for a, b in [(1.5, 2.25), (-3.0, 0.5), (0.125, 0.375)]:
        assert vm.f40_add(f(a), f(b)) == f(a + b)
        assert vm.f40_add(f(a), f(b), sub=True) == f(a - b)
        assert vm.f40_mul(f(a), f(b)) == f(a * b)


def test_f40_keeps_eight_extension_bits_like_the_oracle() -> None:
    # nord-sharc `extended_precision_keeps_eight_more_bits_than_single`: 1 + 2^-24 stays in the
    # extension byte, a single precision add would round it away
    tiny = f(2.0**-24)
    assert vm.f40_add(f(1.0), tiny) == f(1.0) | 0x80
    assert vm.f40_pass(f(1.0) | 0x80) == f(1.0) | 0x80


def test_f40_special_values() -> None:
    assert vm.f40_add(f(2.5), f(2.5), sub=True) == 0
    assert vm.f40_add(f(-0.0), f(-0.0)) == 1 << 39
    assert vm.f40_mul(f(0.0), f(-1.0)) == 1 << 39
    assert vm.f40_pass(f(1e-39)) == 0  # a denormal reads as zero
    assert vm.f40_mul(f(1e-30), f(1e-30)) == 0  # underflow flushes


def test_listing_parser_splits_items_outside_parentheses() -> None:
    text = "00000000  a212 ffff ffe0  f13=f1*f6, f10=f10+f13, r2=dm(i4,m2), r5=pm(i10,m10)\n"
    (ins,) = vm.parse_listing(text)
    assert ins.addr == 0
    assert ins.items == ["f13=f1*f6", "f10=f10+f13", "r2=dm(i4,m2)", "r5=pm(i10,m10)"]
    assert vm.split_items("dm(m3,i6)=r1") == ["dm(m3,i6)=r1"]


def run_text(text: str, mem: dict[int, int], setup=None) -> vm.Vm:  # noqa: ANN001
    machine = vm.Vm(mem)
    if setup:
        setup(machine)
    machine.run(vm.parse_listing(text))
    return machine


def test_simd_load_moves_the_pey_twin_from_the_next_word_and_post_modifies() -> None:
    def setup(m: vm.Vm) -> None:
        m.i[4], m.m[2] = 0x100, 2

    m = run_text(
        "00000000  0000  r2=dm(i4,m2)\n00000003  0000  rts (db)\n", {0x100: 7, 0x101: 9}, setup
    )
    assert (m.r[2] >> 8, m.s[2] >> 8, m.i[4]) == (7, 9, 0x102)


def test_a_load_through_i1_broadcasts_the_same_word_to_both_elements() -> None:
    def setup(m: vm.Vm) -> None:
        m.i[1] = 0x200

    m = run_text(
        "00000000  0000  r3=dm(0xb,i1)\n00000003  0000  rts\n", {0x20B: 5, 0x20C: 6}, setup
    )
    assert (m.r[3] >> 8, m.s[3] >> 8) == (5, 5)


def test_all_items_of_an_instruction_read_the_old_registers() -> None:
    def setup(m: vm.Vm) -> None:
        m.r[1], m.r[2] = f(2.0), f(3.0)
        m.s[1], m.s[2] = f(4.0), f(5.0)

    m = run_text("00000000  0000  f1=f1*f2, f2=f1+f2\n00000003  0000  rts\n", {}, setup)
    assert (m.r[1], m.r[2]) == (f(6.0), f(5.0))
    assert (m.s[1], m.s[2]) == (f(20.0), f(9.0))


def test_swap_exchanges_one_register_pair_and_keeps_the_rest() -> None:
    def setup(m: vm.Vm) -> None:
        m.r[2], m.s[1], m.r[1], m.s[2] = 11 << 8, 22 << 8, 33 << 8, 44 << 8

    m = run_text("00000000  0000  r2<->s1\n00000003  0000  rts\n", {}, setup)
    assert (m.r[2] >> 8, m.s[1] >> 8, m.r[1] >> 8, m.s[2] >> 8) == (22, 11, 33, 44)


def test_hardware_loop_runs_the_body_count_times() -> None:
    text = (
        "00000000  0000  lcntr=0x3, do (pc,0xff800003) until lce\n"
        "00000003  0000  r0=r0+r1\n"
        "00000006  0000  rts\n"
    )

    def setup(m: vm.Vm) -> None:
        m.r[1] = 5 << 8

    assert run_text(text, {}, setup).r[0] >> 8 == 15


def test_circular_buffer_wraps_a_post_modify() -> None:
    def setup(m: vm.Vm) -> None:
        m.i[6], m.b[6], m.l[6], m.m[2] = 0x13F, 0x120, 0x21, 1

    m = run_text("00000000  0000  r0=dm(i6,m2)\n00000003  0000  rts\n", {0x13F: 1}, setup)
    assert m.i[6] == 0x120 + (0x13F + 1 - 0x120) % 0x21


def test_trace_rows_parse_and_check_flags_a_wrong_register() -> None:
    row = "122296 " + " ".join(["0"] * 64)
    pc, regs = vm.parse_trace_line(row)
    assert pc == 0x122296 and len(regs["m"]) == 16
    prog = vm.parse_listing(
        "00000000  0000  i2=dm(0,i1)\n00000003  0000  r1=0x5\n00000006  0000  rts\n"
    )
    bad_row = "122299 " + " ".join(["0"] * 64)  # r1 should be 5 after the instruction
    assert vm.check({}, prog, [row, bad_row], 0x122296) == 2  # R1 and S1 differ
    ok = ["0"] * 64
    ok[1] = "5"
    ok[17] = "5"  # the immediate load broadcasts to S1
    assert vm.check({}, prog, [row, "122299 " + " ".join(ok)], 0x122296) == 0


def test_dump_reader_parses_oracle_output() -> None:
    mem = oc.read_dump("calls 1, steps per call 4\nDM(0x10..): 1 ff\nnoise\n")
    assert mem == {0x10: 1, 0x11: 0xFF}
