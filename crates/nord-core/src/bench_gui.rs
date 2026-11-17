//! `ns4 bench --gui`: the workload and render path of the standalone GUI callback.
//!
//! The GUI renders `render_frame()` per frame (no `process_block`, one stereo pair) after
//! `FirmwareOptions::apply_exact`, with the keys pressed through MIDI. This module times that path
//! block by block from the very first block (no warm-up unless asked), reports the tail
//! (blocks over the deadline, worst block and where it was) and can replay key churn to price a
//! voice start. Timing is wall clock; the hash is deterministic and covers the left/right pair.

use crate::bench::{fnv1a_f32, BLOCK_FRAMES, FNV_BASIS, SAMPLE_RATE};
use crate::engine::Stage4Engine;
use crate::midi::MidiEvent;
use crate::render_opts::FirmwareOptions;
use std::time::Instant;

/// What to render.
#[derive(Debug, Clone, Copy)]
pub struct GuiBenchConfig {
    /// Layers sounding: `3` = Scene I (Organ A, Piano A, Synth A), `7` = Scene II.
    pub layers: u8,
    /// Keys held (consecutive MIDI notes from 36, velocity 100).
    pub keys: u8,
    /// Frames per callback.
    pub frames: usize,
    /// Blocks excluded from timing.
    pub warmup_blocks: usize,
    /// Timed blocks.
    pub blocks: usize,
    /// Every this many blocks release the oldest key and press a new one (`0`: static chord).
    pub churn_every: usize,
    /// Firmware options, applied like the GUI's `SetFirmwareOptions`.
    pub options: FirmwareOptions,
}

impl Default for GuiBenchConfig {
    fn default() -> Self {
        Self {
            layers: 7,
            keys: 88,
            frames: BLOCK_FRAMES,
            warmup_blocks: 0,
            blocks: 3000,
            churn_every: 0,
            options: FirmwareOptions::default(),
        }
    }
}

/// Timing of one run, milliseconds per block.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiBenchReport {
    pub blocks: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    /// Block (0-based, among the timed ones) of the worst time.
    pub worst_index: usize,
    /// Blocks slower than `frames / sample_rate`.
    pub late: usize,
    pub deadline_ms: f64,
    /// First timed block (cold caches, lazy tables when `warmup_blocks == 0`).
    pub first_ms: f64,
    /// Mean of the blocks that carried a key press / of the others (churn runs).
    pub press_mean_ms: f64,
    pub quiet_mean_ms: f64,
    /// Worst press block.
    pub press_max_ms: f64,
    pub hash: u64,
    pub peak: f32,
}

/// The engine of `cfg` before any key is pressed.
pub fn engine(cfg: &GuiBenchConfig) -> Stage4Engine {
    let mut e = Stage4Engine::new(SAMPLE_RATE);
    e.scene.scene_2_active = cfg.layers > 3;
    cfg.options.apply_exact(&mut e);
    e
}

fn note(e: &mut Stage4Engine, note: u8, on: bool) {
    e.handle_midi(if on {
        MidiEvent::NoteOn {
            channel: 0,
            note,
            velocity: 100,
        }
    } else {
        MidiEvent::NoteOff {
            channel: 0,
            note,
            velocity: 0,
        }
    });
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    sorted
        .get(((sorted.len() as f64 - 1.0) * q).round() as usize)
        .copied()
        .unwrap_or(0.0)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

/// Renders `cfg` on the engine `setup` leaves and times every block.
pub fn run_with(cfg: GuiBenchConfig, setup: impl FnOnce(&mut Stage4Engine)) -> GuiBenchReport {
    let mut e = engine(&cfg);
    setup(&mut e);
    let first_key = 36u8;
    for k in 0..cfg.keys {
        note(&mut e, first_key.saturating_add(k).min(127), true);
    }
    let mut next = u16::from(first_key) + u16::from(cfg.keys);
    let mut oldest = u16::from(first_key);
    let mut times = Vec::with_capacity(cfg.blocks);
    let mut pressed = Vec::with_capacity(cfg.blocks);
    let (mut hash, mut peak) = (FNV_BASIS, 0.0f32);
    let mut buf = vec![0.0f32; cfg.frames * 2];
    for i in 0..cfg.warmup_blocks + cfg.blocks {
        let timed = i >= cfg.warmup_blocks;
        let churn = cfg.churn_every > 0 && i % cfg.churn_every == 0 && i > 0;
        let t0 = Instant::now();
        if churn {
            note(&mut e, (oldest % 100 + 21) as u8, false);
            note(&mut e, (next % 100 + 21) as u8, true);
            oldest += 1;
            next += 1;
        }
        for f in buf.chunks_exact_mut(2) {
            let [l, r] = e.render_frame();
            f[0] = l * 0.85;
            f[1] = r * 0.85;
        }
        if timed {
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            pressed.push(churn);
            hash = fnv1a_f32(hash, &buf);
            peak = buf.iter().fold(peak, |p, s| p.max(s.abs()));
        }
    }
    let deadline_ms = cfg.frames as f64 / f64::from(SAMPLE_RATE) * 1000.0;
    let (worst_index, max_ms) =
        times
            .iter()
            .enumerate()
            .fold((0, 0.0), |a, (i, &t)| if t > a.1 { (i, t) } else { a });
    let split = |on: bool| -> Vec<f64> {
        times
            .iter()
            .zip(&pressed)
            .filter(|(_, &p)| p == on)
            .map(|(&t, _)| t)
            .collect()
    };
    let press = split(true);
    let mut sorted = times.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    GuiBenchReport {
        blocks: times.len(),
        mean_ms: mean(&times),
        p50_ms: quantile(&sorted, 0.5),
        p99_ms: quantile(&sorted, 0.99),
        max_ms,
        worst_index,
        late: times.iter().filter(|&&t| t > deadline_ms).count(),
        deadline_ms,
        first_ms: times.first().copied().unwrap_or(0.0),
        press_mean_ms: mean(&press),
        quiet_mean_ms: mean(&split(false)),
        press_max_ms: press.iter().copied().fold(0.0, f64::max),
        hash,
        peak,
    }
}

/// [`run_with`] on the plain engine.
pub fn run(cfg: GuiBenchConfig) -> GuiBenchReport {
    run_with(cfg, |_| {})
}

/// One report line.
pub fn format_row(label: &str, r: &GuiBenchReport) -> String {
    format!(
        "{label:<34} mean {:6.3}  p50 {:6.3}  p99 {:6.3}  max {:7.3} (block {:>4})  late {:>4}/{}  first {:6.3}",
        r.mean_ms, r.p50_ms, r.p99_ms, r.max_ms, r.worst_index, r.late, r.blocks, r.first_ms
    )
}

/// Tail figures with parts of the engine switched off, to find what the spikes come from. The
/// rows are the full engine, FX racks off, no keys, and each layer alone (FX off).
pub fn breakdown(cfg: GuiBenchConfig) -> Vec<(String, GuiBenchReport)> {
    use crate::fx::{LayerFxChain, ReverbModel};
    let fx_off = |e: &mut Stage4Engine| {
        for chain in e.fx_chains.iter_mut() {
            *chain = LayerFxChain::new(SAMPLE_RATE);
            chain.reverb_model = ReverbModel::Off;
        }
        e.master_bus.compressor_amount = 0.0;
    };
    let mut rows = vec![("full".to_string(), run(cfg))];
    rows.push(("FX racks + master comp off".into(), run_with(cfg, fx_off)));
    rows.push((
        "no keys held".into(),
        run(GuiBenchConfig { keys: 0, ..cfg }),
    ));
    let names = [
        "Organ A", "Organ B", "Piano A", "Piano B", "Synth A", "Synth B", "Synth C",
    ];
    let count = usize::from(cfg.layers.min(7));
    let all: [usize; 7] = [0, 1, 2, 3, 4, 5, 6];
    // Scene I plays Organ A, Piano A, Synth A; Scene II all seven.
    let in_scene: Vec<usize> = if count > 3 {
        all.to_vec()
    } else {
        vec![0, 2, 4]
    };
    for i in in_scene {
        let only = move |e: &mut Stage4Engine| {
            fx_off(e);
            e.scene.scene_2_mask = 1 << i;
            e.scene.scene_1_mask = 1 << i;
        };
        rows.push((format!("{} only, FX off", names[i]), run_with(cfg, only)));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick() -> GuiBenchConfig {
        GuiBenchConfig {
            blocks: 40,
            ..GuiBenchConfig::default()
        }
    }

    #[test]
    fn run_is_deterministic_audible_and_counts_blocks() {
        let (a, b) = (run(quick()), run(quick()));
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.blocks, 40);
        assert!(a.peak > 0.0 && a.peak.is_finite());
        assert!(a.max_ms >= a.p99_ms && a.p99_ms >= a.p50_ms);
        assert!((a.deadline_ms - 2.6667).abs() < 1e-3);
        assert!(a.worst_index < a.blocks);
    }

    #[test]
    fn three_and_seven_layers_differ_and_churn_prices_key_presses() {
        let three = run(GuiBenchConfig {
            layers: 3,
            keys: 10,
            ..quick()
        });
        let seven = run(GuiBenchConfig {
            keys: 10,
            ..quick()
        });
        assert_ne!(three.hash, seven.hash, "Scene II adds layers");
        let churn = run(GuiBenchConfig {
            keys: 10,
            churn_every: 4,
            ..quick()
        });
        assert!(churn.press_mean_ms > 0.0 && churn.quiet_mean_ms > 0.0);
        assert!(churn.press_max_ms > 0.0);
    }

    #[test]
    fn warmup_blocks_are_not_reported() {
        let r = run(GuiBenchConfig {
            warmup_blocks: 5,
            ..quick()
        });
        assert_eq!(r.blocks, 40);
    }

    #[test]
    fn breakdown_names_the_rows_for_both_scenes() {
        let rows = breakdown(GuiBenchConfig {
            blocks: 8,
            keys: 4,
            ..GuiBenchConfig::default()
        });
        assert_eq!(rows.len(), 3 + 7);
        let rows3 = breakdown(GuiBenchConfig {
            layers: 3,
            blocks: 8,
            keys: 4,
            ..GuiBenchConfig::default()
        });
        assert_eq!(rows3.len(), 3 + 3);
        assert!(format_row(&rows3[0].0, &rows3[0].1).contains("full"));
    }
}
