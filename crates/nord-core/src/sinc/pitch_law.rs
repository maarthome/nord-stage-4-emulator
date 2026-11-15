//! Firmware pitch law of the SY sample voice sender (ARM `0xC01E14AC..0xC01E15A4`).
//!
//! The voice update function turns the played key into a float with integer code and two
//! soft-float calls (listing and run: `tools/arm_sy_pitch_law.py`, evidence in
//! `docs/research/synth-sample-kernel-rule.md` section 6):
//!
//! ```text
//! cents = trunc((([r6+0x14] + [r6+0x1C]) << 16) / 100)
//! x     = ((key + transpose - 60) << 16) - (tune << 8) - (cents + fine)    // Q16 semitones
//! m     = T1[(x >> 8) & 255] + ((T1[+1] - T1[]) * (x & 255) >> 8)          // 2^23 * 2^(f / 12)
//! n     = MANT[i % 12] * 2^-(17 - ceil(i / 12))      with i = clamp((x >> 16) + 128, 0, 255)
//! word  = float((m * n) >> 23 shifted) * 2^-23
//! ```
//!
//! The three firmware tables (`0xC02FA340` 257 words, `0xC02FA744` and `0xC02FAB44` 256 words) are
//! closed forms: `T1[i] = floor(2^23 * 2^(i / 3072))`, mantissa and shift repeat every 12
//! semitones. The code runs on every note start (no allocation).

use std::sync::OnceLock;

/// VERIFIED: `bank0_CODE@0x2FA744`, entries 0..12 (period 12 over the 256 entries, all 256 checked
/// by `tools/arm_sy_pitch_law.py` golden vectors and the table dump of the research note).
const MANTISSA: [u32; 12] = [
    0x7C_6E76, 0x41_EA50, 0x45_D5B6, 0x49_FCC6, 0x4E_630D, 0x53_0C4D, 0x57_FC81, 0x5D_37E1,
    0x62_C2E7, 0x68_A24D, 0x6E_DB19, 0x75_729B,
];

/// Firmware constant `0x51EB851F` (the multiply-high reciprocal of 100) and the `1/100` it makes.
const RECIP_100: i64 = 0x51EB_851F;

fn fine_table() -> &'static [u32; 257] {
    static T: OnceLock<[u32; 257]> = OnceLock::new();
    T.get_or_init(|| {
        // VERIFIED: bank0_CODE@0x2FA340 equals floor(2^23 * 2^(i/3072)) for all 257 entries.
        let mut t = [0u32; 257];
        for (i, v) in t.iter_mut().enumerate() {
            *v = (8_388_608.0_f64 * (i as f64 / 3072.0).exp2()).floor() as u32;
        }
        t
    })
}

/// Q16 semitone pitch `x` of a voice object (`key` and `transpose` are MIDI semitones, `cents_sum`
/// the sum of the two cent fields in hundredths of a semitone, `fine` a Q16 semitone offset,
/// `tune` a Q8 offset): the value the ARM block feeds into the exponential tables.
pub fn pitch_q16(key: i32, transpose: i32, cents_sum: i32, fine: i32, tune: i32) -> i32 {
    let scaled = cents_sum.wrapping_shl(16);
    let hi = ((i64::from(scaled) * RECIP_100) >> 32) as i32;
    let cents = (hi >> 5).wrapping_sub(scaled >> 31);
    (key.wrapping_add(transpose)
        .wrapping_sub(60)
        .wrapping_shl(16))
    .wrapping_sub(tune.wrapping_shl(8))
    .wrapping_sub(cents.wrapping_add(fine))
}

/// Integer result of the exponential block for a Q16 semitone pitch `x` (units of `2^-23`).
pub fn pitch_int(x: i32) -> i32 {
    let t = fine_table();
    let i = ((x >> 8) & 0xFF) as usize;
    let frac = i64::from(x & 0xFF);
    let (m0, m1) = (i64::from(t[i]), i64::from(t[i + 1]));
    let m = (m0 + (((m1 - m0) * frac) >> 8)) as i32;
    let idx = ((x >> 16) + 128).clamp(0, 255) as usize;
    let mant = MANTISSA[idx % 12] as i32;
    let shift = 17 - ((idx as i32 + 11) / 12);
    let (m_lo, m_hi) = (m & 0xFFFF, m >> 16);
    let (t_lo, t_hi) = (mant & 0xFFFF, mant >> 16);
    let mid = m_lo
        .wrapping_mul(t_hi)
        .wrapping_add(t_lo.wrapping_mul(m_hi));
    let low = mid.wrapping_add(((m_lo.wrapping_mul(t_lo)) as u32 >> 16) as i32);
    let r = (m_hi.wrapping_mul(t_hi).wrapping_shl(9)).wrapping_add(low >> 7);
    if shift >= 0 {
        r >> shift
    } else {
        r.wrapping_shl((-shift) as u32)
    }
}

/// The pair-block float the ARM sends: `float(pitch_int(x)) * 2^-23`.
pub fn pitch_word(x: i32) -> f32 {
    pitch_int(x) as f32 * (1.0 / 8_388_608.0)
}

/// Firmware playback ratio of a key relative to the root key: `word(key - root + 60) / word(60)`.
///
/// VERIFIED: the law (`pitch_word`, golden vectors). ASSUMED: that the sample root key enters as
/// a plain key offset; the ARM fields that carry the root and the sample rate are not decoded.
pub fn key_ratio(key: i32, root_key: i32) -> f64 {
    let at = |k: i32| f64::from(pitch_word(pitch_q16(k, 0, 0, 0, 0)));
    at(key - root_key + 60) / at(60)
}

/// SY frame rate in Hz: the reference of the per-zone tune field.
///
/// VERIFIED: `43402.34375 = 44,444,000 / 1024` (ARM literal `0x47298A58`, see `organ/lane_mix.rs`);
/// as the reference of `zone_tune_q8` it reproduces 9 of 9 captured `[zone + 0x10]` values.
pub const SY_FRAME_RATE_HZ: f64 = 43_402.343_75;

/// Pair-block pitch word the DSP treats as unity (`0x3D4CCCCD` = 0.05, the value the ARM boot
/// writes to every voice and the DSP step divides by).
///
/// VERIFIED: boot log `target/bulk/boot_log.jsonl` (all 56 voices), and the step join of
/// `dsp_step` on the SY oracle (8 `(e, word)` points).
pub const UNITY_WORD: f32 = 0.05;

/// Per-zone tune `[zone + 0x10]` (Q8 semitones) of a sample stroke of native rate `rate_hz`:
/// `trunc(3072 * log2(rate / SY_FRAME_RATE_HZ))`.
///
/// VERIFIED: the nine captured fields of `Carina Accordion 1` (`tools/arm_sy_voice.py`, zones with
/// rates 35054..35445 and 34387..34878 Hz from the `\0stk` header bytes `+6..+8`) equal this
/// truncation, residual of the unrounded value in `(-1, 0]` for all nine.
pub fn zone_tune_q8(rate_hz: f64) -> i32 {
    (3072.0 * (rate_hz / SY_FRAME_RATE_HZ).log2()) as i32
}

/// Pair-block pitch word the ARM sends for a zone: the law at the zone root key (`[zone + 0x60]`,
/// byte 2 of the `\0map` zone record) with the zone tune of its stroke rate. The played key does
/// not enter this word (mode 0), only the zone choice does.
///
/// VERIFIED: bit-identical to the captured word for 8 of 8 notes of `Carina Accordion 1`.
pub fn zone_word(root_key: i32, rate_hz: f64) -> f32 {
    pitch_word(pitch_q16(root_key, 0, 0, 0, zone_tune_q8(rate_hz)))
}

/// SY step (read increment of Stage B) of ISR step 3 (`0x1C0679..0x1C06E6`): `ratio(e) * 0.05 /
/// word`, where `ratio(e)` is the unpacked 16-bit table entry `DM(0x9354A + index)` and `word`
/// the pair-block pitch word `DM(0x9219E + 8)`.
///
/// VERIFIED: 8 `(e, word)` points on the SY oracle agree within 5e-5 relative (the DSP divides
/// with `recips` plus Newton steps). OPEN: which message writes `e` (none in the note-on).
pub fn dsp_step(table_ratio: f64, word: f32) -> f64 {
    table_ratio * f64::from(UNITY_WORD) / f64::from(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_tune_matches_the_nine_captured_fields() {
        // (stk rate Hz, captured [zone + 0x10]) from tools/arm_sy_voice.py on Carina Accordion 1.
        let rows = [
            (35072.0, -944),
            (35076.0, -943),
            (35054.0, -946),
            (35098.0, -941),
            (35100.0, -940),
            (34531.0, -1013),
            (34387.0, -1031),
            (35445.0, -897),
            (34878.0, -969),
        ];
        for (rate, tune) in rows {
            assert_eq!(zone_tune_q8(rate), tune, "rate {rate}");
        }
    }

    #[test]
    fn zone_word_matches_the_captured_pitch_words() {
        // (root key [zone + 0x60], stk rate Hz, captured float bits at 0x921A6).
        let rows = [
            (48, 35072.0, 0x3BF4_6900_u32),
            (59, 35076.0, 0x3C66_A400),
            (60, 35054.0, 0x3C74_8500),
            (71, 35098.0, 0x3CE6_8980),
            (72, 35100.0, 0x3CF4_30C0),
            (82, 34531.0, 0x3D5D_2960),
            (85, 34387.0, 0x3D84_09F0),
            (94, 35445.0, 0x3DD7_72D0),
            (97, 34878.0, 0x3E02_3460),
        ];
        for (root, rate, bits) in rows {
            assert_eq!(zone_word(root, rate).to_bits(), bits, "root {root}");
        }
    }

    #[test]
    fn dsp_step_matches_the_oracle_points() {
        // (ratio of e, pair-block word bits, oracle step word / 2^24), sy_voice_sine.scn variants.
        let rows = [
            (0.625, 0x3D4C_CCCC_u32, 0x00A0_0000_u32),
            (1.25, 0x3D4C_CCCC, 0x0140_0000),
            (0.625, 0x3C74_8500, 0x0218_09F8),
            (1.25, 0x3C74_8500, 0x0430_13F0),
            (0.625, 0x3CF4_30C0, 0x010C_6174),
            (1.25, 0x3CF4_30C0, 0x0218_C2E8),
            (0.625, 0x3E02_3460, 0x003E_EA97),
            (1.25, 0x3E02_3460, 0x007D_D52E),
        ];
        for (ratio, bits, step) in rows {
            let got = dsp_step(ratio, f32::from_bits(bits));
            let want = f64::from(step) / 16_777_216.0;
            assert!(
                ((got - want) / want).abs() < 5e-5,
                "{bits:#x}: {got} vs {want}"
            );
        }
    }

    /// `(key, transpose, cents sum, fine, tune, float bits)` from the firmware instructions run in
    /// `tools/arm_sy_pitch_law.py` (OS v1.66, `run_block`); derived numbers only.
    const GOLDEN: [(i32, i32, i32, i32, i32, u32); 23] = [
        (0, 0, 0, 0, 0, 0x39C58000),
        (60, 0, 0, 0, 0, 0x3C458580),
        (127, 0, 0, 0, 0, 0x3F13F98C),
        (36, 12, 0, 0, 0, 0x3BC58500),
        (60, -24, 0, 0, 0, 0x3B458400),
        (72, 0, 150, 0, 0, 0x3CB52100),
        (60, 0, -250, 0, 0, 0x3C643500),
        (60, 0, 0, 5000, 0, 0x3C44A700),
        (60, 0, 0, -7000, 0, 0x3C46BE80),
        (48, 0, 0, 0, 3, 0x3BC56300),
        (100, 7, 33, 1234, -2, 0x3E36CCD8),
        (82, -8, 104, -16836, -6, 0x3CD43180),
        (24, -1, 296, -16199, 8, 0x3A9F1800),
        (54, -11, -212, 8419, 5, 0x3BA5D300),
        (17, -5, -208, 16113, 5, 0x3A5B5800),
        (15, 6, -174, -5370, -7, 0x3AB8D000),
        (101, -11, -74, -16948, -4, 0x3D9418A0),
        (74, 1, -153, 15434, -5, 0x3CFD6B00),
        (78, 5, -115, -13247, -2, 0x3D49AC00),
        (95, -9, 260, -15886, -7, 0x3D41CA40),
        (52, 3, 244, 8022, 2, 0x3BFF1E00),
        (119, 6, 164, 3696, 1, 0x3EEEFF3C),
        (63, -7, -51, -14636, 1, 0x3C238500),
    ];

    #[test]
    fn matches_the_firmware_block_for_23_inputs() {
        for (k, t, c, f, tu, bits) in GOLDEN {
            let w = pitch_word(pitch_q16(k, t, c, f, tu));
            assert_eq!(
                w.to_bits(),
                bits,
                "key {k} tr {t} cents {c} fine {f} tune {tu}"
            );
        }
    }

    #[test]
    fn fine_table_spot_values_equal_the_firmware_words() {
        let t = fine_table();
        assert_eq!(
            (t[0], t[1], t[128], t[256]),
            (8_388_608, 8_390_500, 8_634_412, 8_887_420)
        );
    }

    #[test]
    fn root_key_ratio_is_unity_and_follows_semitones() {
        assert_eq!(key_ratio(60, 60), 1.0);
        assert!((key_ratio(72, 60) - 2.0).abs() < 1e-5);
        assert!((key_ratio(48, 60) - 0.5).abs() < 1e-5);
        assert!((key_ratio(67, 60) - 2f64.powf(7.0 / 12.0)).abs() < 1e-5);
    }

    #[test]
    fn deviation_from_equal_temperament_is_below_half_a_cent() {
        let base = f64::from(pitch_word(pitch_q16(60, 0, 0, 0, 0)));
        let worst = (0..=127)
            .map(|k| {
                let w = f64::from(pitch_word(pitch_q16(k, 0, 0, 0, 0)));
                (1200.0 * (w / base).log2() - 100.0 * f64::from(k - 60)).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.5, "{worst}");
    }

    #[test]
    fn pitch_q16_applies_each_field() {
        assert_eq!(pitch_q16(60, 0, 0, 0, 0), 0);
        assert_eq!(pitch_q16(61, 0, 0, 0, 0), 1 << 16);
        assert_eq!(pitch_q16(60, 2, 0, 0, 0), 2 << 16);
        assert_eq!(pitch_q16(60, 0, 100, 0, 0), -(1 << 16));
        assert_eq!(pitch_q16(60, 0, 0, 4, 1), -256 - 4);
    }
}
