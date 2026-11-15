"""Tests of the SHARC listing interpreter on synthetic programs (no firmware needed)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import sharc_text_vm as vm  # noqa: E402
import synth_wave_model as m  # noqa: E402


def machine(mem: dict[int, int] | None = None) -> tuple[vm.Vm, dict[int, int]]:
    store = dict(mem or {})
    return vm.Vm(lambda a: store.get(a, 0), store.__setitem__), store


def test_parse_listing_and_split_items() -> None:
    lines = vm.parse_listing("0x2484d2 15b   9408 0b60       I6 = DM(-32, I2)\ntiles exactly\n")
    assert lines == [(0x2484D2, "I6 = DM(-32, I2)")]
    assert vm.split_items("DM(-10, I4) = R9, F2 = MIN (F10, F12)") == [
        "DM(-10, I4) = R9",
        "F2 = MIN (F10, F12)",
    ]


def test_simd_load_reads_the_next_word_for_pey_and_post_modify_updates() -> None:
    v, _ = machine({0x100: 0x3F800000, 0x101: 0x40000000, 0x102: 0x40400000, 0x103: 0x40800000})
    v.i[2] = 0x100
    v.run([(0, "R0 = DM(I2, 2)"), (4, "R1 = DM(I2, 2)"), (8, "RTS")])
    assert (v.regs[0][0], v.regs[1][0]) == (0x3F800000, 0x40000000)
    assert (v.regs[0][1], v.regs[1][1]) == (0x40400000, 0x40800000)
    assert v.i[2] == 0x104


def test_float_ops_use_the_32_bit_register_model_and_stores_truncate() -> None:
    v, mem = machine({0x10: 0x3F800000, 0x11: 0x3F800000, 0x20: 0x40000000, 0x21: 0x40400000})
    v.i[0] = 0
    prog = [
        (0, "R0 = DM(16, I0)"),
        (4, "R1 = DM(32, I0)"),
        (8, "F2 = F0 * F1"),
        (12, "F3 = F2 + F0"),
        (16, "DM(48, I0) = R3"),
        (20, "RTS"),
    ]
    v.run(prog)
    assert mem[48] == m.st(m.add(m.mul(m.ld(0x3F800000), m.ld(0x40000000)), m.ld(0x3F800000)))
    assert mem[49] == m.st(m.add(m.mul(m.ld(0x3F800000), m.ld(0x40400000)), m.ld(0x3F800000)))


def test_fix_float_fext_fdep_ashift_and_integer_add() -> None:
    v, _ = machine()
    prog = [
        (0, "R0 = 0xfe000001"),
        (4, "R1 = FEXT R0 BY 25:7 (SE)"),
        (8, "R2 = FDEP R0 BY 6:25"),
        (12, "R3 = ASHIFT R1 BY 5"),
        (16, "R4 = R3 + R1"),
        (20, "R14 = 0xffffffe1"),
        (24, "F5 = FLOAT R2 BY R14"),
        (28, "R15 = 0x1f"),
        (32, "R6 = FIX F5 BY R15"),
        (36, "RTS"),
    ]
    v.run(prog)
    r = v.regs[0]
    assert m.s32(r[1]) == -1 and m.s32(r[3]) == -32 and m.s32(r[4]) == -33
    assert r[2] == 1 << 6
    assert r[5] == m.flt(1 << 6, -31) and r[6] == 1 << 6


def test_moves_exchange_and_cross_element_forms() -> None:
    v, _ = machine()
    v.regs[0][2], v.regs[1][2] = 11, 22
    v.regs[0][0], v.regs[1][1] = 5, 6
    v.run([(0, "R3 = S2"), (4, "R1 <-> S0"), (8, "RTS")])
    assert (v.regs[0][3], v.regs[1][3]) == (22, 11)
    assert (v.regs[0][1], v.regs[1][0]) == (0, 0)


def test_do_loop_runs_the_body_lcntr_times() -> None:
    v, _ = machine()
    v.regs[0][1] = v.regs[1][1] = 7
    prog = [
        (0, "R0 = 0"),
        (6, "DO (PC, 5) UNTIL LCE, LCNTR = 3"),
        (12, "R0 = R0 + R1"),
        (16, "R2 = R0"),
        (20, "RTS"),
    ]
    v.run(prog)
    assert v.regs[0][0] == 21 and v.regs[0][2] == 21 and v.regs[1][0] == 21


def test_pre_modify_load_through_i1_broadcasts_the_pex_word() -> None:
    v, _ = machine({0x100: 0x3F800000, 0x101: 0x40000000})
    v.i[1] = v.i[5] = 0x100
    v.run([(0, "R0 = DM(0, I1)"), (4, "R1 = DM(0, I5)"), (8, "RTS")])
    assert (v.regs[0][0], v.regs[1][0]) == (0x3F800000, 0x3F800000)
    assert (v.regs[0][1], v.regs[1][1]) == (0x3F800000, 0x40000000)
