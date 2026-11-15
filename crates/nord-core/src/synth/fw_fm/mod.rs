//! Firmware FM stages `0x124269` / `0x12453F` (strip types 49..53) as opt-in equation models.
//!
//! Spec: `docs/specs/synth-engine.md` section 4.4; evidence `docs/research/fm-banks-1-2.md`;
//! bit-exact Python models `tools/synth_stage_fm.py` (1240 of 1240 and 1040 of 1040 words equal to
//! the listing interpreter `tools/sharc_text_vm.py`, itself equal to `sharc-org`), goldens
//! `tools/synth_fm_golden.py` -> `tests_fw_fm_golden.rs`.
//!
//! Both stages run two passes (a first oscillator chain at the increment of `a1`, a second at
//! `a2`), each four samples of a phase-modulation chain (stage 1: A, B, C, D; stage 2: A, B, C)
//! whose sines are the helper `0x12522E`; the lanes are mixed with the P words and the sum is
//! scaled by `P[16]`. The structs carry the DM words the stage reads, named by their offsets.
//! Nothing here is reachable from the default render path (D22): a caller builds [`FmInputs`] and
//! steps [`FmState`] itself. The wiring to a voice (position word `W`, pitch, the ratio table of
//! `bank0_CODE@0x2D82A8`, the P words of the wave) is open (OQ22).
//!
//! Real-time safe: fixed arrays only, no allocation in [`fm_stage1`] / [`fm_stage2`].

#[cfg(test)]
mod golden;
mod stage1;
mod stage2;
pub mod table;

pub use stage1::fm_stage1;
pub use stage2::fm_stage2;
pub use table::{fm_table, FmTable, TABLE_WORDS};

use super::fw_sine_warps::sine_poly;

/// VERIFIED (interpreter equal): the table base the ARM stores at `DM(0xC15A8)` is `0xE0926`; the
/// curve index `sext7(W >> 25)` is added to it.
pub const CURVE_BASE: u32 = 0xE0926;
/// VERIFIED (listing `0x24864A`, `R12 = 0x3f7fbe77`): ceiling of an operator increment.
const INC_CEIL: f32 = f32::from_bits(0x3F7F_BE77);

/// DM words the stage reads; a lane pair is `[PEx, PEy]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FmInputs {
    /// `DM(I2-29)`: the pitch term.
    pub p0: [f32; 2],
    /// `DM(P-32)` and `DM(P-30)` (`P` = `DM(I2-32)`): ratios of the two oscillator passes.
    pub ratio: [[f32; 2]; 2],
    /// `DM(I0-10)`: `FIX` scale of the increments (31).
    pub inc_scale: u32,
    /// `DM(I2-31)`: position word `W` (curve index in bits 25..31, fraction in bits 0..24).
    pub pos: [u32; 2],
    /// `DM(I2-5)`: the modulation depth source `x`.
    pub x: [f32; 2],
    /// ARM words `DM(I1-29 ..= I1-18)`: word at offset `o` is `arm[o + 29]`.
    pub arm: [f32; 12],
    /// P words `P[-16 .. -8]` (weights of the lane mix, two per pass).
    pub weights: [f32; 8],
    /// `P[16]` and `P[17]`: output gain per lane.
    pub gain: [f32; 2],
}

impl FmInputs {
    /// ARM word at `DM(I1 + o)`, `o` in `-29..=-18` (a broadcast load reads this word for both lanes).
    #[inline]
    pub(super) fn arm_at(&self, o: i32) -> f32 {
        self.arm[(o + 29) as usize]
    }

    /// ARM words of the lane pair at `DM(I5 + o)` (lane 1 reads the next word).
    #[inline]
    pub(super) fn arm_pair(&self, o: i32) -> [f32; 2] {
        let k = (o + 29) as usize;
        [self.arm[k], self.arm[(k + 1).min(11)]]
    }
}

/// Phase accumulators `acc[pass][slot][lane]`, slot = `DM(I7 + 2 slot)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FmState {
    /// The eight accumulator pairs.
    pub acc: [[[u32; 2]; 4]; 2],
}

/// `FIX Fx BY shift` to a word: `x 2^shift` rounded to nearest even, saturating.
#[inline]
pub(super) fn fix(x: f32, shift: i32) -> u32 {
    let v = (f64::from(x) * 2f64.powi(shift)).round_ties_even();
    v.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32 as u32
}

/// `FLOAT Rx BY shift`: the signed word times `2^shift`.
#[inline]
pub(super) fn flt(w: u32, shift: i32) -> f32 {
    (f64::from(w as i32) * 2f64.powi(shift)) as f32
}

/// The sine helper on a phase word (`x = word 2^-30`).
#[inline]
pub(super) fn sine_word(w: u32) -> f32 {
    sine_poly(flt(w, -30))
}

/// `FEXT R BY 25:7 (SE)`.
#[inline]
pub(super) fn sext7(w: u32) -> i32 {
    (w as i32) >> 25
}

#[inline]
pub(super) fn lerp(lo: f32, hi: f32, f: f32) -> f32 {
    lo + (hi - lo) * f
}

/// Curve word `slot` (outside `0xE0900..0xE0A68` reads 0, ASSUMED: unknown memory).
#[inline]
pub(super) fn curve(t: &FmTable, slot: i64) -> f32 {
    usize::try_from(slot)
        .ok()
        .and_then(|k| t.get(k))
        .map_or(0.0, |w| f32::from_bits(*w))
}

/// Phase words `acc + (k + 1) inc`, `k = 0..4`, wrapping.
#[inline]
pub(super) fn phases(acc: u32, inc: u32) -> [u32; 4] {
    let mut out = [0u32; 4];
    for (k, o) in out.iter_mut().enumerate() {
        *o = acc.wrapping_add(inc.wrapping_mul(k as u32 + 1));
    }
    out
}

/// Modulation words `FIX(s depth, 26) << 5`.
#[inline]
pub(super) fn mod_words(s: &[f32; 4], depth: f32) -> [u32; 4] {
    let mut out = [0u32; 4];
    for (o, v) in out.iter_mut().zip(s) {
        *o = fix(v * depth, 26) << 5;
    }
    out
}

/// Increment word `FIX(min(e a, ceiling), 31)`.
#[inline]
pub(super) fn inc_word(e: f32, a: f32) -> u32 {
    fix((e * a).min(INC_CEIL), 31)
}

/// The cross-lane mix of one pass: `buf_k = (out_k w0 + buf_k) + swap(out_k w1)`.
#[inline]
pub(super) fn mix(buf: &mut [[f32; 2]; 4], out: &[[f32; 2]; 4], w0: [f32; 2], w1: [f32; 2]) {
    for (b, o) in buf.iter_mut().zip(out) {
        let top = [o[0] * w0[0] + b[0], o[1] * w0[1] + b[1]];
        let bot = [o[0] * w1[0], o[1] * w1[1]];
        *b = [top[0] + bot[1], top[1] + bot[0]];
    }
}

/// Setup shared by both stages.
pub(super) struct Setup {
    /// `a1 = inc1 2^-31` per lane (the first pass increment scale).
    pub a1: [f32; 2],
    /// `a2` per lane (the second pass).
    pub a2: [f32; 2],
    /// Lane 0 lerp of curve 0.
    pub a0: f32,
    /// Lane 1 lerp of curve 1.
    pub a1_curve: f32,
    /// Lerp of curve 2, lane 0 forward and lane 1 reversed.
    pub b: [f32; 2],
}

/// Increments, curve lerps (lane 0 curve 0 / 2, lane 1 curve 1 / reversed 2).
pub(super) fn setup(inp: &FmInputs, t: &FmTable) -> Setup {
    let word = |r: [f32; 2], l: usize| fix(inp.p0[l] * r[l], inp.inc_scale as i32);
    let frac = |l: usize| flt((inp.pos[l] & 0x01FF_FFFF) << 6, -31);
    let slot = i64::from(CURVE_BASE - table::TABLE_BASE) + i64::from(sext7(inp.pos[0]));
    let c = |row: i64, d: i64| curve(t, slot + 129 * row + d);
    Setup {
        a1: [0, 1].map(|l| flt(word(inp.ratio[0], l), -31)),
        a2: [0, 1].map(|l| flt(word(inp.ratio[1], l), -31)),
        a0: lerp(c(0, 0), c(0, 1), frac(0)),
        a1_curve: lerp(c(1, 0), c(1, 1), frac(1)),
        b: [
            lerp(c(2, 0), c(2, 1), frac(0)),
            lerp(c(2, 1), c(2, 0), frac(1)),
        ],
    }
}

/// Plain inputs for unit tests.
#[cfg(test)]
pub(super) fn test_inputs() -> FmInputs {
    FmInputs {
        p0: [0.01, 0.01],
        ratio: [[1.0, 1.0], [2.0, 2.0]],
        inc_scale: 31,
        pos: [0x0400_0000, 0x0400_0000],
        x: [0.5, 0.5],
        arm: [1.0; 12],
        weights: [0.5; 8],
        gain: [1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_rounds_to_even_and_saturates() {
        assert_eq!(fix(0.5, 1), 1);
        assert_eq!(fix(2.5, 0), 2);
        assert_eq!(fix(3.5, 0), 4);
        assert_eq!(fix(10.0, 31), i32::MAX as u32);
        assert_eq!(fix(-10.0, 31), i32::MIN as u32);
    }

    #[test]
    fn sext7_reads_the_top_seven_bits_signed() {
        assert_eq!(sext7(0x7F << 25), -1);
        assert_eq!(sext7(0x3F << 25), 63);
        assert_eq!(sext7(0x40 << 25), -64);
        assert_eq!(sext7(0x01FF_FFFF), 0);
    }

    #[test]
    fn phases_wrap_and_step_four_times() {
        assert_eq!(phases(10, 3), [13, 16, 19, 22]);
        assert_eq!(phases(u32::MAX, 2), [1, 3, 5, 7]);
    }

    #[test]
    fn mod_words_shift_by_five() {
        assert_eq!(mod_words(&[1.0, 0.0, 0.5, -1.0], 1.0)[0], fix(1.0, 26) << 5);
        assert_eq!(mod_words(&[0.0; 4], 3.0), [0; 4]);
    }

    #[test]
    fn sine_word_peaks_at_the_quarter_cycles() {
        // t = |word 2^-30| - 1: word 0 is the trough, a quarter turn the zero, a half turn the peak
        assert!(sine_word(0) < -0.99);
        assert!(sine_word(0x4000_0000).abs() < 1e-6);
        assert!(sine_word(0x8000_0000) > 0.99);
    }

    #[test]
    fn mix_swaps_the_second_weight_across_lanes() {
        let mut buf = [[0.0; 2]; 4];
        let out = [[1.0, 2.0]; 4];
        mix(&mut buf, &out, [1.0, 1.0], [10.0, 100.0]);
        assert_eq!(buf[0], [1.0 + 200.0, 2.0 + 10.0]);
    }

    #[test]
    fn curve_outside_the_table_reads_zero() {
        let t = fm_table(49).unwrap();
        assert_eq!(curve(t, -1), 0.0);
        assert_eq!(curve(t, TABLE_WORDS as i64), 0.0);
        assert_eq!(curve(t, 0), f32::from_bits(t[0]));
    }

    #[test]
    fn arm_words_index_by_offset_and_the_pair_reads_the_next_word() {
        let mut inp = test_inputs();
        inp.arm = std::array::from_fn(|k| k as f32);
        assert_eq!(inp.arm_at(-29), 0.0);
        assert_eq!(inp.arm_pair(-27), [2.0, 3.0]);
    }
}
