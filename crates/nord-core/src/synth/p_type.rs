//! Cutoff factor per filter type of the strip chain (applied with the P table).

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_factors_are_the_fitted_values() {
        assert_eq!(type_cutoff_scale(FilterModel::Lp12), 1.4);
        assert_eq!(type_cutoff_scale(FilterModel::BandPass), 1.4);
        assert_eq!(type_cutoff_scale(FilterModel::Lp24), 1.0);
        assert_eq!(type_cutoff_scale(FilterModel::HighPass), 1.0);
        assert_eq!(type_cutoff_scale(FilterModel::Ladder24), 0.8);
    }
}
