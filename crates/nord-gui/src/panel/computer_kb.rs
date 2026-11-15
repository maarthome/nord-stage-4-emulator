//! Computer-keyboard playing: state, per-frame driver and the "Computer keyboard" control bar.
//!
//! The key map is in `computer_keys.rs`. Notes go through `AudioHandle::send_keyboard_midi`, the
//! same path as the on-screen keybed (so Local Control Off keeps them silent, and they never pass
//! the MIDI-in thread). Key down is a Note On, key up a Note Off, auto-repeat is ignored, and
//! everything is released when the window loses focus, a text field takes the keyboard or the
//! feature is switched off.

use super::computer_keys::{
    self as map, action, midi_note, KeyAction, DEFAULT_OCTAVE, DEFAULT_VELOCITY, NOTE_KEYS,
};
use super::prelude::*;
use egui::{Event, Key};
use nord_core::midi::cc;

/// State of the computer keyboard (kept in `Stage4PanelState`).
#[derive(Debug, Clone)]
pub struct ComputerKeyboard {
    /// Master switch (default on). Off hands P / Z / L back to the view shortcuts.
    pub enabled: bool,
    /// Octave of the A key; C4 = MIDI 60.
    pub octave: i8,
    /// Note On velocity, 1..=127 (default 88 = a mouse click on the keybed).
    pub velocity: u8,
    sustain: bool,
    /// MIDI note each playing key started, so a later octave change cannot orphan it.
    held: [Option<u8>; NOTE_KEYS.len()],
}

impl Default for ComputerKeyboard {
    fn default() -> Self {
        Self {
            enabled: true,
            octave: DEFAULT_OCTAVE,
            velocity: DEFAULT_VELOCITY,
            sustain: false,
            held: [None; NOTE_KEYS.len()],
        }
    }
}

/// One key event the driver feeds the state machine.
#[derive(Debug, Clone, Copy)]
pub struct KeyInput {
    /// Physical key (layout independent).
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
    /// No Shift / Ctrl / Alt / Command held; only plain presses play.
    pub plain: bool,
}

impl ComputerKeyboard {
    /// `true` while a computer key holds `note` down.
    pub fn is_sounding(&self, note: u8) -> bool {
        self.held.contains(&Some(note))
    }

    /// `true` while the space bar holds the sustain pedal down.
    pub fn sustain_down(&self) -> bool {
        self.sustain
    }

    /// Number of notes currently held.
    pub fn held_count(&self) -> usize {
        self.held.iter().flatten().count()
    }

    /// Applies one key event and appends the MIDI it produces to `out`.
    /// Returns `true` when the key belongs to the computer keyboard.
    pub fn on_key(&mut self, k: KeyInput, out: &mut Vec<MidiEvent>) -> bool {
        let Some(act) = action(k.key) else {
            return false;
        };
        if k.repeat {
            return true;
        }
        match act {
            KeyAction::Note(i) if k.pressed => {
                if let (true, None, Some(n)) = (
                    k.plain,
                    self.held[i],
                    midi_note(self.octave, NOTE_KEYS[i].1),
                ) {
                    self.held[i] = Some(n);
                    out.push(MidiEvent::NoteOn {
                        channel: 0,
                        note: n,
                        velocity: self.velocity,
                    });
                }
            }
            KeyAction::Note(i) => {
                if let Some(n) = self.held[i].take() {
                    out.push(note_off(n));
                }
            }
            KeyAction::Sustain if k.pressed => {
                if k.plain && !self.sustain {
                    self.sustain = true;
                    out.push(pedal(127));
                }
            }
            KeyAction::Sustain => {
                if self.sustain {
                    self.sustain = false;
                    out.push(pedal(0));
                }
            }
            _ if !k.pressed || !k.plain => {}
            KeyAction::OctaveDown => self.octave = map::step_octave(self.octave, -1),
            KeyAction::OctaveUp => self.octave = map::step_octave(self.octave, 1),
            KeyAction::VelocityDown => self.velocity = map::step_velocity(self.velocity, -1),
            KeyAction::VelocityUp => self.velocity = map::step_velocity(self.velocity, 1),
        }
        true
    }

    /// Releases every held note and the pedal.
    pub fn release_all(&mut self, out: &mut Vec<MidiEvent>) {
        for slot in &mut self.held {
            if let Some(n) = slot.take() {
                out.push(note_off(n));
            }
        }
        if std::mem::take(&mut self.sustain) {
            out.push(pedal(0));
        }
    }
}

fn note_off(note: u8) -> MidiEvent {
    MidiEvent::NoteOff {
        channel: 0,
        note,
        velocity: 0,
    }
}

fn pedal(value: u8) -> MidiEvent {
    MidiEvent::ControlChange {
        channel: 0,
        controller: cc::SUSTAIN_PEDAL,
        value,
    }
}

/// Reads this frame's key events and plays / releases through the keyboard path. Call once per
/// frame before the shortcut handlers (it publishes whether the playing keys are claimed).
pub fn drive(ctx: &egui::Context, state: &mut super::Stage4PanelState, audio: &AudioHandle) {
    let (events, focused) = ctx.input(|i| {
        let keys: Vec<KeyInput> = i
            .raw
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat,
                    modifiers,
                } => Some(KeyInput {
                    key: physical_key.unwrap_or(*key),
                    pressed: *pressed,
                    repeat: *repeat,
                    plain: !modifiers.any(),
                }),
                _ => None,
            })
            .collect();
        // Both the platform flag and the focus event count; the last event of the frame wins.
        let event = i.raw.events.iter().rev().find_map(|e| match e {
            Event::WindowFocused(f) => Some(*f),
            _ => None,
        });
        (keys, event.unwrap_or(i.focused))
    });
    let kb = &mut state.computer_kb;
    let active = kb.enabled && focused && !ctx.wants_keyboard_input();
    map::set_claimed(ctx, kb.enabled);
    let mut out = Vec::new();
    if active {
        for k in events {
            kb.on_key(k, &mut out);
        }
    } else {
        kb.release_all(&mut out);
    }
    for ev in out {
        if let MidiEvent::NoteOn { note, .. } = ev {
            // SET KEY armed: the next key sets the split point, like a mouse press.
            for c in state.program_ui.key_pressed(note) {
                audio.send_command(c);
            }
        }
        audio.send_keyboard_midi(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(kb: &mut ComputerKeyboard, key: Key, out: &mut Vec<MidiEvent>) {
        let k = |pressed| KeyInput {
            key,
            pressed,
            repeat: false,
            plain: true,
        };
        kb.on_key(k(true), out);
    }

    fn release(kb: &mut ComputerKeyboard, key: Key, out: &mut Vec<MidiEvent>) {
        let k = KeyInput {
            key,
            pressed: false,
            repeat: false,
            plain: true,
        };
        kb.on_key(k, out);
    }

    #[test]
    fn defaults_are_on_c4_at_the_mouse_velocity() {
        let kb = ComputerKeyboard::default();
        assert!(kb.enabled);
        assert_eq!((kb.octave, kb.velocity), (4, 88));
        assert_eq!(kb.held_count(), 0);
    }

    #[test]
    fn key_down_is_note_on_and_key_up_is_note_off() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        press(&mut kb, Key::A, &mut out);
        assert_eq!(
            out,
            [MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 88
            }]
        );
        assert!(kb.is_sounding(60));
        out.clear();
        release(&mut kb, Key::A, &mut out);
        assert_eq!(out, [note_off(60)]);
        assert!(!kb.is_sounding(60));
    }

    #[test]
    fn repeat_and_second_down_do_not_retrigger() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        press(&mut kb, Key::A, &mut out);
        let rep = KeyInput {
            key: Key::A,
            pressed: true,
            repeat: true,
            plain: true,
        };
        assert!(kb.on_key(rep, &mut out), "still a computer key");
        press(&mut kb, Key::A, &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn octave_change_while_held_releases_the_original_note() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        press(&mut kb, Key::A, &mut out);
        press(&mut kb, Key::X, &mut out);
        assert_eq!(kb.octave, 5);
        release(&mut kb, Key::A, &mut out);
        assert_eq!(out.last(), Some(&note_off(60)));
        press(&mut kb, Key::A, &mut out);
        assert!(matches!(
            out.last(),
            Some(MidiEvent::NoteOn { note: 72, .. })
        ));
    }

    #[test]
    fn velocity_keys_scale_the_next_note_on() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        press(&mut kb, Key::V, &mut out);
        press(&mut kb, Key::S, &mut out);
        assert!(matches!(
            out.last(),
            Some(MidiEvent::NoteOn {
                note: 62,
                velocity: 96,
                ..
            })
        ));
        for _ in 0..30 {
            press(&mut kb, Key::C, &mut out);
        }
        assert_eq!(kb.velocity, 1);
    }

    #[test]
    fn space_is_cc64_down_and_up_once() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        press(&mut kb, Key::Space, &mut out);
        press(&mut kb, Key::Space, &mut out);
        release(&mut kb, Key::Space, &mut out);
        assert_eq!(out, [pedal(127), pedal(0)]);
    }

    #[test]
    fn modified_presses_do_not_play_but_releases_still_work() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        let ctrl_a = KeyInput {
            key: Key::A,
            pressed: true,
            repeat: false,
            plain: false,
        };
        assert!(kb.on_key(ctrl_a, &mut out));
        assert!(out.is_empty());
        press(&mut kb, Key::A, &mut out);
        let up_with_shift = KeyInput {
            pressed: false,
            plain: false,
            ..ctrl_a
        };
        kb.on_key(up_with_shift, &mut out);
        assert_eq!(out.last(), Some(&note_off(60)), "no stuck note");
    }

    #[test]
    fn release_all_frees_notes_and_pedal() {
        let mut kb = ComputerKeyboard::default();
        let mut out = Vec::new();
        for k in [Key::A, Key::D, Key::G, Key::Space] {
            press(&mut kb, k, &mut out);
        }
        out.clear();
        kb.release_all(&mut out);
        assert_eq!(out.len(), 4);
        assert_eq!(kb.held_count(), 0);
        assert!(!kb.sustain_down());
        out.clear();
        kb.release_all(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn notes_past_127_do_not_sound() {
        let mut kb = ComputerKeyboard {
            octave: 9,
            ..Default::default()
        };
        let mut out = Vec::new();
        press(&mut kb, Key::Quote, &mut out);
        assert!(out.is_empty());
        assert_eq!(kb.held_count(), 0);
    }

    #[test]
    fn unrelated_keys_are_not_ours() {
        let mut kb = ComputerKeyboard::default();
        let k = KeyInput {
            key: Key::Q,
            pressed: true,
            repeat: false,
            plain: true,
        };
        assert!(!kb.on_key(k, &mut Vec::new()));
    }
}
