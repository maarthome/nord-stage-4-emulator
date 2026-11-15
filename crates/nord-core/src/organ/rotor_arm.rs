//! The ARM's Leslie rotor speeds and ramps, shared by the engine rotary and the firmware processor.
//!
//! Source: the rotor state block of the organ part (`0x9115D2B0` in the settled harness) and its
//! update `0xC02090F4`, [`docs/research/fx-leslie.md`](../../../../docs/research/fx-leslie.md) s11.
//!
//! * VERIFIED (state block, `tools/arm_leslie_speeds.py`): horn slow 0.80 Hz, fast 7.0 Hz, rotor
//!   slow 0.73 Hz, fast 6.4 Hz, in the ARM's own unit (it assumes a 44.1 kHz DSP frame rate).
//! * VERIFIED (run of the update, 700 calls from the settled snapshot): the speed moves linearly,
//!   per update `+0.06` (horn) / `+0.01` (rotor) Hz up and `-0.06` / `-0.0218` Hz down, so slow to
//!   fast takes 104 updates for the horn and 567 for the rotor.
//! * VERIFIED (code, `0xC01185C0..0xC0118650`): the update is a periodic timer entry with the
//!   period word `0xF0` = 240 ticks, due `now + 240` again after every call; the tick counter
//!   `0x80001DC0` runs at 24,000 per second (the arp clock law of `synth-arp-patterns.md` s8.2,
//!   the `0xC005019C(24000)` timer setup divides the 24 MHz count clock by it), so one update is
//!   **10 ms** and the ramps are 6.0 and 1.0 Hz/s up (horn 1.04 s, rotor 5.67 s slow to fast).
//! * The DSP words are `int(hz * 8388607 / 11025) << 8` at a call rate of `fs / 2`, so the speed
//!   the rotors really turn at is the ARM's value times `F / 44,100` (`F` = 43,402.34 Hz).

use super::lane_mix::FIRMWARE_FRAME_RATE_HZ;

/// ARM timer ticks per second (VERIFIED, see the module doc).
pub const TICK_HZ: f64 = 24_000.0;
/// Ticks between two rotor updates (VERIFIED, constructor `0xC01185C0..0xC0118650`).
pub const UPDATE_TICKS: f64 = 240.0;
/// Seconds between two rotor updates.
pub const UPDATE_S: f32 = (UPDATE_TICKS / TICK_HZ) as f32;
/// The frame rate the ARM assumes when it converts Hz into the DSP phase words.
pub const ARM_NOMINAL_FS: f64 = 44_100.0;

/// One rotor section of the ARM state block (speeds in Hz at the ARM's nominal frame rate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// Slow speed (Chorale).
    pub slow: f64,
    /// Fast speed (Tremolo).
    pub fast: f64,
    /// Speed change per update while accelerating.
    pub up: f64,
    /// Speed change per update while braking (magnitude).
    pub down: f64,
}

/// The treble horn section (state block `+0x08..+0x50`).
pub const HORN: Section = Section {
    slow: 0.80,
    fast: 7.0,
    up: 0.06,
    down: 0.06,
};
/// The bass rotor section (state block `+0x70..+0xB8`).
pub const ROTOR: Section = Section {
    slow: 0.73,
    fast: 6.4,
    up: 0.01,
    down: 0.0218,
};

/// Ratio between the physical rotor speed and the ARM's nominal Hz (`F / 44,100`).
pub fn physical_ratio() -> f32 {
    (FIRMWARE_FRAME_RATE_HZ / ARM_NOMINAL_FS) as f32
}

impl Section {
    /// Slow speed in physical Hz (the DSP words at the real frame rate).
    pub fn slow_hz(self) -> f32 {
        self.slow as f32 * physical_ratio()
    }

    /// Fast speed in physical Hz.
    pub fn fast_hz(self) -> f32 {
        self.fast as f32 * physical_ratio()
    }

    /// One update of the ARM ramp in physical Hz: `now` moves by the up or down step toward
    /// `target` and never passes it (`step_scale` stretches the steps, 1.0 = the ARM's).
    pub fn step(self, now: f32, target: f32, step_scale: f32) -> f32 {
        let r = physical_ratio() * step_scale;
        if now < target {
            (now + self.up as f32 * r).min(target)
        } else {
            (now - self.down as f32 * r).max(target)
        }
    }
}

/// Fixed 10 ms update clock: calls `f` once for every ARM update that falls into `dt` seconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateClock {
    acc: f32,
}

impl UpdateClock {
    /// Adds `dt` seconds; returns the number of updates due (0 or more).
    #[inline]
    pub fn advance(&mut self, dt: f32) -> u32 {
        self.acc += dt;
        let mut n = 0;
        while self.acc >= UPDATE_S {
            self.acc -= UPDATE_S;
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn updates_to(sec: Section, from: f32, to: f32) -> u32 {
        let (mut now, mut n) = (from, 0);
        while (now - to).abs() > 1e-6 && n < 100_000 {
            now = sec.step(now, to, 1.0);
            n += 1;
        }
        n
    }

    #[test]
    fn the_update_is_10_ms_from_240_ticks_at_24_khz() {
        assert!((UPDATE_S - 0.01).abs() < 1e-9);
        assert!((TICK_HZ * f64::from(UPDATE_S) - 240.0).abs() < 1e-3);
    }

    #[test]
    fn horn_takes_104_updates_and_rotor_567_from_slow_to_fast() {
        // The harness run of `0xC02090F4` (700 calls) reached the fast value at call index 103
        // (horn) and 566 (rotor), i.e. after 104 and 567 updates.
        let (hs, hf) = (HORN.slow_hz(), HORN.fast_hz());
        let (rs, rf) = (ROTOR.slow_hz(), ROTOR.fast_hz());
        assert_eq!(updates_to(HORN, hs, hf), 104);
        assert_eq!(updates_to(ROTOR, rs, rf), 567);
        let secs = |n: u32| n as f32 * UPDATE_S;
        assert!((secs(104) - 1.04).abs() < 1e-6 && (secs(567) - 5.67).abs() < 1e-5);
    }

    #[test]
    fn braking_follows_the_down_steps() {
        // fast to slow in the harness: horn 0.06 per update, rotor 0.0218 per update
        assert_eq!(updates_to(HORN, HORN.fast_hz(), HORN.slow_hz()), 104);
        assert_eq!(updates_to(ROTOR, ROTOR.fast_hz(), ROTOR.slow_hz()), 261);
        // stop: the horn reaches 0 from slow within 14 updates, the rotor from slow in 34
        assert_eq!(updates_to(HORN, HORN.slow_hz(), 0.0), 14);
        assert_eq!(updates_to(ROTOR, ROTOR.slow_hz(), 0.0), 34);
    }

    #[test]
    fn speeds_are_physical_hz_at_the_dsp_frame_rate() {
        assert!((HORN.slow_hz() - 0.7873).abs() < 5e-4);
        assert!((HORN.fast_hz() - 6.8892).abs() < 5e-4);
        assert!((ROTOR.slow_hz() - 0.7185).abs() < 5e-4);
        assert!((ROTOR.fast_hz() - 6.2987).abs() < 5e-4);
    }

    #[test]
    fn the_clock_counts_whole_updates() {
        let mut c = UpdateClock::default();
        let n: u32 = (0..48_000).map(|_| c.advance(1.0 / 48_000.0)).sum();
        assert!((99..=100).contains(&n), "{n}");
    }
}
