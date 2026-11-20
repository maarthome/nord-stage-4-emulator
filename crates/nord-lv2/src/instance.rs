//! The plugin instance in safe Rust: engine, MIDI dispatch, block rendering, control and program
//! bookkeeping.
//!
//! Real-time rules (`.agents/rules/dsp-and-realtime.md`): `run`, `apply_controls`, `state`,
//! `take_program_request` and `swap_engine` never allocate, lock or do I/O. Engines are built only
//! by [`build_engine`] in `new` / `reset` (LV2 instantiate / activate) and on the worker thread.

use crate::atom;
use crate::ports::{program_number, Controls, State, LAYERS};
use nord_core::cbin::ProgramDecoded;
use nord_core::engine::Stage4Engine;
use nord_core::midi::MidiEvent;

/// Frames rendered per `process_block_multi` call; any host block size is split into these.
pub const CHUNK: usize = 256;

type Scratch = [[f32; CHUNK]; 4];

/// A ready engine: optional program applied, layer levels scaled by the master `gain`, firmware
/// WAVE tables switched on when `wave` (see `wave.rs`). Not real-time safe.
pub fn build_engine(
    sample_rate: f32,
    prog: Option<&ProgramDecoded>,
    gain: f32,
    wave: bool,
) -> Box<Stage4Engine> {
    let mut e = Box::new(Stage4Engine::new(sample_rate));
    for s in [&mut e.synth_a, &mut e.synth_b, &mut e.synth_c] {
        s.wave_bank = wave;
    }
    if let Some(p) = prog {
        e.apply_program(p);
        for l in e.layer_levels.iter_mut() {
            l.base *= gain;
        }
    }
    e
}

pub struct Instance {
    pub engine: Box<Stage4Engine>,
    sample_rate: f32,
    wave: bool,
    midi_event_urid: u32,
    scratch: Box<Scratch>,
    /// Control port values seen last block. A control is written to the engine only when its own
    /// port moves, so a loaded program or a MIDI CC keeps its settings until the user turns that
    /// control.
    last: Option<Controls>,
    gain: f32,
    /// Program number now playing (0 = engine defaults).
    pub loaded_program: u32,
    /// Last value of the `program` port, to request a load only when it moves.
    port_program: Option<u32>,
}

impl Instance {
    pub fn new(sample_rate: f32, midi_event_urid: u32, wave: bool) -> Self {
        Self {
            engine: build_engine(sample_rate, None, 1.0, wave),
            sample_rate,
            wave,
            midi_event_urid,
            scratch: Box::new([[0.0; CHUNK]; 4]),
            last: None,
            gain: 1.0,
            loaded_program: 0,
            port_program: None,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn wave(&self) -> bool {
        self.wave
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Fresh engine (LV2 `activate`). Not real-time safe. The program port is re-read next block.
    pub fn reset(&mut self) {
        self.engine = build_engine(self.sample_rate, None, 1.0, self.wave);
        self.last = None;
        self.gain = 1.0;
        self.loaded_program = 0;
        self.port_program = None;
    }

    /// Writes the controls whose port changed since the last block (all of them the first time).
    pub fn apply_controls(&mut self, c: &Controls) {
        let gain = c.master_gain();
        let prev = self.last.replace(*c);
        let e = &mut self.engine;
        for i in 0..LAYERS {
            let on_moved = prev.is_none_or(|p| p.layer_on[i] != c.layer_on[i]);
            if on_moved {
                let bit = 1u8 << i;
                let m = &mut e.scene.scene_1_mask;
                *m = if c.layer_on[i] { *m | bit } else { *m & !bit };
            }
            let level_moved = prev.is_none_or(|p| p.layer_level[i] != c.layer_level[i]);
            let lvl = &mut e.layer_levels[i].base;
            if level_moved || self.gain <= 0.0 {
                *lvl = c.layer_level[i] * gain;
            } else if gain != self.gain {
                // master moved: rescale the level in place, keeping a program's own level
                *lvl *= gain / self.gain;
            }
        }
        self.gain = gain;
    }

    /// The engine's real layer state for the output ports.
    pub fn state(&self) -> State {
        let mask = self.engine.scene.scene_1_mask;
        let mut s = State {
            program: self.loaded_program,
            layer_on: [false; LAYERS],
            layer_level: [0.0; LAYERS],
        };
        for i in 0..LAYERS {
            s.layer_on[i] = mask & (1 << i) != 0;
            s.layer_level[i] = if self.gain > 0.0 {
                (self.engine.layer_levels[i].base / self.gain).clamp(0.0, 1.0)
            } else {
                self.last.map_or(0.0, |c| c.layer_level[i])
            };
        }
        s
    }

    /// A program to load now, if any: a MIDI Program Change (bank MSB 0: program
    /// `128 * LSB + PC + 1`), or the `program` port moved to a value not already playing.
    pub fn take_program_request(&mut self, port: Option<f32>) -> Option<u32> {
        if let Some(pc) = self.engine.take_program_change() {
            if pc.bank_msb == 0 {
                return Some(128 * u32::from(pc.bank_lsb) + u32::from(pc.program) + 1);
            }
        }
        let p = program_number(port?);
        if self.port_program == Some(p) {
            return None;
        }
        self.port_program = Some(p);
        (p != self.loaded_program).then_some(p)
    }

    /// Puts a worker-built engine in place and returns the old one (to be freed off the audio
    /// thread). Program 0 (engine defaults) re-applies every control port next block.
    pub fn swap_engine(&mut self, new: Box<Stage4Engine>, program: u32) -> Box<Stage4Engine> {
        self.loaded_program = program;
        if program == 0 {
            self.last = None;
        }
        std::mem::replace(&mut self.engine, new)
    }

    /// Renders `n` frames. MIDI events are applied at their frame offset (sample accurate).
    /// `midi` is the valid part of the `atom:Sequence` buffer; unconnected outputs are `None`.
    pub fn run(&mut self, midi: Option<&[u8]>, outs: &mut [Option<&mut [f32]>; 4], n: usize) {
        let mut pos = 0;
        let engine = &mut *self.engine;
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
        let mut inst = Instance::new(48_000.0, MIDI, false);
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
        let mut inst = Instance::new(44_100.0, MIDI, false);
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
    fn controls_apply_only_when_their_port_moves() {
        let mut inst = Instance::new(48_000.0, MIDI, false);
        let mut c = Controls {
            master_db: -6.0,
            ..Controls::default()
        };
        inst.apply_controls(&c);
        let g = c.master_gain();
        assert!((inst.engine.layer_levels[0].base - 0.8 * g).abs() < 1e-6);
        // A program / CC change survives blocks with unchanged ports.
        inst.engine.layer_levels[0].base = 0.1;
        inst.engine.scene.scene_1_mask = 0b10;
        inst.apply_controls(&c);
        assert_eq!(inst.engine.layer_levels[0].base, 0.1);
        assert_eq!(inst.engine.scene.scene_1_mask, 0b10);
        // Moving one toggle changes only that bit; moving master rescales in place.
        c.layer_on[6] = true;
        c.master_db = 0.0;
        inst.apply_controls(&c);
        assert_eq!(inst.engine.scene.scene_1_mask, 0b100_0010);
        assert!((inst.engine.layer_levels[0].base - 0.1 / g).abs() < 1e-5);
        let s = inst.state();
        assert!(s.layer_on[1] && s.layer_on[6] && !s.layer_on[0]);
        assert!((s.layer_level[0] - 0.1 / g).abs() < 1e-5);
        inst.reset();
        assert_eq!(inst.engine.layer_levels[0].base, 0.8);
    }

    #[test]
    fn program_requests_from_port_and_program_change() {
        let mut inst = Instance::new(48_000.0, MIDI, false);
        assert_eq!(inst.take_program_request(None), None);
        assert_eq!(
            inst.take_program_request(Some(0.0)),
            None,
            "0 already playing"
        );
        assert_eq!(inst.take_program_request(Some(3.0)), Some(3));
        assert_eq!(
            inst.take_program_request(Some(3.0)),
            None,
            "only when the port moves"
        );
        // Program Change 4 on the global channel (bank 0) asks for program 5.
        let pc = build_sequence(SEQ, &[(0, MIDI, &[0xC0, 4])]);
        run_once(&mut inst, &pc, 64);
        assert_eq!(inst.take_program_request(Some(3.0)), Some(5));
        let old = inst.swap_engine(build_engine(48_000.0, None, 1.0, false), 5);
        drop(old);
        assert_eq!(inst.loaded_program, 5);
        assert_eq!(inst.state().program, 5);
        assert_eq!(inst.take_program_request(Some(0.0)), Some(0));
    }

    #[test]
    fn long_blocks_and_unconnected_outputs_are_handled() {
        let mut inst = Instance::new(48_000.0, MIDI, false);
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
