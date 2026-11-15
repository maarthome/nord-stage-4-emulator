//! Stage `0x12453F` (strip types 49..53, second FM stage): a three operator chain A -> B -> C.
//!
//! Per lane, with the notation of [`super::stage1`]; three fades `g_j = fade(e_j a)` of the ARM
//! words `DM(I1-29)`, `DM(I1-27)`, `DM(I1-26)` give the depths and the output gain:
//!
//! ```text
//! sA = sine(accA + k incA);   sB = sine(accB + k incB + (FIX(sA mA, 26) << 5))
//! sC = sine(accC + k incC + (FIX(sB mC, 26) << 5))       out_k = sC w18 g_2
//! buf_k = (out_k w0 + buf_k) + swap(out_k w1);            result buf_k P[16]
//! ```

use super::{inc_word, mix, mod_words, phases, setup, sine_word, FmInputs, FmState, FmTable};
use crate::synth::fw_stages::fade;

/// Modulation depth `clip((r x^2 + c) h, |limit|)`.
#[inline]
fn depth(r: f32, c: f32, x2: f32, h: f32, limit: f32) -> f32 {
    ((r * x2 + c) * h).clamp(-limit.abs(), limit.abs())
}

/// Four samples of both lanes (`out[k][lane]`) of one call; the accumulators advance.
pub fn fm_stage2(st: &mut FmState, inp: &FmInputs, table: &FmTable) -> [[f32; 2]; 4] {
    let s = setup(inp, table);
    let g = |o: i32| [0, 1].map(|l| fade(inp.arm_at(o) * s.a1[l]));
    let (g0, g1, g2) = (g(-29), g(-27), g(-26));
    let x2 = inp.x.map(|v| v * v);
    let m_a = [0, 1].map(|l| {
        depth(
            inp.arm_at(-22),
            inp.arm_at(-25),
            x2[l],
            s.a0 * g0[l],
            s.a1_curve,
        )
    });
    let m_c = [0, 1].map(|l| {
        depth(
            inp.arm_at(-20),
            inp.arm_at(-23),
            x2[l],
            s.a0 * g1[l],
            s.b[l],
        )
    });
    let gain_c = [0, 1].map(|l| inp.arm_at(-18) * g2[l]);
    let mut buf = [[0.0f32; 2]; 4];
    for it in 0..2 {
        let a = if it == 0 { s.a1 } else { s.a2 };
        let mut out = [[0.0f32; 2]; 4];
        for l in 0..2 {
            let acc = &mut st.acc[it];
            let ph_a = phases(acc[0][l], inc_word(inp.arm_at(-29), a[l]));
            acc[0][l] = ph_a[3];
            let pm_a = mod_words(&ph_a.map(sine_word), m_a[l]);
            let ph_b = phases(acc[2][l], inc_word(inp.arm_at(-27), a[l]));
            acc[2][l] = ph_b[3];
            let s_b: [f32; 4] = std::array::from_fn(|k| sine_word(ph_b[k].wrapping_add(pm_a[k])));
            let pm_c = mod_words(&s_b, m_c[l]);
            let ph_c = phases(acc[3][l], inc_word(inp.arm_at(-26), a[l]));
            acc[3][l] = ph_c[3];
            for k in 0..4 {
                out[k][l] = sine_word(ph_c[k].wrapping_add(pm_c[k])) * gain_c[l];
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
    fn output_is_finite_and_three_accumulator_slots_advance() {
        let t = fm_table(52).unwrap();
        let mut st = FmState::default();
        let out = fm_stage2(&mut st, &test_inputs(), t);
        assert!(out.iter().flatten().all(|v| v.is_finite()));
        assert!(st.acc[0][0][0] != 0 && st.acc[0][2][0] != 0 && st.acc[0][3][0] != 0);
        assert_eq!(st.acc[0][1], [0, 0]);
    }

    #[test]
    fn stage_two_differs_from_stage_one_on_the_same_words() {
        let t = fm_table(53).unwrap();
        let inp = test_inputs();
        let a = fm_stage2(&mut FmState::default(), &inp, t);
        let b = super::super::fm_stage1(&mut FmState::default(), &inp, t);
        assert_ne!(a, b);
    }

    #[test]
    fn zero_gain_silences_the_stage() {
        let t = fm_table(49).unwrap();
        let mut inp = test_inputs();
        inp.gain = [0.0, 0.0];
        let out = fm_stage2(&mut FmState::default(), &inp, t);
        assert!(out.iter().flatten().all(|v| *v == 0.0));
    }
}
