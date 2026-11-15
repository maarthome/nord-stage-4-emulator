//! Key gesture thresholds read from the host firmware (v1.66).
//!
//! The firmware turns the key matrix into events with one key object. Per key it stores a first
//! delay and a repeat interval; the key event pipeline then emits a "held" event after the delay
//! and, when the interval is non-zero, a "repeat" event every interval. This module holds those
//! numbers as data so the gesture classifier can be checked against them.
//!
//! Reproduce the table (the firmware stays local, never committed):
//! `python tools/arm_key_timing.py rust target/fw_unpacked/bank0_CODE.bin`, decoded from the
//! init routine `0xC024C560..0xC024CA00` (49 calls of the setter `0xC0083248`).
//! Evidence and limits: `docs/research/firmware-gesture-timings.md`.

/// VERIFIED: key object defaults = `bank0_CODE.bin@0xC0083410` (`mov r2,#0x12C`, `mov r1,#0x50`),
/// checked by `tools/arm_key_timing.py table` (`decode_defaults`) and a live read of the key object
/// (`0x900434C0 + 0x24 / 0x28` hold 50 and 13 steps for keys outside the table), exact.
/// Keys that are not in [`KEY_TIMINGS`] hold for this long ...
pub const DEFAULT_HOLD_MS: u32 = 300;
/// VERIFIED: same routine and check as [`DEFAULT_HOLD_MS`]: ... and then repeat at this interval.
pub const DEFAULT_REPEAT_MS: u32 = 80;

/// VERIFIED: `bank0_CODE.bin@0xC0083250..0xC008326C` (`umull` by `0xAAAAAAAB`, `lsr #2`), checked
/// by `tools/arm_key_timing.py` (`steps`) and the live arrays (`500 ms -> 83`, `1000 -> 166`,
/// `350 -> 58`, `80 -> 13`), exact: the setter stores `ms / 6`, so every threshold is a whole number
/// of poll steps. `ASSUMED`: one poll step is 6 ms of real time (the divisor implies it; the poll
/// is a main-loop hook, not a timer interrupt, and its real cadence was not measured).
pub const POLL_STEP_MS: u32 = 6;

/// VERIFIED: `bank0_CODE.bin@0xC024C5E8..0xC024C9A8`, one row per `bl 0xC0083248` of the init
/// routine, rows `(key id, hold_ms, repeat_ms)`; checked by `tools/arm_key_timing.py rust` and the
/// live arrays of the booted key object (`delay24` / `repeat28` for the keys 0, 1, 16, 48, 49, 81,
/// 82, 114, 146, 147), exact. The key id is `column * 32 + bit` of the scan word (port `0x10`
/// bits sit at 16..23), the same id the UI event carries (`arm_events.KEY_IDS`).
pub const KEY_TIMINGS: [(u8, u16, u16); 49] = [
    (81, 1000, 0),
    (49, 1000, 0),
    (48, 1000, 0),
    (16, 500, 0),
    (2, 500, 0),
    (17, 500, 0),
    (20, 500, 0),
    (210, 500, 0),
    (83, 350, 0),
    (50, 350, 0),
    (51, 350, 0),
    (18, 350, 0),
    (160, 350, 0),
    (161, 350, 0),
    (70, 350, 0),
    (53, 500, 0),
    (55, 500, 0),
    (6, 500, 0),
    (212, 500, 0),
    (119, 500, 0),
    (176, 500, 0),
    (179, 500, 0),
    (134, 500, 0),
    (133, 500, 0),
    (129, 500, 0),
    (98, 500, 0),
    (99, 500, 0),
    (100, 500, 0),
    (132, 500, 0),
    (65, 500, 0),
    (162, 500, 0),
    (96, 500, 0),
    (64, 500, 0),
    (163, 500, 0),
    (130, 500, 0),
    (131, 500, 0),
    (67, 500, 0),
    (32, 500, 0),
    (34, 500, 0),
    (3, 500, 0),
    (0, 500, 0),
    (1, 500, 0),
    (5, 500, 0),
    (35, 500, 0),
    (37, 500, 0),
    (146, 350, 80),
    (147, 350, 80),
    (114, 350, 80),
    (82, 350, 80),
];

/// VERIFIED: ids of the three Morph Assign buttons in [`KEY_TIMINGS`] (rows with 1000 ms): key
/// `(strobe 0x02, port 0x10, bit 0)` = 48, `(0x02, 0x10, 1)` = 49, `(0x04, 0x10, 1)` = 81, matched
/// to the controls by the screens each opens (`docs/research/arm-oled-render.md` s7:
/// `MORPH A.TOUCH / PEDAL / WHEEL ASSIGN`), checked by `tools/arm_screens.py sweep`.
pub const MORPH_KEYS: [u8; 3] = [48, 49, 81];

/// Hold threshold of a key in milliseconds (the default for keys outside the table).
pub fn hold_ms(key: u8) -> u32 {
    timing(key).map_or(DEFAULT_HOLD_MS, |(_, hold, _)| u32::from(hold))
}

/// Auto-repeat interval of a key in milliseconds; `0` means the key does not repeat.
pub fn repeat_ms(key: u8) -> u32 {
    timing(key).map_or(DEFAULT_REPEAT_MS, |(_, _, repeat)| u32::from(repeat))
}

fn timing(key: u8) -> Option<(u8, u16, u16)> {
    KEY_TIMINGS.iter().copied().find(|(k, _, _)| *k == key)
}

/// Poll steps the firmware stores for `ms` (`ms / 6`, rounded down).
pub fn steps(ms: u32) -> u32 {
    ms / POLL_STEP_MS
}

/// Hold threshold of the panel control `id`, or `None` when its firmware key is not mapped.
///
/// VERIFIED for the five mapped controls: Master Clock (key 16) and Transpose (key 17) hold 500
/// ms, the three Morph Assign buttons (keys 48, 49, 81) hold 1000 ms. Other controls are not
/// mapped: the control to key-id map is only known for the keys listed in
/// `docs/research/arm-oled-render.md` s7.
pub fn control_hold_ms(id: &str) -> Option<u32> {
    let key = match id {
        "morph.at" => 48,
        "morph.ctrlped" => 49,
        "morph.wheel" => 81,
        "mst_clk.tap_set" => 16,
        "transp.on_set" => 17,
        _ => return None,
    };
    Some(hold_ms(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_one_row_per_setter_call_and_unique_keys() {
        let mut keys: Vec<u8> = KEY_TIMINGS.iter().map(|r| r.0).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 49);
    }

    #[test]
    fn hold_values_are_the_firmware_set() {
        let mut holds: Vec<u16> = KEY_TIMINGS.iter().map(|r| r.1).collect();
        holds.sort_unstable();
        holds.dedup();
        assert_eq!(holds, [350, 500, 1000]);
        let n = |h: u16| KEY_TIMINGS.iter().filter(|r| r.1 == h).count();
        assert_eq!((n(350), n(500), n(1000)), (11, 35, 3));
    }

    #[test]
    fn only_the_four_soft_keys_repeat_in_the_table() {
        let repeating: Vec<u8> = KEY_TIMINGS
            .iter()
            .filter(|r| r.2 != 0)
            .map(|r| r.0)
            .collect();
        assert_eq!(repeating, [146, 147, 114, 82]);
        assert!(KEY_TIMINGS
            .iter()
            .filter(|r| r.2 != 0)
            .all(|r| r.2 == 80 && r.1 == 350));
    }

    #[test]
    fn keys_outside_the_table_use_the_defaults() {
        // 36 = Shift, 118 = Page, 145 = Store, 21 = Undo (arm_events.KEY_IDS).
        for k in [36u8, 118, 145, 21, 255] {
            assert_eq!(
                (hold_ms(k), repeat_ms(k)),
                (DEFAULT_HOLD_MS, DEFAULT_REPEAT_MS)
            );
        }
    }

    #[test]
    fn steps_match_the_live_arrays_of_the_booted_key_object() {
        // `delay24` / `repeat28` read from the key object after the boot.
        assert_eq!(steps(500), 83);
        assert_eq!(steps(1000), 166);
        assert_eq!(steps(350), 58);
        assert_eq!(steps(80), 13);
        assert_eq!(steps(DEFAULT_HOLD_MS), 50);
    }

    #[test]
    fn morph_buttons_hold_one_second_and_master_clock_half_a_second() {
        for k in MORPH_KEYS {
            assert_eq!(hold_ms(k), 1000);
        }
        assert_eq!(control_hold_ms("morph.wheel"), Some(1000));
        assert_eq!(control_hold_ms("morph.at"), Some(1000));
        assert_eq!(control_hold_ms("morph.ctrlped"), Some(1000));
        assert_eq!(control_hold_ms("mst_clk.tap_set"), Some(500));
        assert_eq!(control_hold_ms("transp.on_set"), Some(500));
        assert_eq!(control_hold_ms("organ.preset"), None);
    }
}
