//! Tempo sync law of the delay (`kDlyClkEnable = 1`): `kDlyRateClkDiv` to a note division of the
//! Master Clock and the delay time word `c[0x15]` (`docs/research/fx-delay-tempo.md`).
//!
//! ARM half, handler `0xC010C514` branch `0xC010C828` (VERIFIED: listing, harness run, hook on the
//! clock object `0xC0024CC8`): with the enable flag set, `v` selects a division `n = DIVISION[v >> 7]`
//! (0..=16, table `0xC02B8B40 + 0xCC0`), the note length is `RATIO[n]` of a whole note (table
//! `+ 0xEC0`, `f32`), and the time in milliseconds is, in `f64`,
//! `ms = RATIO[n] * 4.0 * 60000.0 / bpm` (`bpm` = `[clock + 4]`, the Master Clock rate 30..=240;
//! captured at 120 bpm: `1000.0` ms for `n = 0`, `0x4084D55560000000` = `666.667` for `n = 2`).
//! DSP word: the delay unit method `0xC01FF70C` turns a time in ms into `c[0x15]` exactly as for the
//! free running path ([`super::fw_delay_time::word_from_ms`]); called directly with `ms * 5555.4993`
//! it wrote `floor(43.402339 * ms)` for 8 times in sync and in free mode alike (VERIFIED).
//! ASSUMED: the clock object passes the same scaled `ms` to the unit in sync mode (the notification
//! is rate limited by a time stamp in the harness and was not observed after a rate write).

use super::fw_delay_time::{word_from_ms, ECHO_LATENCY, FW_SAMPLE_RATE};

/// Master Clock range of `Edt_Com_kMasterClockRate` (`arm-edit-params.csv`).
pub const BPM_MIN: f64 = 30.0;
/// See [`BPM_MIN`].
pub const BPM_MAX: f64 = 240.0;

/// Division index of `v >> 7`. VERIFIED: ARM table `0xC02B8B40 + 0xCC0`, 128 `u32` entries.
#[rustfmt::skip]
const DIVISION: [u8; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1,
    2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 4,
    4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6,
    6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8,
    8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10,
    10, 10, 10, 11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12,
    12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14, 14,
    14, 15, 15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16, 16,
];

/// Note length in whole notes per division, `f32` bit patterns. VERIFIED: ARM table
/// `0xC02B8B40 + 0xEC0`. Entries 3, 7, 11 are the swing lengths `0.29`, `0.145`, `0.0725` (the
/// engine's `8/7` beat placeholder, `0.2857`, is replaced by them).
const RATIO_BITS: [u32; 17] = [
    0x3f000000, 0x3ec00000, 0x3eaaaaab, 0x3e947ae1, 0x3e800000, 0x3e400000, 0x3e2aaaab, 0x3e147ae1,
    0x3e000000, 0x3dc00000, 0x3daaaaab, 0x3d947ae1, 0x3d800000, 0x3d2aaaab, 0x3d000000, 0x3caaaaab,
    0x3c800000,
];

/// Division `0..=16` of a 14 bit `kDlyRateClkDiv` value.
pub fn division(v: u16) -> usize {
    usize::from(DIVISION[usize::from(v >> 7).min(127)])
}

/// Note length in beats (quarter notes) of a division: `4 * RATIO`.
pub fn beats(n: usize) -> f32 {
    f32::from_bits(RATIO_BITS[n.min(16)]) * 4.0
}

/// Delay time in ms at `bpm`: `f64` arithmetic (ratio, `* 4`, `* 60000`, `/ bpm`) and a round trip
/// through `f32` (the handler ends with `d2f` `0xC025D11C` and `f2d` `0xC025CA38`; VERIFIED by the
/// captured low word `0x60000000` of 666.667).
pub fn sync_ms(v: u16, bpm: f64) -> f64 {
    let ratio = f64::from(f32::from_bits(RATIO_BITS[division(v)]));
    f64::from((ratio * 4.0 * 60_000.0 / bpm.clamp(BPM_MIN, BPM_MAX)) as f32)
}

/// `c[0x15]` of the tempo synced delay: `min(61438, floor(K * ms))`.
pub fn sync_word(v: u16, bpm: f64) -> f32 {
    word_from_ms(sync_ms(v, bpm))
}

/// Samples from an input to its first echo at engine rate `sample_rate`.
pub fn sync_echo_samples_at(v: u16, bpm: f64, sample_rate: f32) -> f32 {
    (sync_word(v, bpm) + ECHO_LATENCY) * (sample_rate / FW_SAMPLE_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn division_table_is_monotone_and_covers_17_notes() {
        assert!(DIVISION.windows(2).all(|w| w[1] >= w[0]));
        assert_eq!((DIVISION[0], DIVISION[127]), (0, 16));
        assert_eq!(division(0), 0);
        assert_eq!(division(16_383), 16);
    }

    #[test]
    fn engine_step_map_equals_the_firmware_table() {
        // the engine reads `idx * 17 / 128` of the 7 bit step (`delay_sync_beats_from_index`)
        for (idx, &n) in DIVISION.iter().enumerate() {
            assert_eq!(usize::from(n), idx * 17 / 128, "step {idx}");
        }
    }

    #[test]
    fn engine_beats_match_except_for_the_swing_placeholders() {
        for n in 0..17_usize {
            let fw = beats(n);
            let eng = crate::fx::delay_sync_beats_from_index(n * 128 / 17 + 1);
            if matches!(n, 3 | 7 | 11) {
                assert!((fw - eng).abs() > 1e-3, "swing {n}");
            } else {
                assert!((fw - eng).abs() < 1e-6, "n {n}: {fw} vs {eng}");
            }
        }
        assert!((beats(3) - 1.16).abs() < 1e-6);
    }

    #[test]
    fn captured_times_at_120_bpm() {
        // harness run, unit L3, `kDlyClkEnable = 1`: the double at `[clock + 0x140]`
        assert_eq!(sync_ms(0, 120.0), 1000.0);
        assert_eq!(sync_ms(2048, 120.0).to_bits(), 0x4084_D555_6000_0000);
        assert_eq!(sync_ms(4096, 120.0).to_bits(), 0x407F_4000_0000_0000);
        assert_eq!(sync_ms(8192, 120.0).to_bits(), 0x406F_4000_0000_0000);
        assert_eq!(sync_ms(12288, 120.0).to_bits(), 0x405F_4000_0000_0000);
        assert_eq!(sync_ms(16383, 120.0).to_bits(), 0x403F_4000_0000_0000);
    }

    #[test]
    fn time_words_of_the_unit_method() {
        // `0xC01FF70C` called with `ms * 5555.4993`: (ms, c[0x15])
        let series = [
            (1000.0, 43_402.0),
            (500.0, 21_701.0),
            (666.0, 28_905.0),
            (250.0, 10_850.0),
            (100.0, 4_340.0),
            (1415.0, 61_414.0),
            (1500.0, 61_438.0),
            (2000.0, 61_438.0),
        ];
        for (ms, word) in series {
            assert_eq!(word_from_ms(ms), word, "ms {ms}");
        }
        assert_eq!(sync_word(0, 120.0), 43_402.0);
        assert_eq!(sync_word(0, 30.0), 61_438.0, "clamped at 1.416 s");
    }

    #[test]
    fn echo_scales_with_the_engine_rate() {
        let f = FW_SAMPLE_RATE;
        assert_eq!(sync_echo_samples_at(0, 120.0, f), 43_404.0);
        assert_eq!(sync_echo_samples_at(0, 120.0, 2.0 * f), 86_808.0);
    }
}
