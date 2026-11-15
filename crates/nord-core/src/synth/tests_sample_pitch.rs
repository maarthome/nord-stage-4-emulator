//! `sample_pitch_fw`: the note-to-step ratio of the `SAMPLES` oscillator (opt-in firmware join of
//! the zone word and the modelled table entry, `sinc::step_join`).

use super::*;
use std::sync::Arc;

fn step(fw: bool, note: u8) -> f64 {
    let pcm: Vec<f32> = (0..512).map(|i| (i as f32 / 32.0).sin()).collect();
    let sound = Arc::new(SynthSampleSound::new_synthetic(
        "PitchLaw",
        60,
        &pcm,
        48_000.0,
        Some((128, 384)),
        16,
    ));
    let mut s = SynthLayer::new(48_000.0);
    s.osc_mode = SynthOscMode::Sample;
    s.sample_pitch_fw = fw;
    s.load_sample(Some(sound));
    s.note_on(note, 100);
    s.voices[0].sample_step
}

#[test]
fn firmware_pitch_law_is_off_by_default_and_near_native_at_the_root_key() {
    assert!(!SynthLayer::new(48_000.0).sample_pitch_fw);
    assert!((step(false, 36) - 0.25).abs() < 1e-12);
    // One Q8 tune count (0.23 %) plus the 11-bit entry rounding bound the root-key error.
    assert!((step(true, 60) - 1.0).abs() < 3e-3, "{}", step(true, 60));
}

#[test]
fn firmware_pitch_law_follows_the_semitone_law_within_half_a_percent() {
    for note in [24u8, 36, 60, 72, 96] {
        let (eq, fw) = (step(false, note), step(true, note));
        assert!(
            eq != fw && (fw / eq - 1.0).abs() < 5e-3,
            "{note}: {eq} {fw}"
        );
    }
}

#[test]
fn firmware_step_equals_the_step_join_of_the_zone() {
    let want = crate::sinc::step_join::output_step(48, 60, 48_000.0, 48_000.0);
    assert!((step(true, 48) - want).abs() < 1e-12);
}
