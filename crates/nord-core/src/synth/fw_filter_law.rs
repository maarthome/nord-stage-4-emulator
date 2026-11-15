//! Filter laws of the strip chain read from the firmware (opt-in, `--synth-filter-law`).
//!
//! Two derived facts replace the fitted per-type cutoff factors of [`super::p_type`] and the
//! linear resonance to table position map of `TablePos::from_resonance`:
//!
//! * **Resonance word to table position (VERIFIED, run).** The ARM writes `kFltRes` as the float
//!   word `w = knob / 127 - 0.5` to the common block word 7 (`docs/research/synth-knob-laws.md`
//!   s3). The ORG core 2 pass turns it into the stage position word `W28 = DM(0xC2926)` with
//!   `idx = 64 + 33.25 w` (the idle LFO scale 0.5 neutral): the oracle (`tools/synth_filter_res_probe.py`)
//!   gives `47.3767 + 0.261784 knob`, fit error 0 over 15 knobs, range 47.38 to 80.62. The position
//!   never reaches the table end, so the maximum of the stage's resonance is `Q = 1.9`, not 90.
//! * **One nominal cutoff for every type (VERIFIED, derived).** The ARM cutoff word is type
//!   independent and every stage of the family takes the same coefficient `R` with the nominal
//!   cutoff `atan(R) / pi` cycles per sample (`docs/research/synth-filter-type-corners.md`), so
//!   there is no per-type cutoff factor; the shape differences between the types live in the
//!   stages that [`super::fw_filter`] already ports.
//!
//! What stays ASSUMED is the unit: the display shows `1.971 x` the natural frequency
//! `F atan(R) / pi` ([`DISPLAY_OVER_NATURAL`]), and whether the stage runs at `F` or `2 F`
//! (`UNIT_F` / `UNIT_2F`) is the open capture of `docs/research/synth-cutoff-voice-law.md` s4.

/// VERIFIED: slope of the stage table position per unit of the `kFltRes` word, `33.25`
/// (`tools/synth_filter_res_probe.py`, fit `47.37672 + 0.261784 knob`, exact over 15 knobs).
pub const RES_POSITION_SPAN: f32 = 33.25;
/// VERIFIED: table position at the neutral word `w = 0` (`W28 = 0x40000000`, idle).
pub const RES_POSITION_CENTRE: f32 = 64.0;

/// VERIFIED: display Hz over the natural frequency `F atan(R) / pi` of the stage (mean 1.971,
/// range 1.88 to 2.06 over knobs 8..116, `docs/research/data/synth-cutoff-voice-law.csv`).
pub const DISPLAY_OVER_NATURAL: f32 = 1.971;
/// ASSUMED: stage rate `F` (one stage sample per DSP frame): the engine cutoff is the natural
/// frequency, `1 / 1.971` of the display.
pub const UNIT_F: f32 = 1.0 / DISPLAY_OVER_NATURAL;
/// ASSUMED: stage rate `2 F`: the corner in Hz doubles, `2 / 1.971` of the display.
pub const UNIT_2F: f32 = 2.0 / DISPLAY_OVER_NATURAL;

/// Table position (`0..=127`) of the firmware resonance word for a panel resonance `0..=1`
/// (`knob / 127`).
#[inline]
pub fn resonance_position(resonance: f32) -> f32 {
    let w = resonance.clamp(0.0, 1.0) - 0.5;
    RES_POSITION_CENTRE + RES_POSITION_SPAN * w
}

/// Resonance `0..=1` that `TablePos::from_resonance` maps to the firmware position.
#[inline]
pub fn resonance_for_table(resonance: f32) -> f32 {
    resonance_position(resonance) / 127.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::TablePos;

    #[test]
    fn position_matches_the_oracle_fit() {
        // oracle: idx = 47.37672 + 0.261784 knob (tools/synth_filter_res_probe.py)
        for knob in [0.0f32, 32.0, 64.0, 127.0] {
            let want = 47.376_72 + 0.261_784 * knob;
            let got = resonance_position(knob / 127.0);
            assert!((got - want).abs() < 0.01, "{knob}: {got} vs {want}");
        }
    }

    #[test]
    fn range_never_reaches_the_table_ends() {
        assert!((resonance_position(0.0) - 47.375).abs() < 1e-4);
        assert!((resonance_position(1.0) - 80.625).abs() < 1e-4);
        assert!((resonance_position(0.5) - 64.0).abs() < 1e-6);
    }

    #[test]
    fn table_position_follows_the_map() {
        let p = TablePos::from_resonance(resonance_for_table(1.0));
        assert_eq!(p.idx, 80);
        assert!(p.fr > 0.5 && p.fr < 0.7);
    }

    #[test]
    fn units_are_one_over_and_two_over_the_display_ratio() {
        assert!((UNIT_2F / UNIT_F - 2.0).abs() < 1e-6);
        assert!((UNIT_2F - 1.0147).abs() < 1e-3);
    }
}
