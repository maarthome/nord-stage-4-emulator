#!/usr/bin/env python3
"""Equation models of the FM stages `0x124269` (type 49..53 first stage) and `0x12453F`.

Listings `0x2484D2..0x248A7E` and `0x248A7E..0x248F6C` (ORG core 2, [fm-banks-1-2](../docs/research/fm-banks-1-2.md)).
The models run on a word memory (`rd(addr)`, `wr(addr, word)`) with the DAG base registers of the
stage and are checked word for word against the listing interpreter (`tools/sharc_text_vm.py`, itself
bit exact against `sharc-org`): `tools/tests/test_synth_stage_fm.py`.

SIMD: a data load or store moves `addr + lane`; a lane pair is `(PEx, PEy)`. The stage ends with a
cross-lane mix (`R12 = S12`), written as `swap`.

Stage 1, per lane `l` (`I0` constants, `I1` ARM words, `I2` record, `I3` accumulator table, `I4` scratch):

    inc1 = FIX(P0 P[-32](I6), M[I0-10]);  inc2 = FIX(P0 P[-30], ...)       stored I2-25, I2-23
    a1 = inc1 2^-31, a2 = inc2 2^-31;  idx = sext7(W >> 25) + M[I1-14];  frac = (W & 0x1FFFFFF) / 2^25 (FDEP R0 BY 6:25: 25 bits at bit 6)
    A0, A1 = lerp of band curves T0, T1 at idx (lane 0, lane 1),  B0 = lerp of T2
    g2 = (clip(0.6914 - e1 a1, 0.0576) + 0.0576) 2.17,  F2 = A0 g2,  G = g3 w19
    mA = clip(e22 x^2 + e25 .. , A1) ...
    two iterations (F4 = a1, then a2) of four operators, each four samples:
        op A: phase = accA + k incA                       -> s
        op B: phase = accB + k inc2' + (FIX(sA mA, 26) << 5)   -> sB G0
        op C: phase = accC + k inc1'                      -> s
        op D: phase = accD + k inc3' + (FIX(sC mC, 26) << 5)   -> sD G1 + sB G0
        mix: buf_k = (D_k w0 + buf_k) + swap(D_k w1)
    buf_k *= P[16]

`sine_of_word` is the verified helper (`t = |w 2^-30| - 1`, Horner in `t^2`, times `t`).
"""

from __future__ import annotations

from collections.abc import Callable

import synth_stage_shape_sine as ss
import synth_wave_model as m

Rd = Callable[[int], int]
Wr = Callable[[int, int], None]

# VERIFIED (listing `0x248578`..: `R10 = 0x3f30f34b`, `R11 = 0x3e6bef0f`, `R7 = 0x400ae2eb`; model
# equal to the interpreter, `tools/tests/test_synth_stage_fm.py`): fade constants of the fades.
FADE_C0 = m.ld(0x3F30F34B)
FADE_C1 = m.ld(0x3E6BEF0F)
FADE_C2 = m.ld(0x400AE2EB)
# VERIFIED (listing `0x24864A`: `R12 = 0x3f7fbe77`): increment ceiling (0.99874).
INC_CEIL = m.ld(0x3F7FBE77)
I4_BASE = 0xC1EC2  # VERIFIED (listing `0x2484F8`): scratch block pointer


class Mem:
    """Word memory with a write overlay, so a read sees the stage's own earlier stores."""

    def __init__(self, rd: Rd, wr: Wr) -> None:
        self.rd0, self.wr0, self.ov = rd, wr, {}

    def rd(self, a: int) -> int:
        a &= 0xFFFFFFFF
        return self.ov[a] if a in self.ov else self.rd0(a)

    def wr(self, a: int, w: int) -> None:
        a &= 0xFFFFFFFF
        self.ov[a] = w & 0xFFFFFFFF
        self.wr0(a, w & 0xFFFFFFFF)

    def lanes(self, a: int) -> list[int]:
        return [self.rd(a), self.rd(a + 1)]

    def fl(self, a: int) -> list[m.X]:
        return [m.ld(w) for w in self.lanes(a)]

    def bc(self, a: int) -> list[m.X]:
        """Pre-modify load through `I1` / `I9`: both elements get the PEx word (MODE1 BDCST1 / 9)."""
        x = m.ld(self.rd(a))
        return [x, x]

    def put(self, a: int, v) -> None:
        for lane in (0, 1):
            x = v[lane]
            self.wr(a + lane, x if isinstance(x, int) else m.st(x))

    def put2(self, a: int, x) -> None:
        """Both lanes get the same value (the exchange broadcast)."""
        self.put(a, [x, x])


def sext7(word: int) -> int:
    v = (word >> 25) & 0x7F
    return v - 128 if v & 0x40 else v


def sine4(words: list[list[int]]) -> list[list[m.X]]:
    """`words[k][lane]` -> `sines[k][lane]`."""
    return [[ss.sine_of_word(words[k][lane]) for lane in (0, 1)] for k in range(4)]


def phases(acc: list[int], inc: list[int]) -> list[list[int]]:
    """`acc + (k + 1) inc` for k = 0..3 per lane (`R0 = R8 + R12`, `R1 = R8 + 2 R12`, ...)."""
    return [[m.u32(acc[lane] + (k + 1) * inc[lane]) for lane in (0, 1)] for k in range(4)]


def mod_words(sines, depth) -> list[list[int]]:
    """`FIX(s_k depth, 26) << 5` per lane."""
    return [
        [m.u32(m.u32(m.fix(m.mul(sines[k][lane], depth[lane]), 26)) << 5) for lane in (0, 1)]
        for k in range(4)
    ]


def lerp(lo: m.X, hi: m.X, frac: m.X) -> m.X:
    return m.add(lo, m.mul(m.sub(hi, lo), frac))


def stage1_setup(mm: Mem, i0: int, i1: int, i2: int) -> None:
    """Prologue up to the loop: increments, band curve lerps, fades, the two modulation depths."""
    i6 = mm.rd(i2 - 32)
    p0, a, b = mm.fl(i2 - 29), mm.fl(i6 - 32), mm.fl(i6 - 30)
    sc = [m.s32(w) for w in mm.lanes(i0 - 10)]
    mm.put(i2 - 25, [m.u32(m.fix(m.mul(p0[x], a[x]), sc[x])) for x in (0, 1)])
    mm.put(i2 - 23, [m.u32(m.fix(m.mul(p0[x], b[x]), sc[x])) for x in (0, 1)])
    word = mm.lanes(i2 - 31)
    i5 = m.u32(sext7(word[0]) + m.s32(mm.rd(i1 - 14)))
    a1 = [m.flt(w, -31) for w in mm.lanes(i2 - 25)]
    a2 = [m.flt(w, -31) for w in mm.lanes(i2 - 23)]
    frac = [m.flt(m.u32((w & 0x1FFFFFF) << 6), -31) for w in word]
    t0, t1, t2 = (mm.fl(i5 + 129 * k) for k in range(3))
    t0n, t1n, t2n = (mm.fl(i5 + 129 * k + 1) for k in range(3))
    # lane 0 interpolates band curve 0, lane 1 curve 1; B0 is lane 0 of curve 2
    big_a0 = lerp(t0[0], t0n[0], frac[0])
    big_a1 = lerp(t1[0], t1n[0], frac[1])
    big_b0 = lerp(t2[0], t2n[0], frac[0])
    mm.put(i4_off(-16), a1)
    mm.put(i4_off(-14), a2)
    mm.put2(i4_off(-24), big_a1)
    mm.put2(i4_off(-22), big_b0)
    e0, e1 = mm.fl(i1 - 29), mm.fl(i1 - 27)
    f12 = [m.mul(e0[x], a1[x]) for x in (0, 1)]
    f13 = [m.mul(e1[x], a1[x]) for x in (0, 1)]
    g2 = [m.mul(m.add(m.clip(m.sub(FADE_C0, f), FADE_C1), FADE_C1), FADE_C2) for f in f12]
    g3 = [m.mul(m.add(m.clip(m.sub(FADE_C0, f), FADE_C1), FADE_C1), FADE_C2) for f in f13]
    f2 = [m.mul(big_a0, g) for g in g2]
    w19 = mm.fl(i1 - 19)
    gg = [m.mul(g3[x], w19[x]) for x in (0, 1)]
    mm.put2(i4_off(-20), f2[0])
    mm.put2(i4_off(-18), f2[1])
    mm.put2(i4_off(8), gg[0])
    mm.put2(i4_off(10), gg[1])
    x = mm.fl(i2 - 5)
    f4 = [m.mul(v, v) for v in x]
    r, s = mm.bc(i1 - 22), mm.bc(i1 - 21)
    c, d = mm.bc(i1 - 25), mm.bc(i1 - 24)
    f0 = [m.add(m.mul(r[k], f4[k]), c[k]) for k in (0, 1)]
    f1 = [m.add(m.mul(s[k], f4[k]), d[k]) for k in (0, 1)]
    h0, h1 = mm.fl(i4_off(-20)), mm.fl(i4_off(-18))
    la, lb = mm.fl(i4_off(-24)), mm.fl(i4_off(-22))
    mm.put(i4_off(4), [m.clip(m.mul(f0[k], h0[k]), la[k]) for k in (0, 1)])
    mm.put(i4_off(6), [m.clip(m.mul(f1[k], h1[k]), lb[k]) for k in (0, 1)])


def i4_off(off: int) -> int:
    return I4_BASE + off


def operator_pass(mm: Mem, i1: int, i5: int, i6: int, i7: int, f4: list[m.X]) -> None:
    """One loop iteration: four operators, then the weighted mix into `buf` at `I5`."""
    e = [mm.bc(i1 - 29 + k) for k in range(4)]
    inc = [
        [m.u32(m.fix(min(m.mul(e[k][lane], f4[lane]), INC_CEIL), 31)) for lane in (0, 1)]
        for k in range(4)
    ]
    mm.put(i4_off(-10), inc[1])
    mm.put(i4_off(-8), inc[2])
    mm.put(i4_off(-6), inc[3])
    ph = phases(mm.lanes(i7), inc[0])
    mm.put(i7, ph[3])
    s_a = sine4(ph)
    pm_a = mod_words(s_a, mm.fl(i4_off(4)))
    ph = phases(mm.lanes(i7 + 4), mm.lanes(i4_off(-8)))
    mm.put(i7 + 4, ph[3])
    s_b = sine4([[m.u32(ph[k][x] + pm_a[k][x]) for x in (0, 1)] for k in range(4)])
    g0 = mm.fl(i4_off(8))
    for k in range(4):
        mm.put(i4_off(-32 + 2 * k), [m.mul(s_b[k][x], g0[x]) for x in (0, 1)])
    ph = phases(mm.lanes(i7 + 2), mm.lanes(i4_off(-10)))
    mm.put(i7 + 2, ph[3])
    s_c = sine4(ph)
    pm_c = mod_words(s_c, mm.fl(i4_off(6)))
    ph = phases(mm.lanes(i7 + 6), mm.lanes(i4_off(-6)))
    mm.put(i7 + 6, ph[3])
    s_d = sine4([[m.u32(ph[k][x] + pm_c[k][x]) for x in (0, 1)] for k in range(4)])
    g1 = mm.fl(i4_off(10))
    out = [
        [m.add(m.mul(s_d[k][x], g1[x]), m.ld(mm.rd(i4_off(-32 + 2 * k) + x))) for x in (0, 1)]
        for k in range(4)
    ]
    w0, w1 = mm.fl(i6), mm.fl(i6 + 2)
    buf = [mm.fl(i5 + 2 * k) for k in range(4)]
    for k in range(4):
        top = [m.add(m.mul(out[k][x], w0[x]), buf[k][x]) for x in (0, 1)]
        bot = [m.mul(out[k][x], w1[x]) for x in (0, 1)]
        mm.put(i5 + 2 * k, [m.add(top[0], bot[1]), m.add(top[1], bot[0])])


def stage1(rd: Rd, wr: Wr, i0: int, i1: int, i2: int, i3: int) -> None:
    """Model of `0x124269`: writes the same words as the listing."""
    mm = Mem(rd, wr)
    stage1_setup(mm, i0, i1, i2)
    i5 = i2 - 13
    i6 = m.u32(mm.rd(i2 - 32) - 16)
    i7 = m.u32(mm.rd(i3 - 22) - 16)
    for k in (-13, -11, -9, -7):
        mm.put(i2 + k, [0, 0])
    f4 = mm.fl(i4_off(-16))
    for it in range(2):
        operator_pass(mm, i1, i5, m.u32(i6 + 4 * it), m.u32(i7 + 8 * it), f4)
        f4 = mm.fl(i4_off(-14))
    gain = mm.fl(mm.rd(i2 - 32) + 16)
    for k in range(4):
        buf = mm.fl(i5 + 2 * k)
        mm.put(i5 + 2 * k, [m.mul(buf[x], gain[x]) for x in (0, 1)])
