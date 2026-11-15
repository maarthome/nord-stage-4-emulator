//! Panel values to firmware strip words and rate indices (opt-in `SynthChain::FirmwareStrip`).
//!
//! The ARM writes strip words, the DSP pass turns them into coefficients (`docs/research/
//! synth-adr-and-strip-law.md`). Which word the ARM writes for a knob position is open (idle edit
//! ids are value independent, `synth-voice-control.md` section 3), so every mapping here is the
//! inverse of a VERIFIED DSP law applied to the panel value the classic engine already uses:
//!
//! | Panel value | Strip word | Law (DSP side) | Tag of the mapping |
//! |---|---|---|---|
//! | Cutoff in Hz | word 3, `i` = top byte | `R = ramp(i)`, `fc = F atan(R) / pi` | ASSUMED (inverse of the law, display Hz is the cutoff) |
//! | Attack seconds | ADR word, index `-64..64` | `cA(idx)`, linear ramp of `1 / cA` passes | VERIFIED (time to full scale = display, ARM run) |
//! | Decay / release seconds | ADR word, index | `a(idx)`, `a^n` per pass, tau = `1 / (1 - a)` passes | VERIFIED by index (tau = display / ln 100 at plateau starts); constant 0.18 EMPIRICAL |
//!
//! A pass is [`STRIP_PASS_FRAMES`] SY frames, VERIFIED by the ARM run of `docs/research/
//! synth-knob-laws.md` s3: the ARM writes the rate index `knob - 64` and the attack ramp of two
//! frames per pass equals the displayed time at the first knob of each table plateau (ratio 1.00
//! to 1.01); the attack table spans 0.50 ms (index -64, 10.9 passes) to 46 s (index 64, 1.0e6
//! passes), the panel's 0.5 ms floor and 45 s ceiling. The displayed decay time is the time to fall
//! 40 dB at the first knob of a plateau (`tau = display / ln 100`); [`DECAY_TAU_FRACTION`] keeps
//! the clip-fitted 0.18 for layers without stored knobs.
//! What stays ASSUMED is the rest of the mapping: the ARM word of a displayed cutoff (section 6 of
//! the same note) and that the panel value of a layer without stored knobs is the display.

use super::{
    attack_coeff, decay_release_coeff, fw_cutoff_cycles_per_sample, fw_strip_cutoff_r,
    FW_CUTOFF_CLIP, SY_FRAME_RATE_HZ,
};

/// Ratio of the decay / release time constant to the displayed time.
///
/// EMPIRICAL 0.18 (the classic engine's `ENV_DECAY_TAU_FRACTION`, fitted on the laser-zap filter
/// clip at display 2.07 s). The firmware law is not a constant fraction: the displayed decay is the
/// time to fall 40 dB at the first knob of each plateau of the rate table (`tau = display / ln 100
/// = 0.2171 x display`, VERIFIED, `docs/research/synth-knob-laws.md` s3) and the ratio drifts
/// from 0.225 down to 0.169 over the plateau (the rate index repeats while the display keeps
/// rising), which is why 0.18 fits a mid-plateau clip. Layers that carry their stored knobs use
/// the exact index (`knob_laws`, default); this constant serves layers built without knobs, where
/// a flat 0.2171 would break that clip (checked: the `engine` golden segment moves 0.12 % in RMS).
pub const DECAY_TAU_FRACTION: f32 = 0.18;

/// VERIFIED: SY frames per envelope pass, 2: the attack ramp time `1 / cA(knob - 64)` passes equals
/// the displayed attack time at the first knob of each plateau, ratio 1.000 to 1.011 at knobs 0,
/// 16, ..., 112 (`docs/research/synth-knob-laws.md` s3; `synth::fw_knob_laws` test
/// `rate_tables_...`).
pub const STRIP_PASS_FRAMES: f32 = 2.0;

/// Strip table index of the cutoff word that gives `R = 0.36083` (`i = 0`); the ramp is
/// `R(i) = 0.36083 + 0.0079301 i` (`fw_strip_cutoff.rs`, EMPIRICAL closed form, 1.0e-6).
const R_BASE: f64 = 0.36083;
const R_STEP: f64 = 0.0079301;

/// Envelope passes per second at the SY frame rate.
#[inline]
pub fn passes_per_second() -> f32 {
    SY_FRAME_RATE_HZ / STRIP_PASS_FRAMES
}

/// Strip word 3 that makes the DSP use the cutoff `hz` (inverse of `R = tan(pi fc / F)` and the
/// ramp, interpolation fraction in the low 24 bits). Clipped to the pass control block limit.
///
/// ASSUMED: the ARM writes the word of the displayed cutoff; the DSP range is `R in [0, 1.0111]`,
/// so cutoffs above `0.2517 F` (10.9 kHz) and below `R = 0` saturate.
pub fn strip_word_for_cutoff(hz: f32) -> i32 {
    let fc = f64::from(hz.max(0.0)) / f64::from(SY_FRAME_RATE_HZ);
    let r = (std::f64::consts::PI * fc.min(0.49)).tan();
    // The DSP table is 0 at i = -46 and the ramp from i = -45, so the cell between interpolates
    // from 0 (`fw_strip_cutoff.rs`: `R(i) = clamp(ramp, 0, ..)` at integer indexes).
    let r_first = R_BASE - R_STEP * 45.0;
    let i = if r < r_first {
        -46.0 + r / r_first
    } else {
        ((r - R_BASE) / R_STEP).min(110.0)
    };
    ((i * f64::from(1u32 << 24)).round() as i32).clamp(-FW_CUTOFF_CLIP, FW_CUTOFF_CLIP)
}

/// Cutoff in Hz at the SY frame rate that the DSP derives from a strip word 3
/// (`fc = F atan(R) / pi`).
#[inline]
pub fn cutoff_hz_from_word(word: i32) -> f32 {
    fw_cutoff_cycles_per_sample(fw_strip_cutoff_r(word)) * SY_FRAME_RATE_HZ
}

/// The cutoff the strip law leaves for a panel cutoff: word, `R`, back to Hz.
#[inline]
pub fn strip_cutoff_hz(hz: f32) -> f32 {
    cutoff_hz_from_word(strip_word_for_cutoff(hz))
}

/// Integer table index in `-64..=64` whose coefficient is closest (in log of `c`) to `target`;
/// `coeff` is one of the rate lookups, `c(idx)` its small coefficient (`cA`, or `1 - a`).
fn nearest_index(target: f32, c: impl Fn(f32) -> f32) -> i32 {
    let want = f64::from(target.max(1e-9)).ln();
    let mut best = (-64, f64::INFINITY);
    for i in -64..=64 {
        let d = (f64::from(c(i as f32).max(1e-9)).ln() - want).abs();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Rate index of an attack that takes `seconds` to reach full scale (ramp of `1 / cA` passes).
pub fn attack_index(seconds: f32) -> i32 {
    let passes = (seconds.max(0.0) * passes_per_second()).max(1.0);
    nearest_index(1.0 / passes, attack_coeff)
}

/// Rate index of a decay or release with time constant `tau_sec` (`1 - a = 1 / (tau passes)`).
pub fn decay_index(tau_sec: f32) -> i32 {
    let passes = (tau_sec.max(0.0) * passes_per_second()).max(1.0);
    nearest_index(1.0 / passes, |i| 1.0 - decay_release_coeff(i))
}

/// Seconds to full scale of an attack index (inverse of [`attack_index`] within its step).
#[inline]
pub fn attack_seconds(index: i32) -> f32 {
    1.0 / attack_coeff(index as f32) / passes_per_second()
}

/// Time constant in seconds of a decay / release index.
#[inline]
pub fn decay_seconds(index: i32) -> f32 {
    1.0 / (1.0 - decay_release_coeff(index as f32)) / passes_per_second()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_word_round_trips_inside_the_dsp_range() {
        for hz in [
            20.0f32, 60.0, 150.0, 370.0, 800.0, 1_500.0, 3_000.0, 6_000.0, 10_000.0,
        ] {
            let back = strip_cutoff_hz(hz);
            assert!(
                (back / hz - 1.0).abs() < 2e-3,
                "{hz} Hz came back as {back} Hz"
            );
        }
    }

    #[test]
    fn cutoff_saturates_at_the_table_ends() {
        let top = strip_cutoff_hz(20_000.0);
        assert!((10_700.0..11_000.0).contains(&top), "top {top}");
        assert_eq!(strip_word_for_cutoff(-5.0), strip_word_for_cutoff(0.0));
        assert!(strip_cutoff_hz(0.0) < 0.01);
        assert!(strip_word_for_cutoff(1.0e9).abs() <= FW_CUTOFF_CLIP);
    }

    #[test]
    fn cutoff_index_is_the_top_byte_of_the_word() {
        // 370 Hz: R = tan(pi 370 / 43402.34) = 0.02679, i = (R - 0.36083) / 0.0079301 = -42.1.
        let w = strip_word_for_cutoff(370.0);
        assert_eq!(w >> 24, -43);
        let i = f64::from(w) / f64::from(1u32 << 24);
        assert!((i + 42.1).abs() < 0.05, "i = {i}");
    }

    #[test]
    fn pass_rate_matches_the_panel_time_range() {
        // Index -64 is the 0.5 ms floor, index 64 about 46 s (the panel attack ceiling is 45 s).
        assert!((attack_seconds(-64) - 0.000_5).abs() < 2e-5);
        assert!((attack_seconds(64) - 46.0).abs() < 1.0);
    }

    #[test]
    fn time_indices_hit_the_table_within_one_plateau() {
        // The tables repeat a value 5 to 6 times, so the achievable resolution is a factor of 3.
        for sec in [0.001f32, 0.005, 0.02, 0.1, 0.5, 2.0, 8.0] {
            let d = decay_seconds(decay_index(sec)) / sec;
            assert!((0.45..2.2).contains(&d), "decay {sec} s -> ratio {d}");
        }
        for sec in [0.001f32, 0.005, 0.02, 0.1, 0.5, 2.0, 10.0, 40.0] {
            let a = attack_seconds(attack_index(sec)) / sec;
            assert!((0.45..2.2).contains(&a), "attack {sec} s -> ratio {a}");
        }
        // Table ends: attack 0.5 ms .. 46 s, decay tau 0.67 ms .. 9.2 s.
        assert!((decay_seconds(-64) - 0.000_67).abs() < 5e-5);
        assert!((decay_seconds(64) - 9.2).abs() < 0.3);
    }

    #[test]
    fn longer_times_never_pick_a_faster_index() {
        let mut prev = -65;
        for k in 0..60 {
            let sec = 0.0005 * 1.2f32.powi(k);
            let i = attack_index(sec);
            assert!(i >= prev, "attack index fell at {sec} s");
            prev = i;
        }
        let mut prev = -65;
        for k in 0..60 {
            let i = decay_index(0.003 * 1.2f32.powi(k));
            assert!(i >= prev, "decay index fell");
            prev = i;
        }
    }
}
