//! Stage `0x124269` (strip types 49..53, first FM stage): four operators A -> B, C -> D per pass.
//!
//! Per lane `l`, with `a` the pass increment scale (`a1`, then `a2`), `e_k` the ARM words
//! `DM(I1-29+k)` and `mA`, `mC` the modulation depths of `setup`:
//!
//! ```text
//! inc_k = FIX(min(e_k a, 0.99873), 31)
//! sA = sine(accA + k incA);   sB = sine(accB + k incB + (FIX(sA mA, 26) << 5)) G0
//! sC = sine(accC + k incC);   sD = sine(accD + k incD + (FIX(sC mC, 26) << 5)) G1
//! out_k = sD G1 + sB G0;      buf_k = (out_k w0 + buf_k) + swap(out_k w1);   result buf_k P[16]
//! ```

use super::{inc_word, mix, mod_words, phases, setup, sine_word, FmInputs, FmState, FmTable};
use crate::synth::fw_stages::fade;

/// Modulation depth `clip((r x^2 + c) h, |limit|)`.
#[inline]
fn depth(r: f32, c: f32, x2: f32, h: f32, limit: f32) -> f32 {
    ((r * x2 + c) * h).clamp(-limit.abs(), limit.abs())
}

/// Four samples of both lanes (`out[k][lane]`) of one call; the accumulators advance.
pub fn fm_stage1(st: &mut FmState, inp: &FmInputs, table: &FmTable) -> [[f32; 2]; 4] {
    let s = setup(inp, table);
    let (e0, e1, w19) = (inp.arm_pair(-29), inp.arm_pair(-27), inp.arm_pair(-19));
    let g2 = [0, 1].map(|l| fade(e0[l] * s.a1[l]));
    let g3 = [0, 1].map(|l| fade(e1[l] * s.a1[l]));
    let f2 = [s.a0 * g2[0], s.a0 * g2[1]];
    // G0, G1: the lane gains of the second and the fourth operator (stored as broadcast pairs).
    let gg = [g3[0] * w19[0], g3[1] * w19[1]];
    let x2 = inp.x.map(|v| v * v);
    let m_a = [0, 1].map(|l| depth(inp.arm_at(-22), inp.arm_at(-25), x2[l], f2[0], s.a1_curve));
    let m_c = [0, 1].map(|l| depth(inp.arm_at(-21), inp.arm_at(-24), x2[l], f2[1], s.b[0]));
    let mut buf = [[0.0f32; 2]; 4];
    for it in 0..2 {
        let a = if it == 0 { s.a1 } else { s.a2 };
        let inc = |k: i32, l: usize| inc_word(inp.arm_at(-29 + k), a[l]);
        let mut out = [[0.0f32; 2]; 4];
        for l in 0..2 {
            let acc = &mut st.acc[it];
            let ph_a = phases(acc[0][l], inc(0, l));
            acc[0][l] = ph_a[3];
            let pm_a = mod_words(&ph_a.map(sine_word), m_a[l]);
            let ph_b = phases(acc[2][l], inc(2, l));
            acc[2][l] = ph_b[3];
            let ph_c = phases(acc[1][l], inc(1, l));
            acc[1][l] = ph_c[3];
            let pm_c = mod_words(&ph_c.map(sine_word), m_c[l]);
            let ph_d = phases(acc[3][l], inc(3, l));
            acc[3][l] = ph_d[3];
            for k in 0..4 {
                let s_b = sine_word(ph_b[k].wrapping_add(pm_a[k]));
                let s_d = sine_word(ph_d[k].wrapping_add(pm_c[k]));
                out[k][l] = s_d * gg[1] + s_b * gg[0];
            }
        }
        let w = &inp.weights[4 * it..4 * it + 4];
        mix(&mut buf, &out, [w[0], w[1]], [w[2], w[3]]);
    }
    for b in &mut buf {
        *b = [b[0] * inp.gain[0], b[1] * inp.gain[1]];
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::super::{fm_table, test_inputs};
    use super::*;

    #[test]
    fn output_is_finite_and_the_accumulators_advance() {
        let t = fm_table(49).unwrap();
        let mut st = FmState::default();
        let inp = test_inputs();
        let out = fm_stage1(&mut st, &inp, t);
        assert!(out.iter().flatten().all(|v| v.is_finite()));
        assert!(st.acc[0][0][0] != 0 && st.acc[1][3][1] != 0);
    }

    #[test]
    fn two_calls_continue_the_phases_and_are_deterministic() {
        let t = fm_table(50).unwrap();
        let (mut a, mut b) = (FmState::default(), FmState::default());
        let inp = test_inputs();
        let first = fm_stage1(&mut a, &inp, t);
        let again = fm_stage1(&mut b, &inp, t);
        assert_eq!(first, again);
        assert_ne!(fm_stage1(&mut a, &inp, t), first);
    }

    #[test]
    fn zero_gain_silences_the_stage() {
        let t = fm_table(51).unwrap();
        let mut inp = test_inputs();
        inp.gain = [0.0, 0.0];
        let out = fm_stage1(&mut FmState::default(), &inp, t);
        assert!(out.iter().flatten().all(|v| *v == 0.0));
    }

    #[test]
    fn depth_clips_to_the_limit_magnitude() {
        assert_eq!(depth(100.0, 0.0, 1.0, 1.0, -0.5), 0.5);
        assert_eq!(depth(-100.0, 0.0, 1.0, 1.0, 0.5), -0.5);
    }
}
