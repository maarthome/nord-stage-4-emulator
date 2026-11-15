//! The System menu Leslie choices (Low / Normal / High) as ARM rotor sections.
//!
//! The host firmware stores, per menu choice `0..2`, the slow and fast speed of the horn and the
//! rotor and the per-update speed steps of their ramps. Source: ARM handlers `0xC0112BD0` (horn
//! speed), `0xC0112C7C` (rotor speed), `0xC0112C14` (horn acceleration) and `0xC0112CC8` (rotor
//! acceleration); the value-to-choice table `0xC02BBC30` is the identity `0, 1, 2`.
//! `python tools/arm_leslie_settings.py target/fw_unpacked/bank0_CODE.bin` evaluates them,
//! [`docs/research/leslie-settings.md`](../../../../docs/research/leslie-settings.md) has the
//! table and `docs/research/data/leslie-settings.csv` the numbers. Normal equals the settled
//! harness state, i.e. [`HORN`] and [`ROTOR`] of [`super::rotor_arm`].

use super::rotor_arm::Section;

/// Choice index of the `Normal` setting (the firmware default).
pub const NORMAL: usize = 1;

/// VERIFIED: horn slow speed (ARM nominal Hz) per choice, blocks `0xC0112F20` / `0xC0112F6C` /
/// `0xC0112F50`; checked against the CSV by `tests::tables_equal_the_committed_csv`, exact doubles.
pub const HORN_SLOW: [f64; 3] = [0.74, 0.80, 0.86];
/// VERIFIED: horn fast speed per choice, same blocks and check.
pub const HORN_FAST: [f64; 3] = [6.5, 7.0, 7.5];
/// VERIFIED: rotor slow speed per choice, blocks `0xC0112CAC` / `0xC0112FCC` / `0xC0112FB0`.
pub const ROTOR_SLOW: [f64; 3] = [0.65, 0.73, 0.76];
/// VERIFIED: rotor fast speed per choice, same blocks and check.
pub const ROTOR_FAST: [f64; 3] = [5.7, 6.4, 6.65];
/// VERIFIED: horn speed step per update, accelerating and braking alike, blocks `0xC0112C44` /
/// `0xC0112F0C` / `0xC0112F3C`.
pub const HORN_STEP: [f64; 3] = [0.03, 0.06, 0.12];
/// VERIFIED: rotor step per update while accelerating, blocks `0xC0112CF8` / `0xC0112F9C` /
/// `0xC0112F88`.
pub const ROTOR_UP: [f64; 3] = [0.006, 0.01, 0.02];
/// VERIFIED: rotor step per update while braking (magnitude), same blocks.
pub const ROTOR_DOWN: [f64; 3] = [0.012, 0.0218, 0.024];

fn at(table: &[f64; 3], choice: usize) -> f64 {
    table[choice.min(2)]
}

/// The horn section for a Horn Speed choice and a Horn Acceleration choice.
pub fn horn_section(speed: usize, acc: usize) -> Section {
    Section {
        slow: at(&HORN_SLOW, speed),
        fast: at(&HORN_FAST, speed),
        up: at(&HORN_STEP, acc),
        down: at(&HORN_STEP, acc),
    }
}

/// The rotor section for a Rotor Speed choice and a Rotor Acceleration choice.
pub fn rotor_section(speed: usize, acc: usize) -> Section {
    Section {
        slow: at(&ROTOR_SLOW, speed),
        fast: at(&ROTOR_FAST, speed),
        up: at(&ROTOR_UP, acc),
        down: at(&ROTOR_DOWN, acc),
    }
}

#[cfg(test)]
mod tests {
    use super::super::rotor_arm::{HORN, ROTOR};
    use super::*;

    const CSV: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/research/data/leslie-settings.csv"
    ));

    fn csv(setting: &str, field: &str) -> [f64; 3] {
        let mut out = [f64::NAN; 3];
        for line in CSV.lines().skip(1) {
            let c: Vec<&str> = line.split(',').collect();
            if c[0] == setting && c[2] == field {
                out[c[1].parse::<usize>().unwrap()] = c[3].parse::<f64>().unwrap().abs();
            }
        }
        out
    }

    #[test]
    fn tables_equal_the_committed_csv() {
        assert_eq!(HORN_SLOW, csv("HornSpeed", "slow_hz"));
        assert_eq!(HORN_FAST, csv("HornSpeed", "fast_hz"));
        assert_eq!(ROTOR_SLOW, csv("RotorSpeed", "slow_hz"));
        assert_eq!(ROTOR_FAST, csv("RotorSpeed", "fast_hz"));
        assert_eq!(HORN_STEP, csv("HornAcc", "up_per_update"));
        assert_eq!(HORN_STEP, csv("HornAcc", "down_per_update"));
        assert_eq!(ROTOR_UP, csv("RotorAcc", "up_per_update"));
        assert_eq!(ROTOR_DOWN, csv("RotorAcc", "down_per_update"));
    }

    #[test]
    fn normal_is_the_settled_firmware_state() {
        assert_eq!(horn_section(NORMAL, NORMAL), HORN);
        assert_eq!(rotor_section(NORMAL, NORMAL), ROTOR);
    }

    #[test]
    fn higher_choices_are_faster_and_ramp_in_fewer_updates() {
        for t in [&HORN_SLOW, &HORN_FAST, &ROTOR_SLOW, &ROTOR_FAST] {
            assert!(t[0] < t[1] && t[1] < t[2]);
        }
        let updates = |s: Section| {
            let (mut now, mut n) = (s.slow_hz(), 0);
            while now < s.fast_hz() && n < 100_000 {
                now = s.step(now, s.fast_hz(), 1.0);
                n += 1;
            }
            n
        };
        for pick in [horn_section, rotor_section] {
            let (low, normal, high) = (
                updates(pick(NORMAL, 0)),
                updates(pick(NORMAL, NORMAL)),
                updates(pick(NORMAL, 2)),
            );
            assert!(low > normal && normal > high, "{low} {normal} {high}");
        }
        assert_eq!(at(&HORN_FAST, 9), 7.5, "choices clamp to High");
    }
}
