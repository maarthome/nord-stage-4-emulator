#!/usr/bin/env python3
"""Bit-exact model of the WAVE bank stage `0x1247B6` (strip types 54 and 56, kOscBank 3).

Listing `0x248F6C..0x24930A` (ORG / PNO core 2, SIMD). It is the two-oscillator sibling of the
SUPER 2..4 stage `0x124985` ([synth_stage_super2](synth_stage_super2.py)): the same band row,
the same 6-word buffer and 8-tap polyphase FIR over the same table layout (int16 pairs at
`DM(0xC15A5)`, rows at PM `DM(0xC15A6)`), the same mix, scale and three-biquad output; only the
set-up differs.

    f   = clip(DM(-29, I2), 0.5)             g = C (B + clip(A - f, B))
    D_j = fix(f P[-32 + 2 j] 2^31)           j = 0, 1 (oscillator a, b), per lane
    positions of oscillator j: ph + 2 D_j and ph + 4 D_j (wrap 2^32), the phase word becomes
        ph + 4 D_j; the row `A, B = PM[0x9631F + 4 idx]` comes from `idx(2 f)` as in SUPER 2..4
    mix: per sample `m = (cross_a + own_a) + own_b + cross_b`, `a = st(m P[16] g)`, outputs
        `a a b b`, then three identical biquads (PM `0x95000`)

Compared with the oracle by `tools/synth_wave_oracle.py --stage 0x1247B6`
(`docs/research/synth-stage-models-13.md`).
"""

from __future__ import annotations

import random

import synth_stage_super2 as s2
import synth_wave_model as m

MASK = 0xFFFFFFFF
OSCILLATORS = 2
PITCH_LIMIT = s2.PITCH_LIMIT  # VERIFIED (listing `0x248F70`): `R5 = 0x3F000000`
# VERIFIED (image word `DM(0xC1DF8) = 0x1F`, listing `0x248F90`): the FIX scale of the increments.
FIX_SCALE = 31


def lane_setup(ln: m.Lane):
    """`(f, g, [D_a, D_b])`: clipped pitch, fade gain and the two integer increments."""
    f = m.clip(m.ld(ln.pitch), PITCH_LIMIT)
    g = m.ld(m.st(m.mul(m.C_FADE, m.add(m.clip(m.sub(m.A_FADE, f), m.B_FADE), m.B_FADE))))
    ds = [m.s32(m.fix(m.mul(f, m.ld(r)), FIX_SCALE)) for r in (ln.ratio1, ln.ratio2)]
    return f, g, ds


def oscillator(tab: s2.Tables, lanes, ds, count: int, offset: int, phases):
    """`[q][lane]` FIR sums and the new phase of one oscillator (both lanes)."""
    pos = []
    for i in (0, 1):
        step = (2 * ds[i]) & MASK
        ph1 = (phases[i] + step) & MASK
        ph2 = (ph1 + step) & MASK
        p1, p2 = ph1 * count, ph2 * count
        i1 = (p1 >> 32) + offset
        i2 = (p2 >> 32) + offset
        if i2 < i1:
            i2 += count
        w1 = i1 >> 1
        pos.append(
            (w1, i1 - 2 * w1, i2 - 2 * w1, (p1 & MASK) >> 23 & 0x1FF, (p2 & MASK) >> 23 & 0x1FF)
            + (ph2,)
        )
    bufs = [
        [tab.wave[pos[i][0] + n] if pos[i][0] + n < len(tab.wave) else 0 for n in range(6)]
        for i in (0, 1)
    ]
    out = []
    for q in (0, 1):
        x = s2.fir_lane(bufs[0], pos[0][1 + q], pos[0][3 + q] * 8, tab.fir)
        y = s2.fir_lane(bufs[1], pos[1][1 + q], pos[1][3 + q] * 8, tab.fir)
        out.append((x, y))
    return out, [p[5] for p in pos]


def stage_wave(lanes: list[m.Lane], tab: s2.Tables, signed: bool = False):
    """Stage `0x1247B6`: `(out_bits[lane][4], new phases[lane][2], lattice state words)`."""
    del signed
    info = [lane_setup(ln) for ln in lanes]
    idx = s2.row_index(info[0][0])
    count, offset = tab.rows[4 * idx + 1], tab.rows[4 * idx]
    mixes = []
    new_phases = [[0] * OSCILLATORS for _ in lanes]
    for j in range(OSCILLATORS):
        ds = [info[i][2][j] for i in (0, 1)]
        phases = [lanes[i].phase1 if j == 0 else lanes[i].phase2 for i in (0, 1)]
        res, phs = oscillator(tab, lanes, ds, count, offset, phases)
        mixes.append([[m.ld(m.st(m.flt(xo, -14))), m.ld(m.st(m.flt(yo, -14)))] for xo, yo in res])
        for i in (0, 1):
            new_phases[i][j] = phs[i]
    pair = []
    for q in (0, 1):
        lane_vals = []
        for i in (0, 1):
            run = None
            for j in range(OSCILLATORS):
                w = [m.ld(w_) for w_ in lanes[0].groups[j]]
                own = m.mul(mixes[j][q][i], w[i])
                cross = m.mul(mixes[j][q][1 - i], w[3 - i])
                run = m.add(cross, own) if j == 0 else m.add(m.add(run, own), cross)
            val = m.mul(m.mul(run, m.ld(lanes[i].out_gain)), info[i][1])
            lane_vals.append(m.st(val))
        pair.append(lane_vals)
    coefs = [m.ld(w) for w in tab.lattice]
    outs, states = [], []
    for i, ln in enumerate(lanes):
        words = [pair[0][i], pair[0][i], pair[1][i], pair[1][i]]
        new_states = []
        for n in range(3):
            lat = ln.lat or (0,) * 6
            s1, s2_ = m.ld(lat[2 * n]), m.ld(lat[2 * n + 1])
            words, s1, s2_ = s2.biquad(words, coefs, s1, s2_)
            new_states += [m.st(s1), m.st(s2_)]
        outs.append(words)
        states.append(new_states)
    return outs, new_phases, states


def random_state(seed: int, lanes: list[m.Lane]) -> None:
    """Ratios near 1 (a position pair stays inside the 6-word buffer) and biquad states."""
    r2 = random.Random(seed * 6700417)
    for ln in lanes:
        ln.ratio1 = m.f2u(2 ** r2.uniform(-0.5, 0.5))
        ln.ratio2 = m.f2u(2 ** r2.uniform(-0.5, 0.5))
        ln.ratios = (ln.ratio1, ln.ratio2)
        ln.phases = ()
        ln.lat = tuple(m.f2u(r2.uniform(-0.5, 0.5)) for _ in range(6))
