//! Cutoff factor per filter type of the strip chain (applied with the P table).

use super::p_table::PWave;
use crate::synth_types::FilterModel;

/// Cutoff factor per filter type, applied with the table on top of the row factor (the strip
/// cutoff word of the displayed value is multiplied by it).
///
/// EMPIRICAL (`docs/research/synth-validation.md` sections 7 and 9; the Hz law of the stages
/// themselves, `R = tan(pi fc / F)`, is VERIFIED against the oracle, the displayed cutoff to
/// word map is not, `docs/research/synth-knob-laws.md` s6):
///
/// LP12 1.4 (grid 0.7 to 1.7 on `sweep_12db`, `mega_line`, `sine_pluck`, `lead_crunch`,
/// `pad_silk`), BP 1.4 (`filter_bank` presence -2.6 to -0.7 dB; 1.0 / 1.8 give score 4.64 /
/// 5.52 against 4.33), LP24 / HP 1.0 (LP24 1.3 is clearly worse, no clip for HP), Ladder24 0.8
/// (`sci_fi_80s` presence / brilliance +1.4 / +3.3 dB at 1.0, +3.8 / -4.4 at 0.5, +1.9 / -0.4 at
/// 0.8 with the MULTI gain 0.75; `phat_3osc`, filter open, 2.59 to 2.63).
pub fn type_cutoff_scale(model: FilterModel) -> f32 {
    match model {
        FilterModel::Lp12 | FilterModel::BandPass => 1.4,
        FilterModel::Lp24 | FilterModel::HighPass => 1.0,
        FilterModel::Ladder24 => 0.8,
    }
}

/// Type factor of the SUPER wave 0 row on an LP24 filter (D81). EMPIRICAL: `horizon_whl` (A:16,
/// LP24 277 / 554 Hz at -12 semitones, `docs/research/synth-validation.md` s12) wants a total
/// factor 2.0 with the verified words (grid on the row factor 0.45 / 1.0 / 2.0 / 4.0: score 3.4 /
/// 3.5 / 1.4 / 3.4 at LP24 factor 1.0); the row factor 0.45 (`pad_silk`, LP12 x 1.4) times 4.4 is
/// 1.98. One LP24 clip, so ASSUMED for other LP24 SUPER programs.
pub const SUPER_LP24_TYPE_SCALE: f32 = 4.4;

/// Total cutoff factor of a table row on a filter type: the row factor times the type factor
/// ([`type_cutoff_scale`]), with the SUPER wave 0 / LP24 exception [`SUPER_LP24_TYPE_SCALE`].
pub fn row_type_cutoff_scale(row: PWave, row_scale: f32, model: FilterModel) -> f32 {
    if row == PWave::Super && model == FilterModel::Lp24 {
        row_scale * SUPER_LP24_TYPE_SCALE
    } else {
        row_scale * type_cutoff_scale(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_wave_zero_on_lp24_has_its_own_total() {
        let t = |m| row_type_cutoff_scale(PWave::Super, 0.45, m);
        assert!(
            (t(FilterModel::Lp12) - 0.63).abs() < 1e-6,
            "pad_silk unchanged"
        );
        assert!((t(FilterModel::Lp24) - 1.98).abs() < 1e-6);
        let saw = row_type_cutoff_scale(PWave::PureSaw, 0.85, FilterModel::Lp24);
        assert!((saw - 0.85).abs() < 1e-6, "other rows keep row x type");
    }

    #[test]
    fn type_factors_are_the_fitted_values() {
        assert_eq!(type_cutoff_scale(FilterModel::Lp12), 1.4);
        assert_eq!(type_cutoff_scale(FilterModel::BandPass), 1.4);
        assert_eq!(type_cutoff_scale(FilterModel::Lp24), 1.0);
        assert_eq!(type_cutoff_scale(FilterModel::HighPass), 1.0);
        assert_eq!(type_cutoff_scale(FilterModel::Ladder24), 0.8);
    }
}
