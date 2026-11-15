//! The ARM generator of the FM band tables as a function of `(type, a4, a8)`.
//!
//! Ported from the listing `0xC01CB0DC .. 0xC01CBF38` of the strip apply `0xC01CE7D8`
//! (`docs/research/fm-banks-1-2.md` s8, `tools/synth_fm_generator.py`): VERIFIED bit exact against
//! the ARM harness on 30 of 30 variants (5 types x `a4` in {0, 1} x `a8` in {0x20000, 0x80000,
//! 0x180000}, 10,830 words, max 0 ulp in Python; this port within 1 ulp, measured by the golden
//! test). Three curves of 129 floats are built per index `j` with `s = -132 + 2 j`:
//!
//! ```text
//! Sc = float(a8) 1.2014794e-7 127              (a4 = 0)      = 2^((((a8 + 0x8000) >> 16) - 12) / 12)  (a4 != 0)
//! E = s / 12;  x = 2^(E - 7);  r = Sc x;  k = 2 pi r;  g = fade(0.6912 - r)
//! c0 = max(0, 1 - 0.2 E) / Sc  (type 49), / (0.9 + 0.1 Sc)  (type 52), unscaled otherwise
//! c1 = max(0, (c30 - x c24) / k - 0.4138)          c30 = 0.75 (types 50..52), 1.5 otherwise
//! c2 = max(0, (1.5 - x c28) / den - 0.4138)        den = 2 pi x c20            (types 49, 53)
//!                                                  den = k (min(c1, (c18 + c1c) c0 g) + 0.4138) + x c24  (types 50..52)
//! ```
//!
//! The table is `c0[26..129] ++ c1 ++ c2` (`DM(0xE0900 ..)`). Not real time (about 400 `powf`
//! calls): build it when a note or a wave is selected, never inside `process_block`.

use super::table::{FmTable, FIRST_TYPE, TABLE_WORDS, TYPES};

/// Curve length.
const N: usize = 129;
/// Curve 0 words that precede `DM(0xE0900)` and are not sent.
const SKIP: usize = 26;
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);
const OFFSET: f32 = f32::from_bits(0x3ED3_DDF7);
const CLIP: f32 = f32::from_bits(0x3E6B_EF0F);
const K_A8: f32 = f32::from_bits(0x3401_0204);
const LN2_12: f32 = f32::from_bits(0x3D6C_9820);
const FADE_TOP: f64 = f64::from_bits(0x3FE6_1E69_6800_0000);
const FADE_GAIN: f64 = f64::from_bits(0x4001_5C5D_6A14_2504);
/// VERIFIED (run, firmware table `0xC02F9E88 + 44 t`, offsets `0xEC, 0xF8, 0x10C, 0x110, 0x114`):
/// per-type words `c18, c1c, c20, c24, c28`.
const TYPE_WORDS: [[u32; 5]; TYPES] = [
    [0x0, 0x4150_0000, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000],
    [0x3EC7_AE14, 0x0, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000],
    [0x3EC7_AE14, 0x0, 0x3F80_0000, 0x4040_0000, 0x3F80_0000],
    [0x3D75_C28F, 0x0, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000],
    [0x0, 0x3FE4_8E8A, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000],
];

/// `Sc` from strip `+0xA4` / `+0xA8` (default `a4 = 0`, `a8 = 0x80000`: 8.0).
pub fn scale(a4: u32, a8: u32) -> f32 {
    let a8 = a8 as i32;
    if a4 == 0 {
        (a8 as f32 * K_A8) * 127.0
    } else {
        let n = (a8.wrapping_add(0x8000)) >> 16;
        f64::from((n as f32 - 12.0) * LN2_12).exp() as f32
    }
}

/// The fade of the generator: 1 above `+0.2304`, 0 below `-0.2304`, `(t + 0.2304) 2.17` between.
fn fade(t: f32) -> f32 {
    if t > CLIP {
        1.0
    } else if t < -CLIP {
        0.0
    } else {
        (f64::from(t + CLIP) * FADE_GAIN) as f32
    }
}

/// `max(0, v)` through double, as the firmware helper `0xC02712D8` does.
fn pos(v: f32) -> f32 {
    f64::from(v).max(0.0) as f32
}

/// Strip `(+0xA4, +0xA8)` the `kOscWave` edit stores for an FM bank wave (`None`: not an FM wave).
///
/// VERIFIED (run, `tools/synth_fm_tables.py select`, 5 categories x 86 waves in
/// `docs/research/data/synth-fm-select.csv`; the handler stores `+0xA8` at `0xC00185B0` and `+0xA4`
/// at `0xC00185D8`): bank 1 (`FM-H`, waves 0..24 = partials 0.5, 1, 2 .. 24) gives `a4 = 0`,
/// `a8 = ratio 2^16`; bank 2 (`FM-I`, waves 0..60 = `wave - 12` semitones) gives `a4 = 1`,
/// `a8 = wave 2^16`. The category 0..4 is the strip type `49 + category`.
pub fn partial_words(bank: u8, wave: u8) -> Option<(u32, u32)> {
    match (bank, wave) {
        (1, 0) => Some((0, 0x8000)),
        (1, 1..=24) => Some((0, u32::from(wave) << 16)),
        (2, 0..=60) => Some((1, u32::from(wave) << 16)),
        _ => None,
    }
}

/// Table of strip type `ty` (49..=53) for the strip words `a4`, `a8`.
pub fn fm_table_for(ty: u8, a4: u32, a8: u32) -> Option<FmTable> {
    let t = usize::from(ty.checked_sub(FIRST_TYPE)?);
    let w = TYPE_WORDS.get(t)?.map(f32::from_bits);
    let (c18, c1c, c20, c24, c28) = (w[0], w[1], w[2], w[3], w[4]);
    let sc = scale(a4, a8);
    let c30 = if (50..=52).contains(&ty) {
        0.75f32
    } else {
        1.5f32
    };
    let mut out = [0u32; TABLE_WORDS];
    for j in 0..N {
        let e = f64::from((-132 + 2 * j as i32) as f32 / 12.0f32);
        let mut c0 = (1.0 + e * -0.2) as f32;
        if c0 < 0.0 {
            c0 = 0.0;
        }
        c0 = match ty {
            49 => c0 / sc,
            52 => (f64::from(c0) / (f64::from(sc) * 0.1 + 0.9)) as f32,
            _ => c0,
        };
        let x = (2f64.powf(e) * 2f64.powi(-7)) as f32;
        let r = sc * x;
        let sl = x * c24;
        let sb = 1.5f32 - x * c28;
        let g = fade((FADE_TOP - f64::from(r)) as f32);
        let k = r * TWO_PI;
        let c1 = pos((c30 - sl) / k - OFFSET);
        let den = if ty == 49 || ty == 53 {
            x * c20 * TWO_PI
        } else {
            let m = f64::from(c1).min(f64::from((c18 + c1c) * c0 * g)) as f32;
            (m + OFFSET) * k + sl
        };
        let c2 = pos(sb / den - OFFSET);
        if j >= SKIP {
            out[j - SKIP] = c0.to_bits();
        }
        out[N - SKIP + j] = c1.to_bits();
        out[2 * N - SKIP + j] = c2.to_bits();
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str =
        include_str!("../../../../../docs/research/data/synth-fm-generator-cases.csv");

    fn ulps(a: u32, b: u32) -> u32 {
        a.abs_diff(b)
    }

    #[test]
    fn scale_matches_the_two_laws() {
        assert_eq!(scale(0, 0x80000), 8.0);
        assert!((scale(1, 0x80000) - 2f32.powf(-4.0 / 12.0)).abs() < 1e-6);
        assert!((scale(0, 0x40000) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn captured_cases_match_within_one_ulp() {
        let mut n = 0;
        for line in CASES.lines().skip(1) {
            let f: Vec<&str> = line.splitn(4, ',').collect();
            let ty: u8 = f[0].parse().unwrap();
            let a4: u32 = f[1].parse().unwrap();
            let a8 = u32::from_str_radix(f[2].trim_start_matches("0x"), 16).unwrap();
            let want: Vec<u32> = f[3]
                .split(' ')
                .map(|h| u32::from_str_radix(h, 16).unwrap())
                .collect();
            let got = fm_table_for(ty, a4, a8).unwrap();
            assert_eq!(want.len(), TABLE_WORDS);
            let worst = got
                .iter()
                .zip(&want)
                .map(|(a, b)| ulps(*a, *b))
                .max()
                .unwrap();
            assert!(worst <= 1, "type {ty} a4 {a4} a8 {a8:#x}: {worst} ulp");
            n += 1;
        }
        assert_eq!(n, 15);
    }

    #[test]
    fn default_words_equal_the_committed_table() {
        for ty in 49..=53u8 {
            let got = fm_table_for(ty, 0, 0x80000).unwrap();
            let want = super::super::fm_table(ty).unwrap();
            let worst = got
                .iter()
                .zip(want.iter())
                .map(|(a, b)| ulps(*a, *b))
                .max()
                .unwrap();
            assert!(worst <= 1, "type {ty}: {worst} ulp");
        }
    }

    #[test]
    fn other_types_have_no_table() {
        assert!(fm_table_for(48, 0, 0x80000).is_none());
        assert!(fm_table_for(54, 0, 0x80000).is_none());
    }

    #[test]
    fn partial_words_equal_the_captured_wave_edit() {
        const SEL: &str = include_str!("../../../../../docs/research/data/synth-fm-select.csv");
        let mut n = 0;
        for line in SEL.lines().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            let bank: u8 = f[0].parse().unwrap();
            let wave: u8 = f[2].parse().unwrap();
            let a4: u32 = f[4].parse().unwrap();
            let a8 = u32::from_str_radix(f[5].trim_start_matches("0x"), 16).unwrap();
            match partial_words(bank, wave) {
                Some(w) => {
                    assert_eq!(w, (a4, a8), "{line}");
                    n += 1;
                }
                None => assert_eq!((f[3], a4, a8), ("2", 0, 0x80000), "{line}"),
            }
        }
        assert_eq!(n, 5 * (25 + 61));
        assert!(partial_words(1, 25).is_none() && partial_words(2, 61).is_none());
    }
}
