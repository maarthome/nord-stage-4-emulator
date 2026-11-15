//! `FirmwareRouting`: the engine follows the decoded section / layer enables and KB zones.

use super::*;
use crate::cbin::ProgramDecoded;

const ORGAN_A: u8 = 1 << LayerId::OrganA as u8;
const ORGAN_B: u8 = 1 << LayerId::OrganB as u8;

/// `B3 Chords n Solo` as decoded: organ A + B on in scene I, scene II empty, Mid split at 65,
/// layer A in the lower zone (code 0), layer B in the upper zone (code 6).
fn b3_chords_n_solo() -> ProgramDecoded {
    let mut prog = ProgramDecoded::default();
    prog.global.section_enable_i = 0b100;
    prog.global.section_enable_ii = 0;
    prog.global.scene_1_mask = ORGAN_A | ORGAN_B;
    prog.global.scene_2_mask = 0;
    prog.global.split_enabled = true;
    prog.global.split_zone_enabled = [false, true, false];
    prog.global.split_points = [48, 65, 84];
    prog.global.kb_zone = [0, 6, 5, 5, 5, 5, 5];
    prog
}

fn press(e: &mut Stage4Engine, note: u8) -> u8 {
    e.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note,
        velocity: 100,
    });
    e.note_recipients_for_test(0, note)
}

#[test]
fn off_keeps_the_legacy_routing() {
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&b3_chords_n_solo());
    // legacy: both layers sound everywhere (the zone codes are not read)
    assert_eq!(press(&mut e, 44), ORGAN_A | ORGAN_B);
    assert_eq!(e.firmware_routing(), FirmwareRouting::Off);
}

#[test]
fn on_sends_each_key_only_to_the_layer_of_its_zone() {
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&b3_chords_n_solo());
    e.set_firmware_routing(FirmwareRouting::On);
    assert_eq!(press(&mut e, 44), ORGAN_A);
    assert_eq!(press(&mut e, 64), ORGAN_A);
    assert_eq!(press(&mut e, 65), ORGAN_B);
    assert_eq!(press(&mut e, 90), ORGAN_B);
}

#[test]
fn routing_set_before_the_program_gives_the_same_result() {
    let mut e = Stage4Engine::new(48_000.0);
    e.set_firmware_routing(FirmwareRouting::On);
    e.apply_program(&b3_chords_n_solo());
    assert_eq!(press(&mut e, 50), ORGAN_A);
    assert_eq!(press(&mut e, 70), ORGAN_B);
}

#[test]
fn empty_scene_mask_is_silent_with_firmware_routing_only() {
    let mut prog = ProgramDecoded::default();
    prog.global.scene_1_mask = 0;
    prog.global.section_enable_i = 0;
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&prog);
    assert_ne!(press(&mut e, 60), 0, "legacy falls back to default layers");
    e.set_firmware_routing(FirmwareRouting::On);
    e.handle_midi(MidiEvent::NoteOff {
        channel: 0,
        note: 60,
        velocity: 0,
    });
    assert_eq!(press(&mut e, 61), 0);
}

#[test]
fn enabled_section_without_layers_plays_layer_a_with_firmware_routing() {
    // `Vox Like Me`: organ section on, organ layer enable 0, nothing else (OQ25).
    let mut prog = ProgramDecoded::default();
    prog.global.section_enable_i = 0b100;
    prog.global.scene_1_mask = 0;
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&prog);
    e.set_firmware_routing(FirmwareRouting::On);
    assert_eq!(press(&mut e, 61), ORGAN_A);
}

#[test]
fn empty_section_next_to_a_sounding_one_plays_layer_a() {
    // `Tines Layer`: piano section on with layer enable 0 beside synth A + B (OQ25, VERIFIED:
    // the loader view refresh turns mask 0 into layer A in every section).
    const PIANO_A: u8 = 1 << LayerId::PianoA as u8;
    const SYNTH_AB: u8 = (1 << LayerId::SynthA as u8) | (1 << LayerId::SynthB as u8);
    let mut prog = ProgramDecoded::default();
    prog.global.section_enable_i = 0b011;
    prog.global.scene_1_mask = SYNTH_AB;
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&prog);
    e.set_firmware_routing(FirmwareRouting::On);
    assert_eq!(press(&mut e, 61), PIANO_A | SYNTH_AB);
}

#[test]
fn scene_two_follows_its_own_exact_mask() {
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&b3_chords_n_solo());
    e.set_firmware_routing(FirmwareRouting::On);
    e.scene.scene_2_active = true;
    assert_eq!(press(&mut e, 44), 0, "section II is off in the program");
}

#[test]
fn switching_back_restores_the_legacy_masks() {
    let mut e = Stage4Engine::new(48_000.0);
    let mut prog = b3_chords_n_solo();
    prog.global.scene_2_mask = 0;
    e.apply_program(&prog);
    let legacy = (e.scene.scene_1_mask, e.scene.scene_2_mask);
    e.set_firmware_routing(FirmwareRouting::On);
    assert_eq!(
        (e.scene.scene_1_mask, e.scene.scene_2_mask),
        (ORGAN_A | ORGAN_B, 0)
    );
    e.set_firmware_routing(FirmwareRouting::Off);
    assert_eq!((e.scene.scene_1_mask, e.scene.scene_2_mask), legacy);
}

#[test]
fn direct_layer_channel_bypasses_the_zone() {
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&b3_chords_n_solo());
    e.set_firmware_routing(FirmwareRouting::On);
    e.set_layer_channel(LayerId::OrganB, Some(3));
    e.handle_midi(MidiEvent::NoteOn {
        channel: 2,
        note: 40,
        velocity: 100,
    });
    assert_eq!(e.note_recipients_for_test(2, 40), ORGAN_B);
}

/// `Concert Grand` / `Felt Upright` / `Midnight Upright` as decoded: piano section on with layer A,
/// organ and synth sections off, while the stored organ block still carries a layer A enable
/// (`organ.layer_a_enabled`, 40+ factory pianos do). Hardware: zone bytes `[0, 0, 1, 0, 0, 0, 0]`
/// after the real program load (`tools/arm_program_load.py`, 4 pianos, 2026-11-15).
fn factory_piano() -> ProgramDecoded {
    let mut prog = ProgramDecoded::default();
    prog.organ.layer_a_enabled = true;
    prog.global.section_enable_i = 0b010;
    prog.global.section_enable_ii = 0b010;
    prog.global.scene_1_mask = 1 << LayerId::PianoA as u8;
    prog.global.scene_2_mask = 1 << LayerId::PianoA as u8;
    prog.global.kb_zone = [5; 7];
    prog
}

#[test]
fn a_piano_program_never_reaches_the_organ_in_either_routing_mode() {
    const PIANO_A: u8 = 1 << LayerId::PianoA as u8;
    for mode in [FirmwareRouting::Off, FirmwareRouting::On] {
        let mut e = Stage4Engine::new(48_000.0);
        e.apply_program(&factory_piano());
        e.set_firmware_routing(mode);
        for note in [36, 60, 96] {
            assert_eq!(press(&mut e, note), PIANO_A, "{mode:?} note {note}");
        }
    }
}

#[test]
fn piano_program_masks_are_identical_in_both_modes() {
    let mut e = Stage4Engine::new(48_000.0);
    e.apply_program(&factory_piano());
    let routing = e.program_routing();
    assert_eq!(routing.exact_masks, routing.legacy_masks);
    assert_eq!(routing.exact_masks[0] & 0b11, 0, "no organ bit in scene I");
}

#[test]
fn enabled_organ_at_level_zero_is_routed_but_adds_nothing() {
    // `J3 Grand Layers`: organ section on, layer enable 0 (the loader reads layer A, zone byte 1
    // on the hardware) with layer levels 0.0, beside piano A + B and synth A + B + C. The render is
    // bit-identical to the same program with the organ section off (WAV hashes of the real program
    // match in both routing modes).
    fn render(organ_section: bool) -> (Vec<f32>, Vec<f32>, u8) {
        let mut prog = factory_piano();
        prog.global.section_enable_i = if organ_section { 0b111 } else { 0b011 };
        prog.global.scene_1_mask = 0b0111_1100;
        prog.organ.layer_a_level = 0.0;
        prog.organ.layer_b_level = 0.0;
        let mut e = Stage4Engine::new(48_000.0);
        e.apply_program(&prog);
        e.set_firmware_routing(FirmwareRouting::On);
        let routed = press(&mut e, 60) & ORGAN_A;
        e.handle_midi(MidiEvent::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0,
        });
        let note_on = MidiEvent::NoteOn {
            channel: 0,
            note: 48,
            velocity: 100,
        };
        let events = [crate::render::TimedEvent {
            frame: 0,
            event: note_on,
        }];
        let (l, r) = crate::render::render_timeline(&mut e, &events, 4096);
        (l, r, routed)
    }
    let (on_l, on_r, routed) = render(true);
    let (off_l, off_r, not_routed) = render(false);
    assert_ne!(routed, 0, "organ A is routed");
    assert_eq!(not_routed, 0);
    assert_eq!((on_l, on_r), (off_l, off_r), "organ level 0 adds no sound");
}
