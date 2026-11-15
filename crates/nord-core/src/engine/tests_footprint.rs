//! Stack-footprint budgets of the engine and its large layer types (TD-05).
//!
//! CI runs debug tests on Windows, where test threads have 2 MiB and the main thread 1 MiB of
//! stack, and an unoptimised build holds several copies of every large value a test moves. The
//! layers therefore live in boxes allocated once in `Stage4Engine::new` (never in the audio path).
//! Measured on 2026-11-15 (x86_64, `size_of`): `Stage4Engine` 152,128 B before the boxing, 3,248 B
//! after (the 7 FX chains, the wavetable bank and the reverb banks are heap-built too); `OrganLayer` 18,184; `PianoLayer` 11,736; `SynthLayer` 16,152; `LayerFxChain` 5,808;
//! `SynthVoice` 896. A budget breach means a new field made a value too big to copy by value:
//! box it (allocate in `new()`, replace in place afterwards) instead of raising the number.
use super::*;
use crate::fx::{LayerFxChain, MasterBus};
use crate::synth::SynthVoice;
use std::mem::size_of;

const KIB: usize = 1024;

#[test]
fn engine_and_state_stay_small_values() {
    assert!(
        size_of::<Stage4Engine>() <= 16 * KIB,
        "{}",
        size_of::<Stage4Engine>()
    );
    assert!(
        size_of::<ProgramState>() <= 16 * KIB,
        "{}",
        size_of::<ProgramState>()
    );
    assert!(size_of::<MasterBus>() <= KIB);
    assert!(size_of::<Option<Clip>>() <= KIB);
}

#[test]
fn layer_types_stay_under_their_budgets() {
    assert!(
        size_of::<OrganLayer>() <= 32 * KIB,
        "{}",
        size_of::<OrganLayer>()
    );
    assert!(
        size_of::<PianoLayer>() <= 24 * KIB,
        "{}",
        size_of::<PianoLayer>()
    );
    assert!(
        size_of::<SynthLayer>() <= 32 * KIB,
        "{}",
        size_of::<SynthLayer>()
    );
    assert!(
        size_of::<LayerFxChain>() <= 16 * KIB,
        "{}",
        size_of::<LayerFxChain>()
    );
    assert!(
        size_of::<SynthVoice>() <= 2 * KIB,
        "{}",
        size_of::<SynthVoice>()
    );
}

/// The whole control path (construct, play, capture, apply, clone) fits a 256 KiB stack, a
/// quarter of the smallest CI stack (the 1 MiB Windows main thread).
#[test]
fn engine_lifecycle_runs_on_a_small_stack() {
    let run = || {
        let mut e = Stage4Engine::new(48_000.0);
        e.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        for _ in 0..64 {
            e.render_frame();
        }
        let state = e.capture_state();
        let mut copy = e.clone();
        copy.apply_state(state);
        assert!(copy.render_frame()[0].is_finite());
    };
    std::thread::Builder::new()
        .stack_size(256 * KIB)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
