//! Library drawer loads start from an Init Program (the hardware picks a sound in a fresh
//! program), unless "Merge into current program" is ticked.
//!
//! Before this, clicking a sound in the Pianos / Samples tab only swapped that layer's sound and
//! left the rest of the previous state alone: the start-up Organ A (level 0.85) kept sounding
//! under every piano, and a synth sample landed on top of whatever the last program had on.

use super::BackupLibraryState;
use crate::audio::{AudioCommand, AudioHandle};
use crate::panel::Stage4PanelState;
use nord_core::cbin::ProgramDecoded;

/// The layer a library sound is loaded into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitTarget {
    /// Piano layer A (`false`) or B (`true`).
    Piano(bool),
    /// Synth layer `0..=2` (A, B, C).
    Synth(usize),
}

/// Init Program with only `target`'s section and layer on: the other sections are off, every
/// effect and level is at its default, scene II equals scene I.
///
/// ASSUMED: the hardware Program Init plays Piano A only (`Stage4Engine::layer_init(All)`,
/// manual p.43); an init with the synth or piano B layer alone is the same program with that layer
/// moved, as a sound picked in a fresh program there.
pub fn init_program(target: InitTarget) -> ProgramDecoded {
    let mut prog = ProgramDecoded::default();
    let (section, layer) = match target {
        InitTarget::Piano(b) => (0b010u8, 2 + u8::from(b)),
        InitTarget::Synth(i) => (0b001u8, 4 + i.min(2) as u8),
    };
    prog.global.section_enable_i = section;
    prog.global.section_enable_ii = section;
    prog.global.scene_1_mask = 1 << layer;
    prog.global.scene_2_mask = 1 << layer;
    prog.organ.layer_a_enabled = false;
    prog.organ.layer_b_enabled = false;
    prog.piano.layer_a.enabled = layer == 2;
    prog.piano.layer_b.enabled = layer == 3;
    prog.synth.layer_a.enabled = layer == 4;
    prog.synth.layer_b.enabled = layer == 5;
    prog.synth.layer_c.enabled = layer == 6;
    prog
}

impl BackupLibraryState {
    /// Replaces the running program with [`init_program`] before a drawer load, unless
    /// `merge_into_program` is set. Panel and audio thread get the same program.
    pub(super) fn init_before_load(
        &mut self,
        target: InitTarget,
        panel: &mut Stage4PanelState,
        audio: &AudioHandle,
    ) {
        if self.merge_into_program {
            return;
        }
        let prog = init_program(target);
        panel.apply_program(&prog);
        audio.send_command(AudioCommand::LoadProgram(Box::new(prog)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_program_has_only_the_target_layer_on() {
        let p = init_program(InitTarget::Piano(false));
        assert_eq!(p.global.scene_1_mask, 1 << 2);
        assert_eq!(p.global.scene_2_mask, 1 << 2);
        assert!(!p.organ.layer_a_enabled && p.piano.layer_a.enabled && !p.synth.layer_a.enabled);
        let b = init_program(InitTarget::Piano(true));
        assert_eq!(b.global.scene_1_mask, 1 << 3);
        let s = init_program(InitTarget::Synth(2));
        assert_eq!(
            (s.global.scene_1_mask, s.global.section_enable_i),
            (1 << 6, 0b001)
        );
        assert!(s.synth.layer_c.enabled && !s.piano.layer_a.enabled);
    }

    #[test]
    fn drawer_load_replaces_the_panel_unless_merging() {
        let (audio, rx) = AudioHandle::new();
        let mut lib = BackupLibraryState::new();
        let mut panel = Stage4PanelState::default();
        assert!(panel.organ_a_active);
        lib.merge_into_program = true;
        lib.init_before_load(InitTarget::Piano(false), &mut panel, &audio);
        assert!(panel.organ_a_active, "merge keeps the program");
        assert!(rx.try_recv().is_err());
        lib.merge_into_program = false;
        lib.init_before_load(InitTarget::Piano(false), &mut panel, &audio);
        assert!(!panel.organ_a_active && panel.piano_a_active);
        assert!(matches!(rx.try_recv(), Ok(AudioCommand::LoadProgram(_))));
    }
}
