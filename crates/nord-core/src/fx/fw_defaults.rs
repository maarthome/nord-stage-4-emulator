//! Firmware default FX parameters (`--fx-defaults firmware`, opt-in).
//!
//! The ARM edit-parameter table holds one default word per `Edt_*` id (`arm-edit-params.csv`,
//! the D97 method). `tools/fx_defaults.py` converts them to the emulator's units and writes
//! `fw_defaults_table.rs`; this file applies the table to a [`LayerFxChain`]. It matters only
//! for an engine with no program loaded: `apply_fx_chain` overwrites every field on a load.
//!
//! VERIFIED (default words): `docs/research/data/fx-defaults.csv`, checked by
//! `python tools/fx_defaults.py --check`. ASSUMED: that the hardware starts a fresh layer from
//! these words (no clip of an init patch), so the flag stays off (D22, D101).

use super::fw_defaults_table::FW_DEFAULTS;
use super::LayerFxChain;

impl LayerFxChain {
    /// Sets every chain field of the table to the firmware default.
    pub fn apply_firmware_defaults(&mut self) {
        for &(field, v) in FW_DEFAULTS.iter() {
            *self.default_field(field) = v;
        }
    }

    /// Mutable access to the field a `FW_DEFAULTS` row names.
    fn default_field(&mut self, field: &str) -> &mut f32 {
        match field {
            "mod1_amount" => &mut self.mod1_amount,
            "mod2_amount" => &mut self.mod2_amount,
            "eq_mid_freq_hz" => &mut self.eq_mid_freq_hz,
            "comp_amount" => &mut self.comp_amount,
            "delay_time_ms" => &mut self.delay_time_ms,
            "delay_mix" => &mut self.delay_mix,
            "delay_feedback" => &mut self.delay_feedback,
            "reverb_mix" => &mut self.reverb_mix,
            other => panic!("FW_DEFAULTS names an unknown field: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_row_lands_in_its_field() {
        let mut c = LayerFxChain::new(48_000.0);
        c.apply_firmware_defaults();
        for &(field, v) in FW_DEFAULTS.iter() {
            assert_eq!(*c.default_field(field), v, "{field}");
        }
    }

    #[test]
    fn the_firmware_defaults_differ_from_the_engine_defaults() {
        let base = LayerFxChain::new(48_000.0);
        let mut c = LayerFxChain::new(48_000.0);
        c.apply_firmware_defaults();
        assert_eq!(c.delay_time_ms, 250.0);
        assert_ne!(c.delay_time_ms, base.delay_time_ms);
        assert!((c.comp_amount - 0.748).abs() < 1e-3 && base.comp_amount == 0.0);
        assert!((c.reverb_mix - 0.2992).abs() < 1e-3);
    }

    #[test]
    fn applying_twice_is_idempotent() {
        let mut a = LayerFxChain::new(48_000.0);
        a.apply_firmware_defaults();
        let mut b = LayerFxChain::new(48_000.0);
        b.apply_firmware_defaults();
        b.apply_firmware_defaults();
        for &(f, _) in FW_DEFAULTS.iter() {
            assert_eq!(*a.default_field(f), *b.default_field(f));
        }
    }
}
