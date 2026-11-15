//! The unison stages (SUPER 0 / 1 / 2..4, MULTI) with the P words the ARM writes
//! (`--synth-p verified`): sixteen unison ratios, the lane mix weights, `P[16]`, the MULTI
//! multiplier record and the spread word `s5 = Osc Ctrl`.
//!
//! The placeholder stages of `mod.rs` keep the ASSUMED constants; this mode reads
//! [`StageWords`] instead. Units per stage: `p_stage_words.rs`; parity with the Python stage
//! models: `tests_fw_stage_golden11.rs` (`tools/synth_stage_golden11.py`).
//!
//! Spec: `docs/specs/synth-engine.md` section 4.2; evidence `docs/research/synth-p-writer.md`.

use super::super::fw_super1::{fold_diff, SUPER1_VOICES};
use super::super::fw_super2::super24_sample_words;
use super::super::gain_dezip::SY_FRAME_RATE_HZ;
use super::super::p_stage_words::{spread_word, StageWords, WORD_VOICES};
use super::{fade, saw_diff, spread_u, FwStage, FwStageState};
use crate::phase_wrap::wrap_pos;

/// Both lanes of one unison voice loop: `own` weights the lane x voice, `cross` the lane y voice.
/// `osc(phase, increment)` is the inline oscillator of the stage.
#[inline]
fn unison_mix(
    phases: [&mut [f32; 8]; 2],
    dt: f32,
    u: f32,
    voices: usize,
    w: &StageWords,
    osc: fn(f32, f32) -> f32,
) -> f32 {
    let [px, py] = phases;
    let mut sum = 0.0;
    for (j, (x, y)) in px.iter_mut().zip(py.iter_mut()).take(voices).enumerate() {
        let dx = dt * (w.ratios[0][j] * u + 1.0 - u);
        let dy = dt * (w.ratios[1][j] * u + 1.0 - u);
        sum += w.own[j] * osc(*x, dx);
        *x = wrap_pos(*x + dx, 1.0);
        if w.cross[j] != 0.0 {
            sum += w.cross[j] * osc(*y, dy);
        }
        *y = wrap_pos(*y + dy, 1.0);
    }
    sum
}

/// MULTI: three voices, increment `(r_v s5 + 1 - s5) oct_v r_0 f` (voice 0 without the first
/// factor), each with its own high-pitch fade folded into the weights (`0x243A3C`).
#[inline]
fn multi_mix(st: &mut FwStageState, dt: f32, i_sy: f32, s5: f32, w: &StageWords) -> f32 {
    let mut sum = 0.0;
    for v in 0..3 {
        let mult = |lane: usize| {
            let d = if v == 0 {
                1.0
            } else {
                w.ratios[lane][v] * s5 + 1.0 - s5
            };
            w.octaves[v] * d * w.ratios[lane][0]
        };
        let (mx, my) = (mult(0), mult(1));
        let (dx, dy) = (dt * mx, dt * my);
        let g = fade(2.0 * i_sy * mx);
        sum += g * w.own[v] * saw_diff(st.phases[v], dx);
        st.phases[v] = wrap_pos(st.phases[v] + dx, 1.0);
        if w.cross[v] != 0.0 {
            sum += g * w.cross[v] * saw_diff(st.phases_y[v], dy);
        }
        st.phases_y[v] = wrap_pos(st.phases_y[v] + dy, 1.0);
    }
    sum
}

/// Start phases of the sixteen unison voices (lane x `0..8`, lane y `8..16`).
///
/// ASSUMED: arbitrary, as the free running firmware phase words are. Picked as the pseudo-random
/// set (Python `random.Random(1369)`, two decimals) whose harmonic sums `|Sum exp(2 pi i k p)|`
/// for `k = 1..5` are 3.5 to 4.4, the value 4 = sqrt(16) of random phases, so the coherent
/// start (detune 0) is neither cancelling nor constructive.
const START_PHASES: [f32; 16] = [
    0.59, 0.29, 0.30, 0.10, 0.75, 0.01, 0.78, 0.75, 0.01, 0.88, 0.10, 0.15, 0.71, 0.65, 0.51, 0.74,
];

impl FwStageState {
    /// Voice state of the verified-words mode: the unison voices start spread over the cycle
    /// (`START_PHASES`) instead of coherent at phase 0.
    ///
    /// ASSUMED: the firmware voice phase words are free running state, not reset at key-on.
    /// EMPIRICAL support: the Super Saw lesson clip keeps its level within 1.2 dB from Detune 0 to
    /// 10 (`docs/research/synth-validation.md` s1), which a coherent start does not (the model
    /// falls 15 dB from ctrl 0 to 1).
    pub fn staggered() -> Self {
        let mut s = Self::default();
        for j in 0..8 {
            s.phases[j] = START_PHASES[j];
            s.phases_y[j] = START_PHASES[8 + j];
        }
        for j in 0..s.s24.phase.len() {
            s.s24.phase[j] = (f64::from(START_PHASES[j]) * 4_294_967_296.0) as u32;
            s.s24.phase_y[j] = (f64::from(START_PHASES[8 + j]) * 4_294_967_296.0) as u32;
        }
        s
    }

    /// One sample of `stage` with the P words of the program. Stages without unison words play
    /// as [`FwStageState::sample`].
    #[inline]
    pub fn sample_words(
        &mut self,
        stage: FwStage,
        t: f32,
        dt: f32,
        freq_hz: f32,
        ctrl: f32,
        w: &StageWords,
    ) -> f32 {
        let i_sy = freq_hz / SY_FRAME_RATE_HZ;
        match stage {
            FwStage::Super => {
                let u = spread_u(spread_word(ctrl));
                let g = fade(2.0 * i_sy);
                let (a, b) = (&mut self.phases, &mut self.phases_y);
                w.gain * g * unison_mix([a, b], dt, u, WORD_VOICES, w, saw_diff)
            }
            FwStage::Super1 => {
                let u = spread_u(spread_word(ctrl));
                let g = fade(2.0 * i_sy);
                let (a, b) = (&mut self.phases, &mut self.phases_y);
                w.gain * g * unison_mix([a, b], dt, u, SUPER1_VOICES, w, fold_diff)
            }
            FwStage::Multi => w.gain * multi_mix(self, dt, i_sy, spread_word(ctrl), w),
            FwStage::Super24 => super24_sample_words(&mut self.s24, dt, i_sy, ctrl, w),
            _ => self.sample(stage, t, dt, freq_hz, ctrl),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(stage: FwStage, cat: u8, wave: u8, ctrl: f32, n: usize) -> Vec<f32> {
        let w = StageWords::for_wave(cat, wave).unwrap();
        let mut s = FwStageState::default();
        let dt = 220.0 / 48_000.0;
        (0..n)
            .map(|_| s.sample_words(stage, 0.0, dt, 220.0, ctrl, &w))
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// `(case id, category, wave, stage)` of `tools/synth_stage_golden11.py`.
    const CASES: [(u8, u8, FwStage); 5] = [
        (6, 0, FwStage::Super),
        (6, 1, FwStage::Super1),
        (5, 0, FwStage::Multi),
        (5, 4, FwStage::Multi),
        (5, 2, FwStage::Multi),
    ];

    /// Start phase of voice `j` of `lane` in case `n` (`phase_word` of the generator).
    fn start_phase(n: usize, lane: usize, j: usize) -> f64 {
        let k = (n * 16 + lane * 8 + j) as u32;
        f64::from((k.wrapping_mul(0x2F1B_3C5).wrapping_add(12_345) & 0xFFFF_FF00) as u32)
            / 4_294_967_296.0
    }

    #[test]
    fn matches_the_python_stage_models_fed_the_arm_words() {
        use super::super::super::tests_fw_stage_golden11 as gold;
        let mut worst = 0.0f32;
        for (row, &case) in gold::CASE.iter().enumerate() {
            let (cat, wave, stage) = CASES[case as usize];
            let w = StageWords::for_wave(cat, wave).unwrap();
            let dt = f32::from_bits(gold::DT[row]);
            let s5 = f32::from_bits(gold::S5[row]);
            let mut st = FwStageState::default();
            // the model computes sample k from `phase + k inc`: start one increment ahead
            let inc = |lane: usize, j: usize| {
                let (rho, d) = (w.ratios[lane][j], dt);
                let u = if stage == FwStage::Multi {
                    0.0
                } else {
                    spread_u(s5)
                };
                match stage {
                    FwStage::Multi if j == 0 => d * w.octaves[0] * w.ratios[lane][0],
                    FwStage::Multi => {
                        let dd = rho * s5 + 1.0 - s5;
                        d * dd * w.octaves.get(j).copied().unwrap_or(1.0) * w.ratios[lane][0]
                    }
                    _ => d * (rho * u + 1.0 - u),
                }
            };
            for j in 0..8 {
                let (x, y) = (
                    start_phase(case as usize, 0, j),
                    start_phase(case as usize, 1, j),
                );
                st.phases[j] = wrap_pos((x + f64::from(inc(0, j))) as f32, 1.0);
                st.phases_y[j] = wrap_pos((y + f64::from(inc(1, j))) as f32, 1.0);
            }
            for k in 0..4 {
                let got = st.sample_words(stage, 0.0, dt, dt * SY_FRAME_RATE_HZ, s5, &w);
                let want = f32::from_bits(gold::OUT[4 * row + k]);
                worst = worst.max((got - want).abs());
            }
        }
        assert!(worst < 5e-4, "worst deviation {worst}");
    }

    #[test]
    fn words_stages_are_finite_bounded_and_deterministic() {
        for (stage, c, v) in [
            (FwStage::Super, 6, 0),
            (FwStage::Super1, 6, 1),
            (FwStage::Super24, 6, 3),
            (FwStage::Multi, 5, 0),
            (FwStage::Multi, 5, 2),
        ] {
            let a = run(stage, c, v, 0.6, 3000);
            assert!(
                a.iter().all(|x| x.is_finite() && x.abs() < 2.0),
                "{stage:?}"
            );
            assert!(rms(&a) > 0.01, "{stage:?} silent");
            assert_eq!(a, run(stage, c, v, 0.6, 3000));
        }
    }

    #[test]
    fn super_unison_has_the_level_of_one_oscillator_at_p16() {
        // sixteen voices of weight 1/16: random-phase sum, RMS near a single saw x P[16] x 1.0
        let x = run(FwStage::Super, 6, 0, 0.5, 8000);
        let r = rms(&x);
        assert!((0.1..0.5).contains(&r), "rms {r}");
    }

    #[test]
    fn other_stages_fall_through_to_the_plain_sample() {
        let w = StageWords::for_wave(6, 0).unwrap();
        let (mut a, mut b) = (FwStageState::default(), FwStageState::default());
        for _ in 0..50 {
            let x = a.sample_words(FwStage::Noise, 0.0, 0.0, 0.0, 0.0, &w);
            let y = b.sample(FwStage::Noise, 0.0, 0.0, 0.0, 0.0);
            assert_eq!(x.to_bits(), y.to_bits());
        }
    }

    #[test]
    fn staggered_start_keeps_the_super_level_flat_over_the_spread() {
        // coherent voices fall 15 dB from ctrl 0 to 1; the staggered start stays within 6 dB
        let w = StageWords::for_wave(6, 0).unwrap();
        let level = |ctrl: f32, mk: fn() -> FwStageState| {
            let mut s = mk();
            let dt = 220.0 / 48_000.0;
            let x: Vec<f32> = (0..9000)
                .map(|_| s.sample_words(FwStage::Super, 0.0, dt, 220.0, ctrl, &w))
                .collect();
            rms(&x[1000..])
        };
        let (a, b) = (
            level(0.0, FwStageState::staggered),
            level(1.0, FwStageState::staggered),
        );
        assert!((20.0 * (a / b).log10()).abs() < 6.0, "{a} {b}");
        let c = level(0.0, FwStageState::default);
        assert!(c > 1.5 * a, "coherent start is louder: {c} {a}");
    }

    #[test]
    fn spread_widens_the_unison_beat() {
        let a = run(FwStage::Super, 6, 0, 0.0, 4000);
        let b = run(FwStage::Super, 6, 0, 1.0, 4000);
        assert_ne!(a, b);
    }
}
