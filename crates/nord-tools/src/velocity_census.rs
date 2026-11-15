//! Per-program synth velocity census: which factory layers respond to note velocity and by how much.
//!
//! One row per (program, layer). The program fields are the decoded `kAmpEnvVel` setting, the filter
//! envelope velocity flag and the oscillator-envelope velocity flag; the dB columns evaluate the
//! firmware law of `nord_core::synth::fw_velocity` (VERIFIED against the oracle) and the engine's
//! default law between two velocities. Derived numbers only; no audio, no firmware content.

use nord_core::synth::{fw_amp_velocity_gain, fw_filter_velocity_factor, key_velocity_g};

/// Header of the census CSV.
pub const HEADER: &str = "entry,program,layer,enabled,in_scene,sample,amp_setting,flt_vel,flt_on,osc_env_vel,amp_fw_db_v40_v100,amp_default_db_v40_v100,flt_fw_factor_v40,flt_fw_factor_v100";

/// Velocities compared in the dB columns (a soft and a firm note).
pub const LOW: u8 = 40;
pub const HIGH: u8 = 100;

fn db(ratio: f32) -> f32 {
    20.0 * ratio.log10()
}

/// Firmware amp gain change from [`LOW`] to [`HIGH`] in dB for a setting `0..=3`.
pub fn amp_fw_db(setting: u8) -> f32 {
    db(fw_amp_velocity_gain(setting, key_velocity_g(HIGH))
        / fw_amp_velocity_gain(setting, key_velocity_g(LOW)))
}

/// The engine's default law (linear blend of `g / 127`, strip chain) over the same velocities.
pub fn amp_default_db(setting: u8) -> f32 {
    let a = f32::from(setting.min(3)) / 3.0;
    let level = |v: u8| (1.0 - a) + a * f32::from(key_velocity_g(v)) / 127.0;
    db(level(HIGH) / level(LOW))
}

/// One CSV row (no trailing newline).
pub struct LayerFacts<'a> {
    pub entry: &'a str,
    pub program: &'a str,
    pub layer: char,
    pub enabled: bool,
    /// The layer bit (4..6) is set in a scene mask of the program.
    pub in_scene: bool,
    /// The layer resolves to a sample sound of the backup (no sample: an oscillator layer).
    pub sound: bool,
    pub amp_setting: u8,
    pub filter_velocity: bool,
    pub filter_on: bool,
    pub osc_env_velocity: bool,
}

/// CSV line of a layer.
pub fn row(f: &LayerFacts) -> String {
    let flt = |v: u8| {
        if f.filter_velocity {
            fw_filter_velocity_factor(key_velocity_g(v))
        } else {
            1.0
        }
    };
    format!(
        "{},{},{},{},{},{},{},{},{},{},{:.2},{:.2},{:.4},{:.4}",
        f.entry.replace(',', ";"),
        f.program.replace(',', ";"),
        f.layer,
        u8::from(f.enabled),
        u8::from(f.in_scene),
        u8::from(f.sound),
        f.amp_setting,
        u8::from(f.filter_velocity),
        u8::from(f.filter_on),
        u8::from(f.osc_env_velocity),
        amp_fw_db(f.amp_setting),
        amp_default_db(f.amp_setting),
        flt(LOW),
        flt(HIGH),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_zero_is_flat_and_depth_grows_with_the_setting() {
        assert_eq!(amp_fw_db(0), 0.0);
        assert_eq!(amp_default_db(0), 0.0);
        for s in 1..=3 {
            assert!(amp_fw_db(s) > amp_fw_db(s - 1), "setting {s}");
            assert!(amp_default_db(s) > amp_default_db(s - 1), "setting {s}");
        }
        // Setting 3: +6 dB at the top and -18 dB at the bottom of the key byte.
        assert!(amp_fw_db(3) > 6.0 && amp_fw_db(3) < 24.0);
    }

    #[test]
    fn rows_have_the_header_width_and_escape_commas() {
        let f = LayerFacts {
            entry: "a,b.ns4p",
            program: "Horizon, Whl",
            layer: 'A',
            enabled: true,
            in_scene: true,
            sound: true,
            amp_setting: 3,
            filter_velocity: true,
            filter_on: true,
            osc_env_velocity: false,
        };
        let line = row(&f);
        assert_eq!(line.split(',').count(), HEADER.split(',').count());
        assert!(line.starts_with("a;b.ns4p,Horizon; Whl,A,1,1,1,3,1,1,0,"));
        let off = row(&LayerFacts {
            filter_velocity: false,
            ..f
        });
        assert!(off.ends_with(",1.0000,1.0000"));
    }
}
