//! `FirmwareFx::delay_time` wiring: the first echo lands at the verified sample, the engine time
//! stays the default, a missing raw value falls back to the engine, tempo sync follows the
//! firmware division table.

use super::*;

fn chain(fw: FirmwareFx) -> LayerFxChain {
    let mut fx = LayerFxChain::new(48_000.0);
    fx.reverb_model = ReverbModel::Off;
    fx.firmware_fx = fw;
    fx
}

/// Frame index of the loudest left output after a unit impulse (wet only, no feedback).
fn echo_index(fx: &mut LayerFxChain, frames: usize) -> usize {
    fx.delay_enabled = true;
    fx.delay_feedback = 0.0;
    fx.delay_mix = 1.0;
    fx.snap_delay_time(120.0);
    let out: Vec<f32> = (0..frames)
        .map(|i| fx.process_frame([f32::from(i == 0), 0.0])[0])
        .collect();
    (1..frames)
        .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
        .unwrap()
}

fn delay_time_only() -> FirmwareFx {
    FirmwareFx {
        delay_time: true,
        ..FirmwareFx::OFF
    }
}

#[test]
fn first_echo_is_c15_plus_two_samples() {
    // kDlyRateClkDiv 16383: c[0x15] = 868, echo 870; 1024: c[0x15] = 46498, echo 46500 frames of
    // the 43,402.34 Hz DSP rate, scaled to the 48 kHz engine rate (x 1.105920).
    for (raw, want) in [(16_383_u16, 962_usize), (1024, 51_426)] {
        let mut fx = chain(delay_time_only());
        fx.delay_rate_raw = Some(raw);
        assert_eq!(echo_index(&mut fx, want + 200), want, "raw {raw}");
    }
}

#[test]
fn switch_off_keeps_the_engine_time() {
    let mut fx = chain(FirmwareFx::OFF);
    fx.delay_rate_raw = Some(16_383);
    fx.delay_time_ms = 375.0;
    assert_eq!(echo_index(&mut fx, 20_000), 18_000);
}

#[test]
fn missing_raw_value_falls_back_and_tempo_sync_uses_the_firmware_division() {
    let mut a = chain(delay_time_only());
    a.delay_time_ms = 100.0;
    assert_eq!(echo_index(&mut a, 6000), 4800, "no raw value");
    let mut b = chain(FirmwareFx::ALL);
    // raw 1024 = division 1 (0.375 of a whole note = 1.5 beats = 750 ms at 120 bpm)
    b.delay_rate_raw = Some(1024);
    b.delay_clock_sync = true;
    let echo = echo_index(&mut b, 40_000);
    assert!(echo.abs_diff(36_001) <= 8, "sync echo at {echo}");
}

#[test]
fn raw_value_is_clamped_to_the_14_bit_range() {
    let mut fx = chain(delay_time_only());
    fx.delay_rate_raw = Some(u16::MAX);
    assert_eq!(echo_index(&mut fx, 1200), 962);
}
