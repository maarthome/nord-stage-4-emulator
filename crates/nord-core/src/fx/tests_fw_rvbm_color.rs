//! The colour switch of the multi-tap reverb (`kRvbColor`): the delta words the ARM sends
//! (`fw_rvbm_colors.rs`, `docs/research/data/fx-reverb-colors.csv`), the switch in place and the
//! engine mapping.

use super::fw_rvbm::FwReverbMultiF32;
use super::fw_rvbm_colors::COLOR_WORDS;
use super::fw_rvbm_words::TYPES;
use super::{LayerFxChain, ReverbColor};

#[test]
fn every_type_has_a_non_empty_dark_and_bright_delta_with_the_output_filter() {
    for (t, colors) in TYPES.iter().zip(COLOR_WORDS) {
        for (c, d) in colors.iter().enumerate() {
            assert!(d.len() >= 9, "type {t} colour {}: {} words", c + 1, d.len());
            assert!(
                d.iter().any(|&(a, _)| a == 0xE0C46),
                "type {t}: the filter gain word"
            );
        }
    }
}

#[test]
fn a_colour_switch_writes_exactly_its_delta_and_colour_zero_restores_the_type() {
    for (k, &t) in TYPES.iter().enumerate() {
        let mut r = FwReverbMultiF32::new(t).unwrap();
        let normal = r.word(0xE0C46);
        for c in 1..=2 {
            assert!(r.set_color(c) && r.color() == c);
            for &(a, w) in COLOR_WORDS[k][c - 1] {
                assert_eq!(r.word(a), w, "type {t} colour {c} word {a:#x}");
            }
            assert_ne!(
                r.word(0xE0C46),
                normal,
                "type {t} colour {c} changes the filter"
            );
        }
        assert!(r.set_color(0));
        assert_eq!(r.word(0xE0C46), normal, "type {t} back to Normal");
        assert!(!r.set_color(3) && r.color() == 0);
    }
}

#[test]
fn colour_zero_is_the_unchanged_type_and_a_retype_keeps_the_colour() {
    let mut a = FwReverbMultiF32::new(4).unwrap();
    a.set_color(2);
    assert!(a.set_type(9) && a.color() == 2);
    let mut b = FwReverbMultiF32::new(9).unwrap();
    b.set_color(2);
    let mut x = [[0.3, -0.1]; 4];
    let mut y = x;
    for _ in 0..200 {
        a.process_block4(&mut x);
        b.process_block4(&mut y);
        assert_eq!(x, y);
    }
}

#[test]
fn the_colour_words_fingerprint() {
    // EMPIRICAL (self-consistency, regenerate with `tools/fx_rvb_colors.py`): a hash of the
    // delta tables so a silent edit of the generated file fails here.
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for t in COLOR_WORDS {
        for d in t {
            for &(a, w) in d {
                for b in a.to_le_bytes().into_iter().chain(w.to_le_bytes()) {
                    h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3);
                }
            }
        }
    }
    assert_eq!(h, FINGERPRINT);
}

const FINGERPRINT: u64 = 8_801_152_105_870_756_208;

#[test]
fn the_engine_colours_map_onto_the_name_list() {
    assert_eq!(LayerFxChain::fw_multi_color(ReverbColor::Normal), 0);
    assert_eq!(LayerFxChain::fw_multi_color(ReverbColor::Dark), 1);
    assert_eq!(LayerFxChain::fw_multi_color(ReverbColor::Bright), 2);
    assert_eq!(LayerFxChain::fw_multi_color(ReverbColor::Chorale), 0);
}

/// DSP frame rate of the FX unit: 43,402.34 Hz (`docs/research/fx-delay-tempo.md`).
const FS_DSP: f64 = 43_402.34;

/// T60 (s) from the Schroeder backward integral of the left wet impulse response, fitted between
/// -5 and -25 dB (a T20 extrapolated to 60 dB).
fn t60(r: &mut FwReverbMultiF32, calls: usize) -> f64 {
    let mut e: Vec<f64> = Vec::with_capacity(calls * 4);
    for k in 0..calls {
        let mut f = [[0.0f32; 2]; 4];
        if k == 0 {
            f[0] = [1.0, 1.0];
        }
        r.process_block4(&mut f);
        e.extend(f.iter().map(|x| f64::from(x[0]).powi(2)));
    }
    let mut acc = 0.0;
    let mut edc = vec![0.0f64; e.len()];
    for (i, v) in e.iter().enumerate().rev() {
        acc += v;
        edc[i] = acc;
    }
    let total = edc[0];
    let at = |level: f64| {
        edc.iter()
            .position(|&v| 10.0 * (v / total).log10() <= level)
    };
    let (a, b) = (at(-5.0).unwrap(), at(-25.0).unwrap());
    3.0 * (b - a) as f64 / FS_DSP
}

/// MEASURED (this test, `FwReverbMultiF32`, impulse at amount 127): T60 of the routine per
/// `(type, colour)`; the type pairs and colours change the tail by up to 36 % (Stage Chorale
/// 0.995 s Normal, 1.35 s Dark / Bright). EMPIRICAL: the routine is verified word for word, the
/// hardware tail of these types is not measured.
#[test]
fn the_firmware_tail_per_type_and_colour_stays_within_three_percent_of_the_measured_values() {
    for (t, c, want) in [
        (2, 0, 0.615),
        (2, 2, 0.522),
        (4, 1, 0.763),
        (7, 0, 0.995),
        (7, 1, 1.349),
        (11, 2, 1.306),
    ] {
        let mut r = FwReverbMultiF32::new(t).unwrap();
        r.set_color(c);
        let got = t60(&mut r, 20_000);
        assert!(
            (got - want).abs() < 0.03 * want,
            "type {t} colour {c}: {got:.3} s against {want} s"
        );
    }
}
