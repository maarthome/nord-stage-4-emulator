//! The P words one unison stage (MULTI, SUPER 0, SUPER 1, SUPER 2..4) reads, as a small `Copy`
//! value for the render loop (`--synth-p verified`, `p_words.rs`).
//!
//! The stages read their lane frame in the units of the firmware listing; the units were derived
//! with the Python stage models (`tools/synth_stage_unison.py`, `synth_stage_super1.py`,
//! `synth_stage_super2.py`: 320 of 320 oracle words each) fed the verified words
//! (`tools/synth_stage_golden11.py` -> `tests_fw_stage_golden11.rs`):
//!
//! | Stage | Per voice `j`, lane `i` | Mix of lane x |
//! |---|---|---|
//! | SUPER 0 / 1 / 2..4 | increment `(rho u + 1 - u) f`, `rho = P[-32 + 2 j + i]` | `P[16] g Sum_j (P[-16 + 4 j] s_xj + P[-13 + 4 j] s_yj)` |
//! | MULTI | increment `(P[-32 + 2 v + i] s5 + 1 - s5) oct_v P[-32 + i] f`, `oct` = `DM(0xC1596..98)` | the same groups, each voice with its own fade |
//!
//! Spec: `docs/specs/synth-engine.md` section 4.2; evidence `docs/research/synth-p-writer.md`.

use super::p_words::{words_for, PWords};

/// Voices per lane the stages loop over at most (SUPER 0).
pub const WORD_VOICES: usize = 8;

/// The words of one unison stage, both lanes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageWords {
    /// `P[16]`, stage output gain (VERIFIED words; times `GAIN_PER_P16` it is the engine level).
    pub gain: f32,
    /// `ratios[lane][j]`: `P[-32 + 2 j + lane]`.
    pub ratios: [[f32; WORD_VOICES]; 2],
    /// Weight of voice `j` of the own lane at the lane x output: `P[-16 + 4 j]`.
    pub own: [f32; WORD_VOICES],
    /// Weight of voice `j` of the other lane at the lane x output: `P[-13 + 4 j]`.
    pub cross: [f32; WORD_VOICES],
    /// MULTI voice multipliers `DM(0xC1596 .. 0xC1598)` (idle `1, 2, 0.5`).
    pub octaves: [f32; 3],
}

impl StageWords {
    /// Builds the value from the words of a type.
    pub fn from_words(w: &PWords) -> StageWords {
        let at = |i: i32| w.get(i).unwrap_or(0.0);
        let mut s = StageWords {
            gain: at(16),
            ratios: [[1.0; WORD_VOICES]; 2],
            own: [0.0; WORD_VOICES],
            cross: [0.0; WORD_VOICES],
            octaves: [at(-260), at(-259), at(-258)],
        };
        for j in 0..WORD_VOICES {
            let k = j as i32;
            for lane in 0..2 {
                s.ratios[lane][j] = w.get(-32 + 2 * k + lane as i32).unwrap_or(1.0);
            }
            s.own[j] = at(-16 + 4 * k);
            s.cross[j] = at(-13 + 4 * k);
        }
        s
    }

    /// The words of an ANALOG `(category, wave)` pair, for the unison stages only.
    pub fn for_wave(category: u8, wave: u8) -> Option<StageWords> {
        match (category, wave) {
            (5, _) | (6, 0..=4) => words_for(category, wave).map(Self::from_words),
            _ => None,
        }
    }
}

impl super::SynthLayer {
    /// Fresh stage state of a note: staggered unison phases in the verified-words mode.
    pub(super) fn new_stage_state(&self) -> super::FwStageState {
        if self.stage_words.is_some() && self.p_table.is_some_and(|t| t.words) {
            super::FwStageState::staggered()
        } else {
            super::FwStageState::default()
        }
    }
}

/// Spread word `s5` of the SUPER / MULTI stages from the Osc Ctrl knob `0..=1`.
///
/// VERIFIED (chain): `s5 = clip(w, 0.5) + 0.5` with `w = DM(0xC271A)`
/// (`docs/research/synth-stage-models-6.md` s1, poke probe: 0.3 gives 0.8) and the ARM writes
/// `-0.5 + knob / 127` for `kOscCtrl` (`docs/research/synth-knob-laws.md`), so `s5 = knob / 127`:
/// scale 1.0. The SUPER knob scale `SUPER_SPREAD_MAX = 0.1` (ASSUMED) of the placeholder stages
/// is therefore replaced by 1.0 in the verified-words mode. The link between the written word
/// `DM(0xE02EE)` and the persistent source `DM(0xC271A)` is not run end to end (ASSUMED, the
/// SHAPE clips fit `shape = ctrl` with the same word).
#[inline]
pub fn spread_word(ctrl: f32) -> f32 {
    ctrl.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_unison_stages_have_stage_words() {
        assert!(StageWords::for_wave(0, 2).is_none());
        assert!(StageWords::for_wave(3, 0).is_none());
        assert!(StageWords::for_wave(6, 5).is_none());
        for (c, w) in [(5, 0), (5, 5), (6, 0), (6, 1), (6, 2), (6, 4)] {
            assert!(StageWords::for_wave(c, w).is_some(), "({c},{w})");
        }
    }

    #[test]
    fn super_zero_carries_sixteen_ratios_and_equal_weights() {
        let s = StageWords::for_wave(6, 0).unwrap();
        assert!((s.gain - 0.088_388_35).abs() < 1e-7);
        assert!((s.ratios[0][0] - 0.997_116).abs() < 1e-6);
        assert!((s.ratios[1][0] - 1.002_892).abs() < 1e-6);
        assert!((s.ratios[0][7] - 1.044_274).abs() < 1e-6);
        assert!((s.ratios[1][7] - 0.957_603).abs() < 1e-6);
        let w = std::f32::consts::FRAC_1_SQRT_2;
        assert!(s.own.iter().chain(&s.cross).all(|x| (x - w).abs() < 1e-6));
        // sixteen voices of weight P[16] x 0.7071 = 1 / 16 each: the sum is one
        let sum: f32 = s.gain * (s.own.iter().sum::<f32>() + s.cross.iter().sum::<f32>());
        assert!((sum - 1.0).abs() < 1e-5, "{sum}");
    }

    #[test]
    fn multi_wave_zero_is_two_saws_and_the_record() {
        let s = StageWords::for_wave(5, 0).unwrap();
        assert_eq!(s.octaves, [1.0, 1.0, 0.1]);
        assert_eq!((s.own[0], s.own[1], s.own[2]), (1.0, 0.85, 0.0));
        assert_eq!(s.cross[..3], [0.0; 3]);
        assert!((s.ratios[0][1] - 1.008_702).abs() < 1e-6);
        assert!((s.ratios[1][1] - 0.991_373).abs() < 1e-6);
        let two = StageWords::for_wave(5, 4).unwrap();
        assert_eq!(two.octaves, [2.0, 4.0, 1.0]);
    }

    #[test]
    fn spread_word_is_the_knob() {
        assert_eq!(spread_word(0.0), 0.0);
        assert_eq!(spread_word(0.5), 0.5);
        assert_eq!(spread_word(2.0), 1.0);
    }
}
