//! Native side of the program ranking (`docs/research/program-ranking.md`): renders every factory
//! program of a backup for ONE held note (default key 60, velocity 100, 2.08 s held, 2.73 s
//! analysed) twice, with the engine defaults and with the firmware-derived opt-ins
//! (`--firmware-all --firmware-routing`, the oracle-verified model), and prints one JSON line per
//! program: engines in scene I and II, layer counts, and level / centroid / band numbers per mode.
//! `--ablate` adds one more render per option group (the firmware options with that group put back
//! to the engine default), which says which group carries the distance. Numbers only; no audio,
//! firmware or sample content.
//!
//! Usage: `cargo run --release -p nord-tools --bin program_rank -- <backup.ns4b> [--note 60]
//! [--velocity 100] [--limit N] [--only S] [--every K] [--ablate]`. Release build only (stack depth in the FX
//! chains). `tools/program_rank.py` joins the lines with the chain census.

use nord_core::cbin::{CbinFile, Ns4BackupArchive, ProgramDecoded, ProgramSoundRefs};
use nord_core::render::render_timeline;
use nord_core::render_opts::FirmwareOptions;
use nord_tools::rank_metrics::{mono, note_metrics, NoteMetrics, ANALYSIS_FRAMES};
use nord_tools::sweep_load::{
    build_engine, events, flag, load_sounds, panic_message, PianoCache, SynthCache, FS,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Frames the note is held (2.08 s), the rest of the analysis window is the release.
const HOLD: usize = 100_000;

/// Sections enabled in a scene from `section_enable_*` (bit 2 organ, bit 1 piano, bit 0 synth).
fn sections(bits: u8) -> Value {
    json!({ "organ": bits & 4 != 0, "piano": bits & 2 != 0, "synth": bits & 1 != 0 })
}

/// Layer census of a program: enabled layers per engine and how many synth layers use a SAMPLES
/// multisample (a reference the backup resolved).
fn layers(prog: &ProgramDecoded, refs: &ProgramSoundRefs) -> Value {
    let piano = [prog.piano.layer_a.enabled, prog.piano.layer_b.enabled];
    let synth = [
        (prog.synth.layer_a.enabled, refs.synth_a.is_some()),
        (prog.synth.layer_b.enabled, refs.synth_b.is_some()),
        (prog.synth.layer_c.enabled, refs.synth_c.is_some()),
    ];
    json!({
        "piano": piano.iter().filter(|e| **e).count(),
        "synth": synth.iter().filter(|(e, _)| *e).count(),
        "synth_sample": synth.iter().filter(|(e, s)| *e && *s).count(),
        "organ_model": format!("{:?}", prog.organ.model),
    })
}

/// Option groups by which `--firmware-all --firmware-routing` differs from the engine default (the
/// fields below are all of them; the firmware amp is on in both: the test `groups_cover_every_difference` checks it).
/// Puts the fields of one group of the first options back to the values of the second.
type Revert = fn(&mut FirmwareOptions, &FirmwareOptions);

const GROUPS: [(&str, Revert); 8] = [
    ("organ_tone", |o, d| o.tone_source = d.tone_source),
    ("synth_filter", |o, d| o.filter_engine = d.filter_engine),
    ("fx_delay", |o, d| {
        o.fx.delay_laws = d.fx.delay_laws;
        o.fx.delay_time = d.fx.delay_time;
    }),
    ("fx_compressor", |o, d| o.fx.compressor = d.fx.compressor),
    ("fx_reverb", |o, d| {
        o.fx.reverb_mix = d.fx.reverb_mix;
        o.fx.reverb = d.fx.reverb;
    }),
    ("fx_eq", |o, d| o.fx.eq = d.fx.eq),
    ("fx_mod1", |o, d| o.fx.mod1 = d.fx.mod1),
    ("fx_mod2", |o, d| o.fx.mod2 = d.fx.mod2),
];

/// `firmware` with one group put back to the default.
fn reverted(firmware: &FirmwareOptions, group: usize) -> FirmwareOptions {
    let mut o = *firmware;
    GROUPS[group].1(&mut o, &FirmwareOptions::default());
    o
}

/// Renders `prog` once with `opts` and measures the mono mean.
fn measure(
    prog: &ProgramDecoded,
    refs: &ProgramSoundRefs,
    pianos: &PianoCache,
    synths: &SynthCache,
    opts: &FirmwareOptions,
    note: u8,
    velocity: u8,
) -> Result<NoteMetrics, String> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut engine = build_engine(prog, refs, pianos, synths, opts);
        let ev = events(&[note], velocity, HOLD);
        let (l, r) = render_timeline(&mut engine, &ev, ANALYSIS_FRAMES);
        note_metrics(&mono(&l, &r), f64::from(FS))
    }))
    .map_err(panic_message)
}

fn metrics_json(m: &Result<NoteMetrics, String>) -> Value {
    match m {
        Ok(m) => json!({
            "rms_db": m.rms_db, "peak_db": m.peak_db,
            "centroid_hz": m.centroid_hz, "bands_db": m.bands_db,
        }),
        Err(p) => json!({ "panic": p }),
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: program_rank <backup.ns4b> [--note N] [--velocity V] [--limit N] [--only S] [--every K]")?;
    let num = |name: &str, default: usize| {
        flag(&args, name)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let note = num("--note", 60).min(127) as u8;
    let velocity = num("--velocity", 100).min(127) as u8;
    let (limit, every) = (num("--limit", usize::MAX), num("--every", 1).max(1));
    let only = flag(&args, "--only");
    let ablate = args.iter().any(|a| a == "--ablate");
    let archive = Ns4BackupArchive::open(path).map_err(|e| e.to_string())?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let (mut pianos, mut synths): (PianoCache, SynthCache) = (HashMap::new(), HashMap::new());
    let names: Vec<String> = archive
        .entries_with_ext(".ns4p")
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| only.is_none_or(|s| n.contains(s.as_str())))
        .step_by(every)
        .take(limit)
        .collect();
    std::panic::set_hook(Box::new(|_| {}));
    let firmware = FirmwareOptions {
        firmware_routing: true,
        ..FirmwareOptions::firmware_all()
    };
    for name in names {
        let bytes = archive
            .read_entry(&mut file, &name)
            .map_err(|e| e.to_string())?;
        let prog = CbinFile::parse(&bytes)
            .and_then(|c| c.decode_program())
            .map_err(|e| format!("{name}: {e}"))?;
        let refs = archive.resolve_program_sounds(&prog);
        load_sounds(&archive, &mut file, &refs, &mut pianos, &mut synths);
        let run = |o: &FirmwareOptions| measure(&prog, &refs, &pianos, &synths, o, note, velocity);
        let default = run(&FirmwareOptions::default());
        let fw = run(&firmware);
        let mut line = json!({
            "entry": name,
            "name": prog.name,
            "scene_1": sections(prog.global.section_enable_i),
            "scene_2": sections(prog.global.section_enable_ii),
            "layers": layers(&prog, &refs),
            "note": note,
            "velocity": velocity,
            "default": metrics_json(&default),
            "firmware": metrics_json(&fw),
        });
        if ablate {
            let ab: serde_json::Map<String, Value> = (0..GROUPS.len())
                .map(|g| {
                    (
                        GROUPS[g].0.to_string(),
                        metrics_json(&run(&reverted(&firmware, g))),
                    )
                })
                .collect();
            line["ablate"] = Value::Object(ab);
        }
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_decode_the_enable_bits() {
        let s = sections(0b101);
        assert_eq!(s["organ"], true);
        assert_eq!(s["piano"], false);
        assert_eq!(s["synth"], true);
    }

    #[test]
    fn layers_census_counts_enabled_layers_of_the_default_program() {
        let prog = ProgramDecoded::default();
        let l = layers(&prog, &ProgramSoundRefs::default());
        assert_eq!(l["synth_sample"], 0);
        assert!(l["organ_model"].as_str().is_some());
    }

    #[test]
    fn measure_default_program_is_finite_and_serialises() {
        let prog = ProgramDecoded::default();
        let refs = ProgramSoundRefs::default();
        let m = measure(
            &prog,
            &refs,
            &HashMap::new(),
            &HashMap::new(),
            &FirmwareOptions::default(),
            60,
            100,
        );
        let v = metrics_json(&m);
        assert!(v["rms_db"].is_number(), "{v}");
        assert_eq!(v["bands_db"].as_array().map(Vec::len), Some(8));
    }

    #[test]
    fn groups_cover_every_difference() {
        let firmware = FirmwareOptions {
            firmware_routing: true,
            ..FirmwareOptions::firmware_all()
        };
        let mut o = firmware;
        for (_, revert) in GROUPS {
            revert(&mut o, &FirmwareOptions::default());
        }
        assert_eq!(o, FirmwareOptions::default(), "a group is missing");
        assert_ne!(firmware, FirmwareOptions::default());
        for g in 0..GROUPS.len() {
            assert_ne!(
                reverted(&firmware, g),
                firmware,
                "group {g} changes nothing"
            );
        }
    }

    #[test]
    fn panic_is_reported_not_propagated() {
        let v = metrics_json(&Err("boom".into()));
        assert_eq!(v["panic"], "boom");
    }
}
