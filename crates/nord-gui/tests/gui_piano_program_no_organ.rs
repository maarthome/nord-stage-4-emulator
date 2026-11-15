//! A piano program loaded through the GUI path plays no organ and the panel shows the Organ
//! section off (`docs/gui-testing.md`, `docs/research/engine-routing.md` section 8, OQ33).
//!
//! The GUI applies a program twice: `Stage4PanelState::apply_program` (LEDs) and
//! `AudioCommand::LoadProgram` on the audio thread (`apply_audio_command`, the same function the
//! cpal callback runs). Notes arrive as `AudioCommand::Keyboard`. The organ energy is the RMS of
//! the difference between a render and the same render with the two organ faders at zero.

use nord_core::cbin::{CbinFile, Ns4BackupArchive, ProgramDecoded};
use nord_core::engine::Stage4Engine;
use nord_core::midi::MidiEvent;
use nord_core::piano::NpnoSound;
use nord_core::render_opts::FirmwareOptions;
use nord_gui::audio::{apply_audio_command, standalone_engine, AudioCommand};
use nord_gui::panel::Stage4PanelState;
use std::io::BufReader;
use std::sync::Arc;

const FRAMES: usize = 12_000;

/// Organ / piano section enable bits of `section_enable_*` (bit 2 organ, 1 piano, 0 synth).
const PIANO_ONLY: u8 = 0b010;

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

/// Renders `notes` through the GUI command path; returns the output and the held keys per layer.
fn render(
    prog: &ProgramDecoded,
    piano: &Arc<NpnoSound>,
    opts: FirmwareOptions,
    organ_faders_zero: bool,
    notes: &[u8],
) -> (Vec<f32>, [u8; 7]) {
    let mut eng: Stage4Engine = standalone_engine(48_000.0);
    let mut vol = 0.8;
    let mut send =
        |cmd: AudioCommand, eng: &mut Stage4Engine| apply_audio_command(cmd, eng, &mut vol);
    send(AudioCommand::SetFirmwareOptions(opts), &mut eng);
    send(AudioCommand::LoadProgram(Box::new(prog.clone())), &mut eng);
    send(
        AudioCommand::LoadPianoSound {
            slot_b: false,
            sound: Some(Arc::clone(piano)),
        },
        &mut eng,
    );
    if organ_faders_zero {
        eng.layer_levels[0].base = 0.0;
        eng.layer_levels[1].base = 0.0;
    }
    for &note in notes {
        let ev = MidiEvent::NoteOn {
            channel: 0,
            note,
            velocity: 100,
        };
        send(AudioCommand::Keyboard(ev), &mut eng);
    }
    let out: Vec<f32> = (0..FRAMES).map(|_| eng.render_frame()[0]).collect();
    (out, eng.telemetry().layer_held)
}

/// `(total RMS, organ RMS, held keys)` of `prog` through the GUI path.
fn organ_energy(
    prog: &ProgramDecoded,
    piano: &Arc<NpnoSound>,
    opts: FirmwareOptions,
) -> (f32, f32, [u8; 7]) {
    let notes = [48u8, 60, 64, 67];
    let (with, held) = render(prog, piano, opts, false, &notes);
    let (without, _) = render(prog, piano, opts, true, &notes);
    let diff: Vec<f32> = with.iter().zip(&without).map(|(a, b)| a - b).collect();
    (rms(&with), rms(&diff), held)
}

fn synthetic_piano() -> Arc<NpnoSound> {
    let tone: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    Arc::new(NpnoSound::new_synthetic(
        "Test Upright",
        60,
        &tone,
        48_000.0,
    ))
}

/// A piano program the way the factory stores them: piano section on, Organ layer A still holding
/// its (unused) drawbars, scene II empty.
fn piano_program() -> ProgramDecoded {
    let mut prog = ProgramDecoded::default();
    prog.global.section_enable_i = PIANO_ONLY;
    prog.global.section_enable_ii = PIANO_ONLY;
    prog.global.scene_1_mask = 1 << 2;
    prog.global.scene_2_mask = 0;
    prog.organ.layer_a_enabled = true;
    prog.organ.drawbars_a = [8; 9];
    prog.piano.layer_a.enabled = true;
    prog.synth.layer_a.enabled = true;
    prog
}

#[test]
fn piano_program_plays_no_organ_in_firmware_and_classic_presets() {
    let piano = synthetic_piano();
    let prog = piano_program();
    for opts in [
        FirmwareOptions::default(),
        FirmwareOptions::firmware_all(),
        FirmwareOptions::legacy_all(),
    ] {
        let (total, organ, held) = organ_energy(&prog, &piano, opts);
        assert!(total > 1e-4, "the piano must sound ({})", opts.summary());
        assert_eq!(organ, 0.0, "organ energy with {}", opts.summary());
        assert_eq!(
            held[0] + held[1],
            0,
            "organ held keys with {}",
            opts.summary()
        );
        assert!(
            held[2] > 0,
            "piano A holds the keys with {}",
            opts.summary()
        );
    }
}

#[test]
fn panel_shows_the_organ_section_off_for_a_piano_program() {
    let mut panel = Stage4PanelState::default();
    assert!(panel.organ_a_active, "the start-up panel has Organ A on");
    panel.apply_program(&piano_program());
    assert!(!panel.organ_a_active && !panel.organ_b_active);
    assert!(panel.piano_a_active && !panel.synth_a_active);
}

#[test]
fn panel_follows_the_engine_for_an_enabled_section_without_layers() {
    // OQ25: organ section enabled beside piano, organ mask 0: the firmware plays organ A.
    let mut prog = piano_program();
    prog.global.section_enable_i = 0b110;
    prog.global.scene_1_mask = 1 << 2;
    let mut panel = Stage4PanelState::default();
    panel.apply_program(&prog);
    let mut eng = Stage4Engine::new(48_000.0);
    eng.apply_program(&prog);
    assert_eq!(eng.program_routing().exact_masks[0] & 1, 1);
    assert!(panel.organ_a_active, "same layer A the engine plays");
}

/// Factory pianos (local `.reference` archive or `NS4_FACTORY_BACKUP`; skipped when it is absent, as in CI).
#[test]
fn factory_piano_programs_have_zero_organ_energy() {
    let path = std::env::var_os("NS4_FACTORY_BACKUP").map_or_else(
        || {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.reference/Nord Stage 4 Factory Restore RevK v1.66.ns4b")
        },
        std::path::PathBuf::from,
    );
    let Ok(arch) = Ns4BackupArchive::open(&path) else {
        eprintln!("skipped: {} not present", path.display());
        return;
    };
    let mut r = BufReader::new(std::fs::File::open(&path).unwrap());
    for name in [
        "Felt Upright",
        "Astoria Grand",
        "Pno Brass Punch",
        "LoFi Piano",
    ] {
        let entry = arch
            .entries()
            .iter()
            .find(|e| {
                e.name.starts_with("Program/") && e.name.ends_with(".ns4p") && e.name.contains(name)
            })
            .unwrap()
            .name
            .clone();
        let bytes = arch.read_entry(&mut r, &entry).unwrap();
        let prog = ProgramDecoded::from_cbin_file(&CbinFile::parse(&bytes).unwrap()).unwrap();
        let refs = arch.resolve_program_sounds(&prog);
        let npno = arch
            .read_entry(&mut r, refs.piano_a.as_ref().unwrap())
            .unwrap();
        let piano = Arc::new(NpnoSound::from_npno_bytes(&npno).unwrap());
        let (total, organ, held) = organ_energy(&prog, &piano, FirmwareOptions::default());
        println!("{name}: total rms {total:.4} organ rms {organ:.6} held {held:?}");
        assert!(total > 1e-3, "{name} must sound");
        assert_eq!(organ, 0.0, "{name}");
        let mut panel = Stage4PanelState::default();
        panel.apply_program(&prog);
        assert!(
            !panel.organ_a_active && !panel.organ_b_active,
            "{name} panel"
        );
        // The library drawer before this fix: start-up engine, only the piano loaded and enabled.
        let old = |zero: bool| {
            let mut eng = standalone_engine(48_000.0);
            let mut vol = 0.8;
            let cmd = AudioCommand::LoadPianoSound {
                slot_b: false,
                sound: Some(Arc::clone(&piano)),
            };
            apply_audio_command(cmd, &mut eng, &mut vol);
            apply_audio_command(AudioCommand::SetLayerActive(2, true), &mut eng, &mut vol);
            if zero {
                eng.layer_levels[0].base = 0.0;
            }
            for note in [48u8, 60, 64, 67] {
                let ev = MidiEvent::NoteOn {
                    channel: 0,
                    note,
                    velocity: 100,
                };
                apply_audio_command(AudioCommand::Keyboard(ev), &mut eng, &mut vol);
            }
            (0..FRAMES)
                .map(|_| eng.render_frame()[0])
                .collect::<Vec<f32>>()
        };
        let (with, without) = (old(false), old(true));
        let diff: Vec<f32> = with.iter().zip(&without).map(|(a, b)| a - b).collect();
        println!(
            "{name}: drawer before the fix: total rms {:.4} organ rms {:.4}",
            rms(&with),
            rms(&diff)
        );
        assert!(rms(&diff) > 0.01, "{name}: the old drawer path keeps organ");
        let init = nord_gui::library::init_program(nord_gui::library::InitTarget::Piano(false));
        let (total, organ, held) = organ_energy(&init, &piano, FirmwareOptions::default());
        println!(
            "{name}: drawer after the fix: total rms {total:.4} organ rms {organ:.6} held {held:?}"
        );
        assert_eq!(organ, 0.0, "{name}: init + piano");
    }
}

/// Previous state that a loaded program must not leave behind: an organ program with effects on.
fn organ_program() -> ProgramDecoded {
    let mut prog = ProgramDecoded {
        name: "Organ X".to_string(),
        ..ProgramDecoded::default()
    };
    prog.global.section_enable_i = 0b111;
    prog.global.section_enable_ii = 0b111;
    prog.global.scene_1_mask = 0b011_0111;
    prog.global.scene_2_mask = 0b011_0111;
    prog.global.split_enabled = true;
    prog.global.split_zone_enabled = [false, true, false];
    prog.organ.layer_a_enabled = true;
    prog.organ.layer_b_enabled = true;
    prog.organ.drawbars_a = [8; 9];
    prog.organ.fx.delay_enabled = true;
    prog.organ.fx.reverb_enabled = true;
    prog.piano.layer_a.enabled = true;
    prog.synth.layer_a.enabled = true;
    prog.synth.layer_b.enabled = true;
    prog
}

/// Loads `programs` in order, then the piano, plays `NOTES`, returns the left channel.
fn render_sequence(programs: &[ProgramDecoded], piano: &Arc<NpnoSound>) -> Vec<f32> {
    let mut eng = standalone_engine(48_000.0);
    let mut vol = 0.8;
    for p in programs {
        apply_audio_command(
            AudioCommand::LoadProgram(Box::new(p.clone())),
            &mut eng,
            &mut vol,
        );
    }
    let sound = AudioCommand::LoadPianoSound {
        slot_b: false,
        sound: Some(Arc::clone(piano)),
    };
    apply_audio_command(sound, &mut eng, &mut vol);
    for note in [48u8, 60, 64, 67] {
        let ev = MidiEvent::NoteOn {
            channel: 0,
            note,
            velocity: 100,
        };
        apply_audio_command(AudioCommand::Keyboard(ev), &mut eng, &mut vol);
    }
    (0..FRAMES).map(|_| eng.render_frame()[0]).collect()
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn a_program_load_replaces_the_previous_state() {
    let piano = synthetic_piano();
    let y = piano_program();
    let fresh = render_sequence(std::slice::from_ref(&y), &piano);
    let after = render_sequence(&[organ_program(), y], &piano);
    let d = max_abs_diff(&fresh, &after);
    assert!(d < 1e-6, "program Y after X differs from a fresh Y by {d}");
}

#[test]
fn a_library_piano_after_an_organ_program_equals_a_fresh_init_plus_the_piano() {
    use nord_gui::library::{init_program, InitTarget};
    let piano = synthetic_piano();
    let init = init_program(InitTarget::Piano(false));
    let fresh = render_sequence(std::slice::from_ref(&init), &piano);
    let after = render_sequence(&[organ_program(), init.clone()], &piano);
    let d = max_abs_diff(&fresh, &after);
    assert!(d < 1e-6, "differs from a fresh init by {d}");
    let (total, organ, held) = organ_energy(&init, &piano, FirmwareOptions::default());
    assert!(total > 1e-4 && organ == 0.0 && held[0] + held[1] == 0);
    // without the init (the old drawer behaviour) the organ program stays on under the piano
    let merged = render_sequence(&[organ_program()], &piano);
    assert!(
        max_abs_diff(&fresh, &merged) > 1e-3,
        "control: merging keeps the organ program"
    );
}
