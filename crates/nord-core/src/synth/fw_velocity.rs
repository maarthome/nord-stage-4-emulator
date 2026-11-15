//! Synth velocity laws of the ORG core 2 strip: amp velocity gain `V` and filter-envelope factor `F`.
//!
//! The ARM key message carries `g = key_velocity_g(vel)` (byte 1 = `0x80 + g`, [`super::key_velocity`])
//! in the controller word of the slot. The core 2 pass turns it into two gains (run of the real
//! pass with `sharc-org`, `tools/synth_velocity_law.py`, `docs/research/synth-velocity.md`):
//!
//! * Amp: `kAmpEnvVel` (setting `0..=3`) sends the pair `a = strip 0 word 53`, `b = strip 6 word 53`
//!   (`docs/research/data/synth-knob-laws.csv`); the slot word `DM(0xC10B4)` is
//!   `bits(V) = bits(a) + ((bits(a + b) - bits(a)) * G >> 7)`, i.e. a geometric blend from `a`
//!   (`-6 s` dB, `s` the setting) at `g = 0` to `a + b` (`+2 s` dB) at the top, `G = g` and `G = 128` for
//!   `g = 127`. Setting 0 is 1.0 for every velocity.
//! * Filter envelope: with `kFltEnvVel` on the peak of the filter envelope is scaled by
//!   `F = G / 128` (`DM(0xC2723)`), off it is 1.0.
//!
//! VERIFIED: every `V` and `F` word equals the oracle series of 768 rows within 1 float ulp (test
//! `law_matches_the_oracle_series`, `python tools/synth_velocity_law.py --check`). ASSUMED: that `V`
//! multiplies the voice amplitude and `F` the filter envelope depth the way the engine applies them
//! (the slot words are read by the envelope loop; the final multiply is not isolated).

/// VERIFIED: `kAmpEnvVel` strip 0 word 53 (`a`) for the settings `0..=3` (float32 bits).
const AMP_A: [u32; 4] = [0x3F80_0000, 0x3F00_4DCE, 0x3E80_9BCC, 0x3E00_E9F9];
/// VERIFIED: `bits(a + b)` for the settings `0..=3`, `b` = strip 6 word 53 `0, 0x3F41FB22, 0x3FAAB6D5,
/// 0x3FEF4782` (derived by the float add; equals the oracle word at `G = 128`).
const AMP_TOP: [u32; 4] = [0x3F80_0000, 0x3FA1_2478, 0x3FCA_DDC8, 0x3FFF_64C1];

/// `G`: the key byte `g = 127` acts as 128 (VERIFIED, oracle).
#[inline]
pub fn effective_g(g: u8) -> u32 {
    if g >= 127 {
        128
    } else {
        u32::from(g)
    }
}

/// Amp velocity gain `V` of the setting `0..=3` (clamped) and key byte `g` (`0..=127`).
#[inline]
pub fn amp_gain(setting: u8, g: u8) -> f32 {
    let s = usize::from(setting.min(3));
    let a = AMP_A[s];
    let span = AMP_TOP[s] - a;
    f32::from_bits(a + ((span * effective_g(g)) >> 7))
}

/// ARM key byte `g` of a normalised velocity (`0..=1`, rounded to the MIDI velocity).
#[inline]
pub fn key_byte(velocity: f32) -> u8 {
    super::key_velocity::key_velocity_g((velocity.clamp(0.0, 1.0) * 127.0).round() as u8)
}

/// Filter-envelope velocity factor `F = G / 128`.
#[inline]
pub fn filter_factor(g: u8) -> f32 {
    effective_g(g) as f32 / 128.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = include_str!("../../../../docs/research/data/synth-velocity-law.csv");

    fn rows() -> Vec<(String, usize, u8, u32)> {
        CSV.lines()
            .skip(1)
            .map(|l| {
                let c: Vec<&str> = l.trim().split(',').collect();
                let word = u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap();
                (
                    c[0].to_string(),
                    c[1].parse().unwrap(),
                    c[2].parse().unwrap(),
                    word,
                )
            })
            .collect()
    }

    #[test]
    fn law_matches_the_oracle_series() {
        let rows = rows();
        assert_eq!(rows.len(), 768);
        for (law, setting, g, word) in rows {
            let got = match law.as_str() {
                "amp" => amp_gain(setting as u8, g).to_bits(),
                _ if setting == 0 => 1.0f32.to_bits(),
                _ => filter_factor(g).to_bits(),
            };
            assert!(
                got.abs_diff(word) <= 1,
                "{law} {setting} g{g}: {got:#x} vs {word:#x}"
            );
        }
    }

    #[test]
    fn endpoints_are_the_dsp_pair_and_setting_zero_ignores_velocity() {
        for g in 0..=127u8 {
            assert_eq!(amp_gain(0, g), 1.0);
        }
        // g = 0: a = 10^(-0.3 s), top: 10^(0.1 s) (-6 s dB to +2 s dB).
        for s in 1..=3u8 {
            let lo = 10f32.powf(-0.3 * f32::from(s));
            let hi = 10f32.powf(0.1 * f32::from(s));
            assert!((amp_gain(s, 0) / lo - 1.0).abs() < 1e-3, "setting {s} low");
            assert!(
                (amp_gain(s, 127) / hi - 1.0).abs() < 1e-3,
                "setting {s} high"
            );
        }
        assert_eq!(filter_factor(0), 0.0);
        assert_eq!(filter_factor(64), 0.5);
        assert_eq!(filter_factor(127), 1.0);
    }

    #[test]
    fn gains_rise_monotonically_with_the_key_byte_and_the_setting_deepens_the_range() {
        for s in 1..=3u8 {
            for g in 1..=127u8 {
                assert!(amp_gain(s, g) >= amp_gain(s, g - 1), "setting {s} g{g}");
                assert!(filter_factor(g) >= filter_factor(g - 1));
            }
        }
        for g in [0u8, 32, 64] {
            assert!(amp_gain(3, g) < amp_gain(2, g) && amp_gain(2, g) < amp_gain(1, g));
        }
        assert!(amp_gain(3, 127) > amp_gain(2, 127) && amp_gain(2, 127) > amp_gain(1, 127));
        assert_eq!(amp_gain(9, 40), amp_gain(3, 40), "settings above 3 clamp");
    }
}
