//! Rotary speaker speeds and accelerations of the firmware (System menu Leslie settings).
//!
//! The host firmware stores, per menu choice `0..2` (Low / Normal / High), the slow and fast speed
//! of the horn and the rotor and the per-update speed steps of the ramp. Source:
//! `docs/research/leslie-settings.md`, table `docs/research/data/leslie-settings.csv`, produced by
//! `python tools/arm_leslie_settings.py target/fw_unpacked/bank0_CODE.bin` (handlers
//! `0xC0112BD0`, `0xC0112C7C`, `0xC0112C14`, `0xC0112CC8`; choice table `0xC02BBC30` is the
//! identity `0, 1, 2`).

/// Choice index of the `Normal` setting (the settled firmware state).
pub const NORMAL: usize = 1;

/// VERIFIED: horn slow speed in nominal Hz per choice, `0xC0112F20` / `0xC0112F6C` /
/// `0xC0112F50`, checked by `tests::table_equals_the_committed_csv`, exact doubles.
pub const HORN_SLOW_HZ: [f32; 3] = [0.74, 0.80, 0.86];
/// VERIFIED: horn fast speed in nominal Hz per choice, same blocks and check.
pub const HORN_FAST_HZ: [f32; 3] = [6.5, 7.0, 7.5];
/// VERIFIED: rotor slow speed in nominal Hz per choice (`0xC0112CAC` / `0xC0112FCC` /
/// `0xC0112FB0`), same check.
pub const ROTOR_SLOW_HZ: [f32; 3] = [0.65, 0.73, 0.76];
/// VERIFIED: rotor fast speed in nominal Hz per choice, same blocks and check.
pub const ROTOR_FAST_HZ: [f32; 3] = [5.7, 6.4, 6.65];
/// VERIFIED: horn speed step per ARM update when accelerating, per choice (`0xC0112C44` /
/// `0xC0112F0C` / `0xC0112F3C`), same check. The decelerating step is the negative of it.
pub const HORN_STEP_UP: [f32; 3] = [0.03, 0.06, 0.12];
/// VERIFIED: rotor speed step per ARM update when accelerating, per choice (`0xC0112CF8` /
/// `0xC0112F9C` / `0xC0112F88`), same check.
pub const ROTOR_STEP_UP: [f32; 3] = [0.006, 0.01, 0.02];
/// VERIFIED: rotor speed step per ARM update when decelerating (magnitude), same blocks and check.
pub const ROTOR_STEP_DOWN: [f32; 3] = [0.012, 0.0218, 0.024];

/// VERIFIED: the speed words mean cycles of the DSP frame rate. The ARM converts Hz with the
/// 44.1 kHz nominal rate (`word = int(hz * 8388607 / 11025) << 8`, `tools/arm_leslie_speeds.py`)
/// while the DSP runs at `44_444_000 / 1024` Hz (`docs/research/dsp-frame-rate.md`), so a nominal
/// Hz of this module turns at `nominal * DSP_RATE_RATIO` Hz in the air.
pub const DSP_RATE_RATIO: f32 = (44_444_000.0 / 1024.0) / 44_100.0;

/// Clamps a menu choice to `0..=2`.
fn at(table: &[f32; 3], choice: usize) -> f32 {
    table[choice.min(2)]
}

/// Physical horn speed in Hz for a choice: `fast` selects the Fast speed, else Slow.
pub fn horn_hz(choice: usize, fast: bool) -> f32 {
    let t = if fast { &HORN_FAST_HZ } else { &HORN_SLOW_HZ };
    at(t, choice) * DSP_RATE_RATIO
}

/// Physical rotor speed in Hz for a choice.
pub fn rotor_hz(choice: usize, fast: bool) -> f32 {
    let t = if fast { &ROTOR_FAST_HZ } else { &ROTOR_SLOW_HZ };
    at(t, choice) * DSP_RATE_RATIO
}

/// Ramp time-constant factor of the horn acceleration choice (`1.0` at Normal; the ARM step size
/// is inversely proportional to the time).
pub fn horn_tau_scale(choice: usize) -> f32 {
    at(&HORN_STEP_UP, NORMAL) / at(&HORN_STEP_UP, choice)
}

/// Ramp time-constant factor of the rotor acceleration choice when speeding up.
pub fn rotor_tau_scale_up(choice: usize) -> f32 {
    at(&ROTOR_STEP_UP, NORMAL) / at(&ROTOR_STEP_UP, choice)
}

/// Ramp time-constant factor of the rotor acceleration choice when slowing down, relative to the
/// Normal speed-up time (the ARM brakes the rotor 2.18 times faster than it accelerates it).
pub fn rotor_tau_scale_down(choice: usize) -> f32 {
    at(&ROTOR_STEP_UP, NORMAL) / at(&ROTOR_STEP_DOWN, choice)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/research/data/leslie-settings.csv"
    ));

    fn csv(setting: &str, field: &str) -> [f32; 3] {
        let mut out = [f32::NAN; 3];
        for line in CSV.lines().skip(1) {
            let c: Vec<&str> = line.split(',').collect();
            if c[0] == setting && c[2] == field {
                out[c[1].parse::<usize>().unwrap()] = c[3].parse::<f64>().unwrap().abs() as f32;
            }
        }
        out
    }

    #[test]
    fn table_equals_the_committed_csv() {
        assert_eq!(HORN_SLOW_HZ, csv("HornSpeed", "slow_hz"));
        assert_eq!(HORN_FAST_HZ, csv("HornSpeed", "fast_hz"));
        assert_eq!(ROTOR_SLOW_HZ, csv("RotorSpeed", "slow_hz"));
        assert_eq!(ROTOR_FAST_HZ, csv("RotorSpeed", "fast_hz"));
        assert_eq!(HORN_STEP_UP, csv("HornAcc", "up_per_update"));
        assert_eq!(HORN_STEP_UP, csv("HornAcc", "down_per_update"));
        assert_eq!(ROTOR_STEP_UP, csv("RotorAcc", "up_per_update"));
        assert_eq!(ROTOR_STEP_DOWN, csv("RotorAcc", "down_per_update"));
    }

    #[test]
    fn normal_matches_the_settled_firmware_state() {
        // arm_leslie_speeds.py on the settled snapshot: horn 0.8 / 7.0, rotor 0.73 / 6.4
        assert_eq!(HORN_SLOW_HZ[NORMAL], 0.8);
        assert_eq!(HORN_FAST_HZ[NORMAL], 7.0);
        assert_eq!(ROTOR_SLOW_HZ[NORMAL], 0.73);
        assert_eq!(ROTOR_FAST_HZ[NORMAL], 6.4);
        assert!((horn_tau_scale(NORMAL) - 1.0).abs() < 1e-6);
        assert!((rotor_tau_scale_up(NORMAL) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dsp_rate_ratio_turns_nominal_into_physical_hz() {
        // 0x26000 is 0.786 Hz at the DSP rate (`arm_leslie_speeds.hz_of_word`)
        assert!((horn_hz(NORMAL, false) - 0.786).abs() < 0.002);
        assert!((rotor_hz(NORMAL, false) - 0.718).abs() < 0.002);
    }

    #[test]
    fn higher_choices_are_faster_and_accelerate_sooner() {
        for t in [&HORN_SLOW_HZ, &HORN_FAST_HZ, &ROTOR_SLOW_HZ, &ROTOR_FAST_HZ] {
            assert!(t[0] < t[1] && t[1] < t[2]);
        }
        assert!(horn_tau_scale(0) > 1.0 && horn_tau_scale(2) < 1.0);
        assert!(rotor_tau_scale_up(0) > 1.0 && rotor_tau_scale_up(2) < 1.0);
        assert!(rotor_tau_scale_down(NORMAL) < 0.5);
        assert_eq!(at(&HORN_FAST_HZ, 9), 7.5);
    }
}
