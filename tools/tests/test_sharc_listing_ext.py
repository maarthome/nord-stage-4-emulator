"""Unit tests for tools/sharc_listing_ext.py and tools/organ_replay.py (no oracle needed)."""

from __future__ import annotations

import organ_replay as rp
import sharc_listing_ext as ext
import sharc_listing_vm as vm


def run(text: str, setup=None) -> ext.GenVm:  # noqa: ANN001
    machine = ext.GenVm({})
    if setup:
        setup(machine)
    machine.run(vm.parse_listing(text + "\n00000040  0000  rts (db)\n"))
    return machine


def test_shift_field_and_wrap_helpers_follow_nord_sharc() -> None:
    assert ext.shift(0x80000001, 1, False) == 2
    assert ext.shift(0x80000000, -31, True) == 0xFFFFFFFF
    assert ext.shift(5, 40, True) == 0
    assert ext.fdep(0xFFFF, 7, 25, False) == 0xFE000000
    assert ext.fext(0xFE000000, 7, 25, False) == 0x7F
    assert ext.fext(0xFE000000, 7, 25, True) == 0xFFFFFFFF
    assert ext.wrap80(1 << 79) == -(1 << 79)
    assert ext.round_mr(0x8000_0000) == 0  # tie rounds to even
    assert ext.round_mr(0x1_8000_0000) == 0x2_0000_0000


def test_fixed_multiply_modes_and_the_per_pe_accumulator() -> None:
    def setup(m: ext.GenVm) -> None:
        m.r[0], m.r[2] = 0x4000_0000 << 8, 0x4000_0000 << 8
        m.s[0], m.s[2] = 3 << 8, 5 << 8

    m = run(
        "00000000  0000  MRF=r0*r2(ssf)\n00000003  0000  r4=MRF+r0*r2(ssf)\n"
        "00000006  0000  r6=r0*r2(ssi)",
        setup,
    )
    assert m.mr[0] == 0x2000_0000_0000_0000
    assert m.r[4] >> 8 == 0x4000_0000 and m.s[4] >> 8 == 0
    assert m.mr[1] == 30 and m.s[6] >> 8 == 15


def test_field_clip_float_conversion_and_the_per_pe_condition() -> None:
    def setup(m: ext.GenVm) -> None:
        m.r[1], m.s[1] = 0xFE000000 << 8, 0x0F << 8
        m.r[11] = m.s[11] = (7 << 6 | 25) << 8
        m.r[15] = m.s[15] = 0xFFFFFFE1 << 8  # scale -31
        m.r[3], m.s[3] = 0x12345 << 8, (-3 & 0xFFFFFFFF) << 8
        m.r[4] = m.s[4] = 100 << 8
        m.r[5], m.s[5] = 1 << 8, 9 << 8
        m.r[6] = m.s[6] = 5 << 8

    m = run(
        "00000000  0000  r2=fext r1 by r11\n00000003  0000  f7=float r4 by r15\n"
        "00000006  0000  r8=clip r3 by r4, r9=r3 and r4\n00000009  0000  r10=r4+1\n"
        "0000000c  0000  comp(r5,r6)\n0000000f  0000  if lt r5=r5+r6",
        setup,
    )
    assert m.r[2] >> 8 == 0x7F and m.s[2] >> 8 == 0
    assert m.r[7] == vm.f32_bits_of(100 * 2.0**-31) << 8
    assert (m.r[8] >> 8, m.s[8] >> 8) == (100, 0xFFFFFFFD)
    assert m.r[10] >> 8 == 101
    assert (m.r[5] >> 8, m.s[5] >> 8) == (6, 9)  # PEx: 1 < 5 so it adds, PEy: 9 < 5 is false


def test_long_word_store_writes_the_index_register_pair() -> None:
    def setup(m: ext.GenVm) -> None:
        m.i[12], m.i[13] = 0x111, 0x222

    m = run("00000000  0000  dm(0x1000)=i12 (LW)", setup)
    assert (m.mem[0x1000], m.mem[0x1001]) == (0x111, 0x222)


def test_replay_helpers() -> None:
    assert rp.parse_step("0x121bbe:0x121e28:I0=0xc00f2,I1=0xb52c7") == (
        0x121BBE,
        0x121E28,
        {"i0": 0xC00F2, "i1": 0xB52C7},
    )
    assert rp.diff({1: 5, 2: 0}, {1: 5, 3: 0}) == []
    assert rp.diff({1: 5}, {1: 6}) == [1]
    row = "122296 " + " ".join(f"{k:x}" for k in range(64))
    machine = rp.start_vm({0: 1}, row)
    assert machine.r[1] == 1 << 8 and machine.s[0] == 16 << 8 and machine.i[0] == 32
    rp.preset(machine, {"i0": 7})
    assert machine.i[0] == 7
