//! Synth morph targets the engine had no slot for (opt-in `--firmware-morph`, D157): Osc Ctrl,
//! filter resonance, LFO rate, LFO amount and filter envelope amount.
//!
//! The ARM re-dispatches every morph-assigned edit id with `base + sum(delta_i * w_i)` clamped to
//! `0..127` (VERIFIED, `docs/research/morph-fw-vs-native.md`), so each slot here is a
//! [`MorphParameter`] in knob units (`raw / 127`) that is converted back to the engine field
//! with the inverse of the preset decode (`cbin/synth_preset.rs`).

use crate::cbin::{MorphDeltas, SynthLayerDecoded, LFO_RATE_HZ_TABLE};
use crate::morph::{MorphControllers, MorphParameter};
use crate::synth::SynthLayer;

/// Morph slots of one Synth layer beyond level and cutoff.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynthMorphSet {
    pub osc_ctrl: MorphParameter,
    pub resonance: MorphParameter,
    pub lfo_rate: MorphParameter,
    pub lfo_amount: MorphParameter,
    pub env_amount: MorphParameter,
    /// Base pulse width of the loaded wave (`pulse_base_width`), needed to redo Osc Ctrl.
    pub pulse_base: f32,
}

impl Default for SynthMorphSet {
    fn default() -> Self {
        Self {
            osc_ctrl: MorphParameter::new(0.0),
            resonance: MorphParameter::new(0.0),
            lfo_rate: MorphParameter::new(0.0),
            lfo_amount: MorphParameter::new(0.0),
            env_amount: MorphParameter::new(0.0),
            pulse_base: 0.5,
        }
    }
}

fn copy_deltas(p: &mut MorphParameter, m: MorphDeltas) {
    p.delta_wheel = m.wheel;
    p.delta_aftertouch = m.aftertouch;
    p.delta_pedal = m.ctrl_pedal;
}

fn active(p: &MorphParameter) -> bool {
    p.delta_wheel.abs() > 1e-5 || p.delta_pedal.abs() > 1e-5 || p.delta_aftertouch.abs() > 1e-5
}

/// Cutoff position the ARM sender `0xC01C56CC` turns a morphed Q16.16 knob into: one whole step
/// lower once the fraction reaches 0.5.
///
/// VERIFIED: static dispatch of 54.0 .. 55.0 on synth layer A sends `(x - 52) << 24` below 54.5 and
/// `(x - 53) << 24` from 54.5 (`54.4375 -> 0x02700000`, `54.5 -> 0x01800000`); the Horizon Whl
/// wheel run sends the same words (`0x01800000` at 54.5, `0xF3800000` at 40.5), reproduced by
/// `tools/morph_law_report.py` (`sender_knob`, test
/// `cutoff_rows_reproduce_the_measured_horizon_words`). The DSP-side reading of the fractional
/// word (geometric interpolation of the table) is `ASSUMED`.
pub fn sender_norm(norm: f32) -> f32 {
    let knob = norm.clamp(0.0, 1.0) * 127.0;
    let sent = if knob - knob.floor() >= 0.5 {
        (knob - 1.0).max(0.0)
    } else {
        knob
    };
    sent / 127.0
}

/// LFO rate in Hz at a fractional table position `norm * 127` (geometric between steps, like
/// `cutoff_norm_to_hz`).
pub fn lfo_norm_to_hz(norm: f32) -> f32 {
    let pos = norm.clamp(0.0, 1.0) * 127.0;
    let lo = (pos.floor() as usize).min(127);
    let hi = (lo + 1).min(127);
    let a = LFO_RATE_HZ_TABLE[lo];
    let b = LFO_RATE_HZ_TABLE[hi];
    a * (b / a).powf(pos - lo as f32)
}

impl SynthMorphSet {
    /// Bases and depths of a decoded layer.
    pub fn from_decoded(dec: &SynthLayerDecoded) -> Self {
        let mut s = Self {
            pulse_base: crate::synth_params::pulse_base_width(dec),
            ..Self::default()
        };
        s.osc_ctrl.base = (dec.detune / 10.0).clamp(0.0, 1.0);
        s.resonance.base = (dec.filter_resonance / 0.95).clamp(0.0, 1.0);
        s.lfo_rate.base = dec.raw.map_or(0.0, |r| f32::from(r.lfo_rate_idx) / 127.0);
        s.lfo_amount.base = dec.lfo_amount.clamp(0.0, 1.0);
        s.env_amount.base = dec.filter_env_amount.clamp(0.0, 1.0);
        copy_deltas(&mut s.osc_ctrl, dec.osc_ctrl_morph);
        copy_deltas(&mut s.resonance, dec.filter_resonance_morph);
        copy_deltas(&mut s.lfo_rate, dec.lfo_rate_morph);
        copy_deltas(&mut s.lfo_amount, dec.lfo_amount_morph);
        copy_deltas(&mut s.env_amount, dec.filter_env_amount_morph);
        s
    }

    /// `true` when any slot has a non-zero depth.
    pub fn is_active(&self) -> bool {
        [
            &self.osc_ctrl,
            &self.resonance,
            &self.lfo_rate,
            &self.lfo_amount,
            &self.env_amount,
        ]
        .into_iter()
        .any(active)
    }

    /// Writes the morphed values into `synth` (allocation free; untouched slots stay as loaded).
    pub fn apply(&self, ctrl: &MorphControllers, synth: &mut SynthLayer) {
        if active(&self.osc_ctrl) {
            crate::synth_params::set_osc_ctrl(synth, self.osc_ctrl.evaluate(ctrl), self.pulse_base);
        }
        if active(&self.resonance) {
            synth.resonance = self.resonance.evaluate(ctrl) * 0.95;
        }
        if active(&self.lfo_rate) {
            synth.lfo_rate_hz = lfo_norm_to_hz(self.lfo_rate.evaluate(ctrl));
        }
        if active(&self.lfo_amount) {
            synth.lfo_amount = self.lfo_amount.evaluate(ctrl);
        }
        if active(&self.env_amount) {
            synth.filter_env_amount = self.env_amount.evaluate(ctrl);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(base: f32, wheel: f32) -> MorphParameter {
        let mut p = MorphParameter::new(base);
        p.delta_wheel = wheel;
        p
    }

    fn wheel_full() -> MorphControllers {
        MorphControllers {
            mod_wheel: 1.0,
            ..MorphControllers::default()
        }
    }

    #[test]
    fn sender_norm_drops_a_step_from_half_a_knob() {
        let k = |x: f32| sender_norm(x / 127.0) * 127.0;
        assert!((k(54.0) - 54.0).abs() < 1e-4);
        assert!((k(54.4375) - 54.4375).abs() < 1e-4);
        assert!((k(54.5) - 53.5).abs() < 1e-4);
        assert!((k(59.5) - 58.5).abs() < 1e-4);
        assert!((k(127.0) - 127.0).abs() < 1e-4);
        assert_eq!(sender_norm(0.0), 0.0);
        assert!(sender_norm(0.2 / 127.0) < 0.2 / 127.0 + 1e-9);
    }

    #[test]
    fn inactive_set_leaves_the_voice_alone() {
        let mut synth = SynthLayer::new(48_000.0);
        synth.resonance = 0.3;
        let s = SynthMorphSet::default();
        assert!(!s.is_active());
        s.apply(&wheel_full(), &mut synth);
        assert_eq!(synth.resonance, 0.3);
    }

    #[test]
    fn env_amount_morph_adds_the_knob_steps() {
        // Horizon Whl layer B: base 32 steps, wheel +23 steps -> 55 steps (ARM run, 0x138 = 55).
        let mut synth = SynthLayer::new(48_000.0);
        let s = SynthMorphSet {
            env_amount: set(32.0 / 127.0, 23.0 / 127.0),
            ..SynthMorphSet::default()
        };
        assert!(s.is_active());
        s.apply(&wheel_full(), &mut synth);
        assert!((synth.filter_env_amount * 127.0 - 55.0).abs() < 1e-3);
        s.apply(&MorphControllers::default(), &mut synth);
        assert!((synth.filter_env_amount * 127.0 - 32.0).abs() < 1e-3);
    }

    #[test]
    fn resonance_maps_to_the_decoded_scale() {
        let mut synth = SynthLayer::new(48_000.0);
        let s = SynthMorphSet {
            resonance: set(0.0, 1.0),
            ..SynthMorphSet::default()
        };
        s.apply(&wheel_full(), &mut synth);
        assert!((synth.resonance - 0.95).abs() < 1e-6);
    }

    #[test]
    fn lfo_rate_follows_the_table_between_steps() {
        assert_eq!(lfo_norm_to_hz(0.0), LFO_RATE_HZ_TABLE[0]);
        assert_eq!(lfo_norm_to_hz(1.0), LFO_RATE_HZ_TABLE[127]);
        let mid = lfo_norm_to_hz(63.5 / 127.0);
        let (a, b) = (LFO_RATE_HZ_TABLE[63], LFO_RATE_HZ_TABLE[64]);
        assert!(mid > a && mid < b);
        let mut synth = SynthLayer::new(48_000.0);
        let s = SynthMorphSet {
            lfo_rate: set(0.0, 64.0 / 127.0),
            ..SynthMorphSet::default()
        };
        s.apply(&wheel_full(), &mut synth);
        assert!((synth.lfo_rate_hz - LFO_RATE_HZ_TABLE[64]).abs() < 1e-4);
    }

    #[test]
    fn lfo_amount_and_osc_ctrl_follow_the_sum() {
        let mut synth = SynthLayer::new(48_000.0);
        let s = SynthMorphSet {
            lfo_amount: set(0.25, 0.5),
            osc_ctrl: set(0.0, 1.0),
            pulse_base: 0.5,
            ..SynthMorphSet::default()
        };
        s.apply(&wheel_full(), &mut synth);
        assert!((synth.lfo_amount - 0.75).abs() < 1e-6);
        assert!((synth.supersaw_detune - 1.0).abs() < 1e-6);
        assert!((synth.wavetable_position - 1.0).abs() < 1e-6);
    }
}
