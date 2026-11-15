//! Headless GUI tests of computer-keyboard playing: real egui key events through the full panel,
//! checking the commands that reach the audio queue.

use super::sim::Sim;
use crate::audio::AudioCommand;
use egui::{Event, Key, Modifiers};
use nord_core::engine::Stage4Engine;
use nord_core::midi::MidiEvent;

fn sim() -> Sim {
    let mut s = Sim::full_panel();
    s.state.computer_kb.enabled = true;
    s.frame(vec![], Modifiers::NONE);
    s
}

fn key_event(key: Key, physical: Option<Key>, pressed: bool, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: physical,
        pressed,
        repeat,
        modifiers: Modifiers::NONE,
    }
}

fn tap(s: &mut Sim, key: Key, pressed: bool) {
    s.frame(
        vec![key_event(key, Some(key), pressed, false)],
        Modifiers::NONE,
    );
}

/// Every command queued since the last call.
fn drain(s: &Sim) -> Vec<AudioCommand> {
    s.rx.try_iter().collect()
}

fn keyboard_events(cmds: &[AudioCommand]) -> Vec<MidiEvent> {
    cmds.iter()
        .filter_map(|c| match c {
            AudioCommand::Keyboard(e) => Some(*e),
            _ => None,
        })
        .collect()
}

#[test]
fn a_key_plays_middle_c_and_releases_on_key_up() {
    let mut s = sim();
    let _ = drain(&s);
    tap(&mut s, Key::A, true);
    let on = drain(&s);
    assert_eq!(
        keyboard_events(&on),
        [MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 88
        }]
    );
    assert!(
        on.iter().all(|c| !matches!(c, AudioCommand::Midi(_))),
        "never through the MIDI-in path"
    );
    assert!(s.state.computer_kb.is_sounding(60));
    tap(&mut s, Key::A, false);
    assert!(matches!(
        keyboard_events(&drain(&s)).as_slice(),
        [MidiEvent::NoteOff { note: 60, .. }]
    ));
}

#[test]
fn physical_position_wins_over_the_layout_dependent_key() {
    // A key whose logical meaning differs (another layout) but sits where the US "A" is.
    let mut s = sim();
    let _ = drain(&s);
    s.frame(
        vec![key_event(Key::Q, Some(Key::A), true, false)],
        Modifiers::NONE,
    );
    assert!(matches!(
        keyboard_events(&drain(&s)).as_slice(),
        [MidiEvent::NoteOn { note: 60, .. }]
    ));
}

#[test]
fn auto_repeat_does_not_retrigger() {
    let mut s = sim();
    let _ = drain(&s);
    tap(&mut s, Key::D, true);
    for _ in 0..3 {
        s.frame(
            vec![key_event(Key::D, Some(Key::D), true, true)],
            Modifiers::NONE,
        );
    }
    assert_eq!(keyboard_events(&drain(&s)).len(), 1);
}

#[test]
fn space_is_the_sustain_pedal_and_z_x_move_the_octave() {
    let mut s = sim();
    let _ = drain(&s);
    tap(&mut s, Key::Space, true);
    tap(&mut s, Key::Space, false);
    let pedal = keyboard_events(&drain(&s));
    assert!(matches!(
        pedal.as_slice(),
        [
            MidiEvent::ControlChange {
                controller: 64,
                value: 127,
                ..
            },
            MidiEvent::ControlChange {
                controller: 64,
                value: 0,
                ..
            }
        ]
    ));
    tap(&mut s, Key::X, true);
    tap(&mut s, Key::X, false);
    assert_eq!(s.state.computer_kb.octave, 5);
    tap(&mut s, Key::A, true);
    assert!(matches!(
        keyboard_events(&drain(&s)).as_slice(),
        [MidiEvent::NoteOn { note: 72, .. }]
    ));
}

#[test]
fn losing_window_focus_releases_every_note() {
    let mut s = sim();
    tap(&mut s, Key::A, true);
    tap(&mut s, Key::G, true);
    tap(&mut s, Key::Space, true);
    let _ = drain(&s);
    s.frame(vec![Event::WindowFocused(false)], Modifiers::NONE);
    let ev = keyboard_events(&drain(&s));
    assert_eq!(ev.len(), 3, "two notes and the pedal: {ev:?}");
    assert_eq!(s.state.computer_kb.held_count(), 0);
}

#[test]
fn switching_the_feature_off_releases_and_stops_playing() {
    let mut s = sim();
    tap(&mut s, Key::A, true);
    let _ = drain(&s);
    s.state.computer_kb.enabled = false;
    s.frame(vec![], Modifiers::NONE);
    assert!(matches!(
        keyboard_events(&drain(&s)).as_slice(),
        [MidiEvent::NoteOff { note: 60, .. }]
    ));
    tap(&mut s, Key::S, true);
    assert!(keyboard_events(&drain(&s)).is_empty());
}

#[test]
fn a_focused_text_field_keeps_the_keys_for_typing() {
    let ctx = egui::Context::default();
    let (audio, rx) = crate::audio::AudioHandle::new();
    let mut state = super::Stage4PanelState::default();
    let mut text = String::new();
    let mut time = 1.0;
    let mut frame = |events: Vec<Event>, state: &mut super::Stage4PanelState| {
        time += 0.05;
        let input = egui::RawInput {
            time: Some(time),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = ui.text_edit_singleline(&mut text);
                if !ui.memory(|m| m.focused().is_some()) {
                    r.request_focus();
                }
            });
            super::computer_kb::drive(ctx, state, &audio);
        });
    };
    frame(vec![], &mut state);
    frame(vec![], &mut state);
    frame(
        vec![key_event(Key::A, Some(Key::A), true, false)],
        &mut state,
    );
    assert!(
        keyboard_events(&rx.try_iter().collect::<Vec<_>>()).is_empty(),
        "typing in a text field must not play"
    );
}

#[test]
fn local_control_off_keeps_the_computer_keys_silent_but_midi_in_plays() {
    let mut s = sim();
    let _ = drain(&s);
    tap(&mut s, Key::A, true);
    let cmds = drain(&s);
    let peak = |local: bool| {
        let mut e = Stage4Engine::new(48_000.0);
        e.midi_menu.local_control = local;
        let mut vol = 0.8;
        for c in cmds.iter().cloned() {
            crate::audio::apply_audio_command(c, &mut e, &mut vol);
        }
        (0..2048).fold(0.0f32, |m, _| m.max(e.render_frame()[0].abs()))
    };
    assert_eq!(peak(false), 0.0);
    assert!(peak(true) > 1e-4);
}

#[test]
fn p_z_l_are_notes_while_on_and_shift_p_toggles_the_panorama() {
    let mut s = sim();
    tap(&mut s, Key::P, true);
    tap(&mut s, Key::P, false);
    assert_eq!(s.state.view.panorama, None, "plain P plays");
    let shift = Modifiers::SHIFT;
    let ev = |pressed| Event::Key {
        key: Key::P,
        physical_key: Some(Key::P),
        pressed,
        repeat: false,
        modifiers: shift,
    };
    s.frame(vec![ev(true)], shift);
    s.frame(vec![ev(false)], Modifiers::NONE);
    assert!(s.state.view.panorama.is_some(), "Shift+P toggles the view");
    s.state.computer_kb.enabled = false;
    s.frame(vec![], Modifiers::NONE);
    tap(&mut s, Key::Escape, true);
    tap(&mut s, Key::Escape, false);
    tap(&mut s, Key::P, true);
    tap(&mut s, Key::P, false);
    assert!(s.state.view.panorama.is_some(), "plain P again when off");
}
