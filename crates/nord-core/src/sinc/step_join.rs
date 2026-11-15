//! Firmware step of a `SAMPLES` voice: the ARM zone word joined to the DSP table entry `e`.
//!
//! The SY audio interrupt forms `step = ratio(e) * 0.05 / word` ([`super::pitch_law::dsp_step`]).
//! `word` is the zone pitch word the ARM sends ([`super::pitch_law::zone_word`]); `e` is the 16-bit
//! packed float at `DM(0x9354A + index)`. No ARM message, no DSP routine and no DMA setup of the
//! SY1SY2 image writes `e` (`docs/research/synth-sample-kernel-rule.md` section 14), so the played
//! key enters the step through an entry this module only **models**: `e` = the key's pitch word
//! over the unity word, packed to 16 bits.
use super::pitch_law::{dsp_step, pitch_q16, pitch_word, zone_word, SY_FRAME_RATE_HZ, UNITY_WORD};

/// Ratio of a 16-bit table entry: with `E = e >> 11` (5 bits) and `m = (e & 0x7FF) / 2048`,
/// `0.625 * 2^(E - 1) * (1 + m)` for `E >= 1` and the denormal `0.625 * m` for `E = 0`.
///
/// VERIFIED: the six measured steps (`e = 0x0800 .. 0x1800`, `E = 1 .. 3`) of
/// `docs/research/sy-smp-audio.md` section 4, the oracle points of `dsp_step`, and the nine-note
/// oracle run `tools/oracle_scenarios/sy_pitch_nine.scn` (`E = 0` denormals and `E = 1 .. 2`, test
/// `step_join_matches_the_oracle_for_the_nine_notes`). `E = 0x1F` is not a ratio: the oracle clips
/// the step to its maximum (`0x08000000`); the entries `0xF458 ..` of an earlier signed reading do.
pub fn unpack_ratio(e: u16) -> f64 {
    let (exp, man) = (i32::from(e >> 11), f64::from(e & 0x7FF) / 2048.0);
    if exp == 0 {
        0.625 * man
    } else {
        0.625 * 2f64.powi(exp - 1) * (1.0 + man)
    }
}

/// Nearest 16-bit entry of a ratio (inverse of [`unpack_ratio`], round to nearest, saturating at
/// the largest finite exponent `E = 30`).
///
/// ASSUMED: the DSP stores the entry rounded to nearest (11-bit mantissa); only the unpack side is
/// measured.
pub fn pack_ratio(ratio: f64) -> u16 {
    let code = if ratio < 0.625 {
        (ratio / 0.625 * 2048.0).round() as i32
    } else {
        let exp = (ratio / 0.625).log2().floor() as i32 + 1;
        let man = ((ratio / (0.625 * 2f64.powi(exp - 1)) - 1.0) * 2048.0).round() as i32;
        (exp << 11) + man
    };
    code.clamp(0, 0xF7FF) as u16
}

/// Table entry that makes a zone-root sample play at the pitch of `key`: the key's pitch word
/// (`pitch_word(pitch_q16(key, 0, 0, 0, 0))`) over the unity word, packed.
///
/// ASSUMED: the writer of `e` is not found. This is the entry for which `dsp_step` equals the
/// native step `rate / 43402.34375` at the root key (a sample must play at its native rate when its
/// root key is struck), and it fits the nine captured zone words; it is not read from firmware.
pub fn table_entry_for_key(key: i32) -> u16 {
    pack_ratio(f64::from(pitch_word(pitch_q16(key, 0, 0, 0, 0))) / f64::from(UNITY_WORD))
}

/// Step of a `SAMPLES` voice in **source frames per SY frame** (`dsp_step` of the modelled entry
/// and the zone word).
pub fn sy_step(key: i32, root_key: i32, rate_hz: f64) -> f64 {
    dsp_step(
        unpack_ratio(table_entry_for_key(key)),
        zone_word(root_key, rate_hz),
    )
}

/// Source frames per **output** frame: [`sy_step`] scaled by the SY frame rate over the engine rate.
pub fn output_step(key: i32, root_key: i32, rate_hz: f64, engine_rate: f64) -> f64 {
    sy_step(key, root_key, rate_hz) * SY_FRAME_RATE_HZ / engine_rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpack_matches_the_six_measured_entries() {
        for (e, ratio) in [
            (0x0800, 0.625),
            (0x0A00, 0.78125),
            (0x0C00, 0.9375),
            (0x1000, 1.25),
            (0x1800, 2.5),
        ] {
            assert!((unpack_ratio(e) - ratio).abs() < 1e-12, "{e:#x}");
        }
        assert!((unpack_ratio(0x0B33) - 0.8749).abs() < 1e-4);
        // Denormals (E = 0) are linear in the mantissa and meet 0.625 at E = 1.
        assert_eq!(unpack_ratio(0x0000), 0.0);
        assert!((unpack_ratio(0x0400) - 0.3125).abs() < 1e-12);
        assert!((unpack_ratio(0x07FF) - 0.625 * 2047.0 / 2048.0).abs() < 1e-12);
    }

    #[test]
    fn pack_inverts_unpack_up_to_the_largest_finite_exponent() {
        for e in (0..=0xF7FFu16).step_by(7) {
            assert_eq!(pack_ratio(unpack_ratio(e)), e, "{e:#x}");
        }
    }

    #[test]
    fn entry_for_key_sixty_is_the_unit_entry_of_the_key_law() {
        let e = table_entry_for_key(60);
        let want = f64::from(pitch_word(pitch_q16(60, 0, 0, 0, 0))) / f64::from(UNITY_WORD);
        assert!((unpack_ratio(e) / want - 1.0).abs() < 1.0 / 2048.0);
    }

    #[test]
    fn root_key_plays_at_the_native_rate_within_the_tune_truncation() {
        for (root, rate) in [(60, 35_054.0), (48, 35_072.0), (97, 34_878.0)] {
            let got = sy_step(root, root, rate);
            let want = rate / SY_FRAME_RATE_HZ;
            // 11-bit entry rounding (0.025 %) plus one Q8 tune count (0.23 %).
            assert!((got / want - 1.0).abs() < 3e-3, "{root}: {got} vs {want}");
        }
    }

    #[test]
    fn output_step_rescales_to_the_engine_rate() {
        let sy = sy_step(64, 60, 40_000.0);
        let out = output_step(64, 60, 40_000.0, 48_000.0);
        assert!((out - sy * SY_FRAME_RATE_HZ / 48_000.0).abs() < 1e-15);
    }

    /// `(played key, zone root, stk rate Hz, oracle step word)`: the nine notes of `Carina Accordion
    /// 1` through `tools/oracle_scenarios/sy_pitch_nine.scn` (SY1SY2 OS v1.66, the entry and the
    /// zone word poked as `table_entry_for_key` and `zone_word` give them); derived numbers only.
    const ORACLE: [(i32, i32, f64, u32); 9] = [
        (48, 48, 35_072.0, 0x00CE_DD89),
        (59, 59, 35_076.0, 0x00CF_018B),
        (60, 60, 35_054.0, 0x00CE_C5D9),
        (71, 71, 35_098.0, 0x00CE_F5CD),
        (72, 72, 35_100.0, 0x00CF_0D30),
        (83, 82, 34_531.0, 0x00D7_CE54),
        (84, 85, 34_387.0, 0x00BF_7577),
        (95, 94, 35_445.0, 0x00DD_8751),
        (96, 97, 34_878.0, 0x00C2_27EE),
    ];

    #[test]
    fn step_join_matches_the_oracle_for_the_nine_notes() {
        for (key, root, rate, word) in ORACLE {
            let want = f64::from(word) / 16_777_216.0;
            let got = sy_step(key, root, rate);
            assert!(
                ((got - want) / want).abs() < 5e-5,
                "key {key}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn oracle_steps_sit_within_half_a_percent_of_the_semitone_law() {
        // The sample plays at 2^((key - root) / 12) of its native rate per SY frame.
        for (key, root, rate, word) in ORACLE {
            let native = rate / SY_FRAME_RATE_HZ * 2f64.powf(f64::from(key - root) / 12.0);
            let step = f64::from(word) / 16_777_216.0;
            assert!(
                (step / native - 1.0).abs() < 5e-3,
                "key {key}: {step} vs {native}"
            );
        }
    }
}
