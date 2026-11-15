//! Computer-keyboard playing: the pure key map (no egui state, no audio).
//!
//! Layout is the "tracker / DAW" one (Ableton, GarageBand, Reaper virtual keyboard) on physical
//! key positions, so it is the same on QWERTY, ABNT2 and other layouts: egui reports the
//! layout-independent `physical_key` (the key a US keyboard would have at that spot), which the
//! driver in `computer_kb.rs` prefers over the layout-dependent logical `key`.
//!
//! ```text
//!   W E   T Y U   O P        black keys   C# D#  F# G# A#  C# D#
//!  A S D F G H J K L ; '     white keys   C  D E F  G  A  B C  D  E  F
//!  Z X = octave - / +   C V = velocity - / +   Space = sustain pedal
//! ```
//!
//! ASSUMED: this is the de-facto DAW layout (A = C, W = C#, ... ' = F one octave up); no Nord
//! source defines it. The velocity step of 8 is a UI choice (88, the mouse velocity, is on the grid).

use egui::Key;

/// Lowest and highest base octave (C4 = MIDI 60 is the middle C and the default base).
pub const MIN_OCTAVE: i8 = 0;
pub const MAX_OCTAVE: i8 = 7;
pub const DEFAULT_OCTAVE: i8 = 4;
/// Default velocity: the same as a mouse click on the on-screen keybed (`keybed::MOUSE_VELOCITY`).
pub const DEFAULT_VELOCITY: u8 = 88;
/// Velocity change of one C / V press.
pub const VELOCITY_STEP: u8 = 8;

/// Playing keys and their semitone offset above the base C (white row, then the black row).
pub const NOTE_KEYS: [(Key, u8); 18] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
    (Key::Semicolon, 16),
    (Key::Quote, 17),
];

/// What a computer key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Play the key at this index of [`NOTE_KEYS`].
    Note(usize),
    OctaveDown,
    OctaveUp,
    VelocityDown,
    VelocityUp,
    /// Sustain pedal (CC 64).
    Sustain,
}

/// Action bound to a physical key, if any.
pub fn action(key: Key) -> Option<KeyAction> {
    if let Some(i) = NOTE_KEYS.iter().position(|(k, _)| *k == key) {
        return Some(KeyAction::Note(i));
    }
    match key {
        Key::Z => Some(KeyAction::OctaveDown),
        Key::X => Some(KeyAction::OctaveUp),
        Key::C => Some(KeyAction::VelocityDown),
        Key::V => Some(KeyAction::VelocityUp),
        Key::Space => Some(KeyAction::Sustain),
        _ => None,
    }
}

/// MIDI note of `offset` semitones above the C of `octave` (C4 = 60); `None` outside 0..=127.
pub fn midi_note(octave: i8, offset: u8) -> Option<u8> {
    let n = (i16::from(octave) + 1) * 12 + i16::from(offset);
    u8::try_from(n).ok().filter(|n| *n <= 127)
}

/// Octave after `delta` steps, kept in `MIN_OCTAVE..=MAX_OCTAVE`.
pub fn step_octave(octave: i8, delta: i8) -> i8 {
    octave.saturating_add(delta).clamp(MIN_OCTAVE, MAX_OCTAVE)
}

/// Velocity after `steps` presses of C (negative) or V (positive), kept in `1..=127`.
pub fn step_velocity(velocity: u8, steps: i8) -> u8 {
    let v = i16::from(velocity) + i16::from(steps) * i16::from(VELOCITY_STEP);
    v.clamp(1, 127) as u8
}

/// `true` when `key` is a playing / control key, so it also has to give way as a view shortcut
/// (P panorama, Z zoom, L library) while the computer keyboard is on.
pub fn is_claimed(key: Key) -> bool {
    action(key).is_some()
}

/// View / library shortcut test. While the computer keyboard is on (`claimed`), keys that also
/// play need Shift held (Shift+P, Shift+Z, Shift+L); everything else needs no modifier.
pub fn shortcut_pressed(i: &egui::InputState, key: Key, claimed: bool) -> bool {
    if !i.key_pressed(key) {
        return false;
    }
    let m = i.modifiers;
    if claimed && is_claimed(key) {
        m.shift && !m.ctrl && !m.alt && !m.command
    } else {
        !m.any()
    }
}

/// Memory slot the driver publishes "the computer keyboard is on" in, for the shortcut handlers.
fn claim_id() -> egui::Id {
    egui::Id::new("ns4_computer_keyboard_claims_keys")
}

/// Publishes whether the computer keyboard currently owns the playing keys.
pub fn set_claimed(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(claim_id(), on));
}

/// `true` while the computer keyboard owns the playing keys (see [`shortcut_pressed`]).
pub fn claimed(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(claim_id()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, RawInput};

    fn semitones(keys: &[Key]) -> Vec<u8> {
        keys.iter()
            .map(|k| NOTE_KEYS.iter().find(|(x, _)| x == k).unwrap().1)
            .collect()
    }

    #[test]
    fn white_row_is_c_major_from_a_to_quote() {
        let white = [
            Key::A,
            Key::S,
            Key::D,
            Key::F,
            Key::G,
            Key::H,
            Key::J,
            Key::K,
            Key::L,
            Key::Semicolon,
            Key::Quote,
        ];
        assert_eq!(semitones(&white), [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17]);
    }

    #[test]
    fn black_row_sits_on_the_sharps() {
        let black = [Key::W, Key::E, Key::T, Key::Y, Key::U, Key::O, Key::P];
        assert_eq!(semitones(&black), [1, 3, 6, 8, 10, 13, 15]);
    }

    #[test]
    fn every_key_has_exactly_one_action() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in NOTE_KEYS {
            assert!(seen.insert(k), "{k:?} mapped twice");
            assert!(matches!(action(k), Some(KeyAction::Note(_))));
        }
        for k in [Key::Z, Key::X, Key::C, Key::V, Key::Space] {
            assert!(seen.insert(k), "{k:?} collides with a note key");
            assert!(action(k).is_some());
        }
        assert_eq!(action(Key::Q), None);
        assert_eq!(action(Key::Num1), None);
        assert_eq!(action(Key::Escape), None);
    }

    #[test]
    fn control_keys_map_to_their_actions() {
        assert_eq!(action(Key::Z), Some(KeyAction::OctaveDown));
        assert_eq!(action(Key::X), Some(KeyAction::OctaveUp));
        assert_eq!(action(Key::C), Some(KeyAction::VelocityDown));
        assert_eq!(action(Key::V), Some(KeyAction::VelocityUp));
        assert_eq!(action(Key::Space), Some(KeyAction::Sustain));
    }

    #[test]
    fn midi_note_uses_c4_is_60_and_stops_at_127() {
        assert_eq!(midi_note(4, 0), Some(60));
        assert_eq!(midi_note(4, 17), Some(77));
        assert_eq!(midi_note(0, 0), Some(12));
        assert_eq!(midi_note(MAX_OCTAVE, 17), Some(113));
        assert_eq!(midi_note(9, 17), None, "past MIDI 127");
        assert_eq!(midi_note(-2, 0), None, "below MIDI 0");
    }

    #[test]
    fn octave_and_velocity_steps_clamp() {
        assert_eq!(step_octave(4, 1), 5);
        assert_eq!(step_octave(MAX_OCTAVE, 1), MAX_OCTAVE);
        assert_eq!(step_octave(MIN_OCTAVE, -1), MIN_OCTAVE);
        assert_eq!(step_velocity(88, 1), 96);
        assert_eq!(step_velocity(88, -1), 80);
        assert_eq!(step_velocity(124, 1), 127);
        assert_eq!(
            step_velocity(4, -1),
            1,
            "never 0, a Note On at 0 is a Note Off"
        );
        assert_eq!(DEFAULT_VELOCITY % VELOCITY_STEP, 0);
    }

    fn shortcut(key: Key, mods: Modifiers, claimed: bool) -> bool {
        let ctx = egui::Context::default();
        let input = RawInput {
            modifiers: mods,
            events: vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            }],
            ..Default::default()
        };
        let mut hit = false;
        let _ = ctx.run(input, |ctx| {
            hit = ctx.input(|i| shortcut_pressed(i, key, claimed));
        });
        hit
    }

    #[test]
    fn view_shortcuts_need_shift_while_the_keys_play() {
        for k in [Key::P, Key::Z, Key::L] {
            assert!(shortcut(k, Modifiers::NONE, false), "{k:?} plain when off");
            assert!(!shortcut(k, Modifiers::NONE, true), "{k:?} plays when on");
            assert!(shortcut(k, Modifiers::SHIFT, true), "Shift+{k:?} when on");
            assert!(!shortcut(k, Modifiers::CTRL | Modifiers::SHIFT, true));
        }
        assert!(
            shortcut(Key::Num1, Modifiers::NONE, true),
            "digits stay free"
        );
        assert!(!shortcut(Key::Num1, Modifiers::SHIFT, true));
    }

    #[test]
    fn claim_flag_defaults_off_and_round_trips() {
        let ctx = egui::Context::default();
        assert!(!claimed(&ctx));
        set_claimed(&ctx, true);
        assert!(claimed(&ctx));
        set_claimed(&ctx, false);
        assert!(!claimed(&ctx));
    }
}
