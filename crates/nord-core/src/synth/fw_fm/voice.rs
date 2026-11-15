//! Opt-in FM voice: pitch words, P words and the band table of a strip type 49..53 wired to the
//! stages [`fm_stage1`] / [`fm_stage2`].
//!
//! Spec `docs/specs/synth-engine.md` section 4.5, evidence `docs/research/fm-banks-1-2.md` s5.
//! One call renders four samples at the SY frame rate `F_SY = 43402.34375 Hz` (both lanes; lane 0
//! is the x output). Nothing here is reachable from the default render path (D22).
//!
//! What is wired (all from committed numbers):
//!
//! | Stage input | Source | Status |
//! |---|---|---|
//! | `ratio`, `weights`, `gain` | P words `P[-32..-29]`, `P[-16..-9]`, `P[16..17]` of the type (`p_words.rs`) | VERIFIED (run) |
//! | `arm[12]` | ARM words `DM(0xC1599 ..)` = `P[-257 ..= -246]` of the type | VERIFIED (run) |
//! | `p0` | `f / F_SY`, `f = 440 2^((midi - 69) / 12)` (0.0060277 at MIDI 60) | VERIFIED (note term `DM(0xC2716) = 0` gives 0.006028, run) |
//! | `pos` (W) | `semi 2^24` clipped to `+-0x58000000` (`DM(0xC1478)`), `semi = midi - 60` (zero at MIDI 60; the DSP writes `DM(0xC2716) = (midi - 60) 2^24` from the controller word, run) | VERIFIED |
//! | `x` | image constant `0.2` (`DM(I2-5)`) | ASSUMED: modulated by the performance in hardware |
//! | band table | `fm_table_for(type, a4, a8)` of the wave (`generator.rs`, bit exact); `FmVoice::select(bank, category, wave)` | VERIFIED (run, 30 of 30 variants) |

use super::generator::{fm_table_for, partial_words};
use super::{fm_stage1, fm_stage2, fm_table, FmInputs, FmState, FmTable};
use crate::synth::p_words::{words_of_type, PWords};

/// VERIFIED: SY frame rate `44,444,000 / 1024` (ARM literal `0x47298A58`, `organ/lane_mix.rs`).
pub const F_SY: f64 = 43_402.343_75;
/// ASSUMED: modulation depth source `DM(I2-5)`, the value of the loaded image (0.2).
pub const X_DEFAULT: f32 = 0.2;
/// VERIFIED (pitch stage listing `0x24C4FC`): one semitone in the position word.
pub const W_PER_SEMITONE: f64 = 16_777_216.0;
/// VERIFIED (`docs/research/synth-stage-equations.md` s pitch sum, `DM(0xC1478)`): clip of the pitch sum.
pub const W_CLIP: i64 = 0x5800_0000;
/// Lowest ARM word index `P[-257]` of the `DM(I1-29 ..)` block.
const ARM_FIRST: i32 = -257;
const FIRST_FM_TYPE: u8 = 49;
const LAST_FM_TYPE: u8 = 53;

/// Position word `W` of a note (`semi` semitones above MIDI 60): `semi 2^24` clipped to `+-0x58000000`.
pub fn position_word(semi: f32) -> u32 {
    let v = (f64::from(semi) * W_PER_SEMITONE).round() as i64;
    v.clamp(-W_CLIP, W_CLIP) as i32 as u32
}

/// Pitch term `p0 = f / F_SY` of a (fractional) MIDI note.
pub fn pitch_term(midi: f32) -> f32 {
    let f = 440.0 * ((f64::from(midi) - 69.0) / 12.0).exp2();
    (f / F_SY) as f32
}

/// Static stage inputs of strip type `ty` (49..=53) from its P words; `None` without capture.
pub fn inputs_for(ty: u8) -> Option<FmInputs> {
    let p: &PWords = words_of_type(usize::from(ty))?;
    fm_table(ty)?;
    let g = |i: i32| p.get(i).unwrap_or(0.0);
    Some(FmInputs {
        p0: [0.0; 2],
        ratio: [[g(-32), g(-31)], [g(-30), g(-29)]],
        inc_scale: 31,
        pos: [0; 2],
        x: [X_DEFAULT; 2],
        arm: std::array::from_fn(|k| g(ARM_FIRST + k as i32)),
        weights: std::array::from_fn(|k| g(-16 + k as i32)),
        gain: [g(16), g(17)],
    })
}

/// One FM voice: type, static words, phase state.
#[derive(Debug, Clone)]
pub struct FmVoice {
    ty: u8,
    inp: FmInputs,
    table: FmTable,
    /// Oscillator phases.
    pub state: FmState,
}

impl FmVoice {
    /// New voice of strip type `ty` (49..=53); `None` for other types.
    pub fn new(ty: u8) -> Option<Self> {
        Some(Self {
            ty,
            inp: inputs_for(ty)?,
            table: *fm_table(ty)?,
            state: FmState::default(),
        })
    }

    /// Voice of an FM oscillator selection: `bank` 1 (`FM-H`) or 2 (`FM-I`), `category` 0..=4 (strip
    /// type `49 + category`), `wave` the item of the bank (`partial_words`).
    pub fn select(bank: u8, category: u8, wave: u8) -> Option<Self> {
        let ty = FIRST_FM_TYPE
            .checked_add(category)
            .filter(|t| *t <= LAST_FM_TYPE)?;
        let (a4, a8) = partial_words(bank, wave)?;
        let mut v = Self::new(ty)?;
        v.table = fm_table_for(ty, a4, a8)?;
        Some(v)
    }

    /// Sets the note (`midi` may be fractional); W and `p0` follow the pitch stage.
    pub fn set_note(&mut self, midi: f32) {
        let w = position_word(midi - 60.0);
        self.inp.pos = [w; 2];
        self.inp.p0 = [pitch_term(midi); 2];
    }

    /// Four samples of both lanes (`out[k][lane]`). Types 49 and 53 run stage 1, 50..=52 stage 2.
    pub fn render4(&mut self) -> [[f32; 2]; 4] {
        match self.ty {
            49 | 53 => fm_stage1(&mut self.state, &self.inp, &self.table),
            _ => fm_stage2(&mut self.state, &self.inp, &self.table),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_word_is_zero_at_midi_60_and_one_semitone_is_2_pow_24() {
        assert_eq!(position_word(0.0), 0);
        assert_eq!(position_word(1.0), 1 << 24);
        assert_eq!(position_word(-1.0), (-(1i32 << 24)) as u32);
        assert_eq!(position_word(1000.0), 0x5800_0000);
        assert_eq!(position_word(-1000.0), (-0x5800_0000i32) as u32);
    }

    #[test]
    fn pitch_term_of_midi_60_matches_the_captured_0_006028() {
        assert!((pitch_term(60.0) - 0.006_028).abs() < 2e-6);
        assert!((pitch_term(72.0) / pitch_term(60.0) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn inputs_carry_the_type_words_and_only_types_49_to_53_build() {
        for ty in 49..=53 {
            let inp = inputs_for(ty).expect("captured type");
            assert!(inp.gain[0] > 0.0, "type {ty} has P[16]");
            assert!(inp.arm.iter().any(|v| *v != 0.0));
        }
        assert!(FmVoice::new(48).is_none() && FmVoice::new(54).is_none());
    }

    #[test]
    fn voice_renders_finite_audio_and_advances_phase() {
        for ty in 49..=53 {
            let mut v = FmVoice::new(ty).unwrap();
            v.set_note(60.0);
            let a = v.render4();
            let b = v.render4();
            assert!(a.iter().chain(&b).flatten().all(|s| s.is_finite()));
            assert_ne!(a, b, "type {ty} phases advance");
        }
    }

    #[test]
    fn rendering_is_deterministic_and_note_dependent() {
        let run = |m: f32| {
            let mut v = FmVoice::new(49).unwrap();
            v.set_note(m);
            (0..8).flat_map(|_| v.render4()).collect::<Vec<_>>()
        };
        assert_eq!(run(60.0), run(60.0));
        assert_ne!(run(60.0), run(72.0));
    }

    #[test]
    fn select_builds_the_table_of_the_wave() {
        let h = FmVoice::select(1, 0, 8).expect("FM-H 8");
        let want = fm_table(49).unwrap();
        assert!(h.table.iter().zip(want).all(|(a, b)| a.abs_diff(*b) <= 1));
        let i = FmVoice::select(2, 2, 12).expect("FM-I 12");
        assert_ne!(i.table, h.table);
        assert!(FmVoice::select(1, 5, 0).is_none());
        assert!(FmVoice::select(1, 0, 25).is_none());
        assert!(FmVoice::select(3, 0, 0).is_none());
    }

    #[test]
    fn partial_ratio_changes_the_audio() {
        let run = |wave: u8| {
            let mut v = FmVoice::select(1, 0, wave).unwrap();
            v.set_note(60.0);
            (0..16).flat_map(|_| v.render4()).collect::<Vec<_>>()
        };
        assert_ne!(run(2), run(9));
    }
}
