//! The plugin instance in safe Rust: engine, MIDI dispatch and block rendering.
//!
//! Real-time rules (`.agents/rules/dsp-and-realtime.md`): `run` and everything it calls never
//! allocates, locks or does I/O. The engine is only rebuilt in `new` / `reset` (LV2
//! instantiate / activate, which are not real-time).

use crate::atom;
use crate::ports::Controls;
use nord_core::engine::Stage4Engine;
use nord_core::midi::MidiEvent;

/// Frames rendered per `process_block_multi` call; any host block size is split into these.
pub const CHUNK: usize = 256;

type Scratch = [[f32; CHUNK]; 4];

pub struct Instance {
    pub engine: Stage4Engine,
    sample_rate: f32,
    midi_event_urid: u32,
    scratch: Box<Scratch>,
    /// Controls last written to the engine. Re-applied only when a port value changes, so MIDI
    /// CCs that move the same engine state (layer level, scene) keep working in between.
    applied: Option<Controls>,
}

impl Instance {
    pub fn new(sample_rate: f32, midi_event_urid: u32) -> Self {
        Self {
            engine: Stage4Engine::new(sample_rate),
            sample_rate,
            midi_event_urid,
            scratch: Box::new([[0.0; CHUNK]; 4]),
            applied: None,
        }
    }

    /// Fresh engine (LV2 `activate`). Not real-time safe.
    pub fn reset(&mut self) {
        self.engine = Stage4Engine::new(self.sample_rate);
        self.applied = None;
    }

    /// Writes changed control values into the engine (Scene I mask, layer levels × master).
    pub fn apply_controls(&mut self, c: &Controls) {
        if self.applied.as_ref() == Some(c) {
            return;
        }
        self.engine.scene.scene_1_mask = c.layer_mask();
        let gain = c.master_gain();
        for (lvl, &v) in self
            .engine
            .layer_levels
            .iter_mut()
            .zip(c.layer_level.iter())
        {
            lvl.base = v * gain;
        }
        self.applied = Some(*c);
    }

    /// Renders `n` frames. MIDI events are applied at their frame offset (sample accurate).
    /// `midi` is the valid part of the `atom:Sequence` buffer; unconnected outputs are `None`.
    pub fn run(&mut self, midi: Option<&[u8]>, outs: &mut [Option<&mut [f32]>; 4], n: usize) {
        let mut pos = 0;
        let engine = &mut self.engine;
        let scratch = &mut *self.scratch;
        if let Some(seq) = midi {
            let urid = self.midi_event_urid;
            atom::for_each_event(seq, |frame, ty, data| {
                let at = (frame.max(0) as usize).min(n).max(pos);
                render(engine, scratch, outs, pos, at);
                pos = at;
                if ty == urid {
                    if let Some(ev) = MidiEvent::from_bytes(data) {
                        engine.handle_midi(ev);
                    }
                }
            });
        }
        render(engine, scratch, outs, pos, n);
    }
}

/// Renders frames `from..to` into every connected output via the scratch buffers.
fn render(
    engine: &mut Stage4Engine,
    scratch: &mut Scratch,
    outs: &mut [Option<&mut [f32]>; 4],
    from: usize,
    to: usize,
) {
    let mut at = from;
    while at < to {
        let len = (to - at).min(CHUNK);
        {
            let [a, b, c, d] = &mut *scratch;
            engine.process_block_multi([
                &mut a[..len],
                &mut b[..len],
                &mut c[..len],
                &mut d[..len],
            ]);
        }
        for (src, out) in scratch.iter().zip(outs.iter_mut()) {
            if let Some(o) = out {
                if let Some(dst) = o.get_mut(at..at + len) {
                    dst.copy_from_slice(&src[..len]);
                }
            }
        }
        at += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::tests::build_sequence;

    const MIDI: u32 = 7;
    const SEQ: u32 = 3;

    fn run_once(inst: &mut Instance, seq: &[u8], n: usize) -> [Vec<f32>; 2] {
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        let mut outs = [Some(&mut l[..]), Some(&mut r[..]), None, None];
        inst.run(Some(seq), &mut outs, n);
        [l, r]
    }

    fn peak(b: &[f32]) -> f32 {
        b.iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    #[test]
    fn note_on_is_sample_accurate_and_audible() {
        let mut inst = Instance::new(48_000.0, MIDI);
        inst.apply_controls(&Controls::default());
        let on = build_sequence(SEQ, &[(300, MIDI, &[0x90, 60, 110])]);
        let [l, r] = run_once(&mut inst, &on, 1000);
        assert_eq!(peak(&l[..300]), 0.0, "silent before the event frame");
        assert_eq!(peak(&r[..300]), 0.0);
        let mut loud = peak(&l[300..]).max(peak(&r[300..]));
        let empty = build_sequence(SEQ, &[]);
        for _ in 0..20 {
            let [l, r] = run_once(&mut inst, &empty, 512);
            loud = loud.max(peak(&l)).max(peak(&r));
            assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
        }
        assert!(loud > 0.0, "a held note produced no audio");
    }

    #[test]
    fn non_midi_events_and_all_layers_off_stay_silent() {
        let mut inst = Instance::new(44_100.0, MIDI);
        let off = Controls {
            layer_on: [false; 7],
            ..Controls::default()
        };
        inst.apply_controls(&off);
        assert_eq!(inst.engine.scene.scene_1_mask, 0);
        let seq = build_sequence(
            SEQ,
            &[(0, MIDI, &[0x90, 60, 110]), (0, MIDI + 1, &[0x90, 64, 110])],
        );
        let mut loud = 0.0f32;
        for _ in 0..8 {
            let [l, r] = run_once(&mut inst, &seq, 333);
            loud = loud.max(peak(&l)).max(peak(&r));
        }
        assert_eq!(loud, 0.0);
    }

    #[test]
    fn controls_apply_only_on_change_and_scale_levels() {
        let mut inst = Instance::new(48_000.0, MIDI);
        let c = Controls {
            master_db: -6.0,
            ..Controls::default()
        };
        inst.apply_controls(&c);
        let expect = 0.8 * c.master_gain();
        assert!((inst.engine.layer_levels[0].base - expect).abs() < 1e-6);
        // A MIDI-driven change survives a run with unchanged ports.
        inst.engine.layer_levels[0].base = 0.1;
        inst.apply_controls(&c);
        assert_eq!(inst.engine.layer_levels[0].base, 0.1);
        inst.reset();
        assert_eq!(inst.engine.layer_levels[0].base, 0.8);
    }

    #[test]
    fn long_blocks_and_unconnected_outputs_are_handled() {
        let mut inst = Instance::new(48_000.0, MIDI);
        let n = CHUNK * 3 + 17;
        let mut aux_l = vec![1.0; n];
        let mut outs = [None, None, Some(&mut aux_l[..]), None];
        inst.run(None, &mut outs, n);
        assert!(
            aux_l.iter().all(|&v| v == 0.0),
            "aux is silent when nothing routes there"
        );
    }
}
