//! Button gesture recognition: tap, hold and double-tap (manual p.18, p.43-44).
//!
//! Timings: the hold threshold is read from the firmware key table
//! ([`super::firmware`], 500 ms for most keys, 1000 ms for the Morph Assign buttons);
//! `DOUBLE_TAP_MS` stays `ASSUMED` (manual p.39, p.43 describe double-tapping Morph Assign and
//! Section Edit but give no interval, and the firmware's own double-press word has an unresolved
//! unit; OQ13).

use super::firmware;

/// Default hold threshold in milliseconds.
/// VERIFIED: 500 ms is the firmware hold of 35 of the 49 keys in its key table
/// (`bank0_CODE.bin@0xC024C5E8..0xC024C9A8`, `tools/arm_key_timing.py table`, test
/// `firmware::tests::hold_values_are_the_firmware_set`), and manual p.18 says "roughly half a
/// second". The Morph Assign buttons use [`HOLD_MS_MORPH`]; see `firmware::control_hold_ms`.
pub const HOLD_MS: u32 = 500;
/// Hold threshold of the Morph Assign buttons. VERIFIED: key ids 48, 49 and 81 hold 1000 ms in the
/// firmware table (`firmware::MORPH_KEYS`, same source and check as [`HOLD_MS`]).
pub const HOLD_MS_MORPH: u32 = 1000;
/// Maximum gap between a release and the next press that counts as a double tap.
/// ASSUMED: 300 ms is a common UI default. The firmware key object stores a double-press word 300
/// (`bank0_CODE.bin@0xC0083410`) and scales it by 24 (`0xC0083314`) before comparing it with the
/// 64-bit tick (`0xC007B204`); the tick unit of that compare was not resolved (1 kHz ISR would make
/// it 7.2 s), so the number is not confirmed. What would confirm it: the consumer of the firmware's
/// double-press event (kind 4), or a hardware clip.
pub const DOUBLE_TAP_MS: u32 = 300;

/// A recognised gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// Press and release before the hold threshold.
    Tap,
    /// Held down for at least [`HOLD_MS`]; fired once while still pressed.
    Hold,
    /// A second press within [`DOUBLE_TAP_MS`] of the previous tap's release.
    DoubleTap,
}

/// Per-button state machine. Times are monotonic milliseconds from the caller.
#[derive(Debug, Clone)]
pub struct ButtonTracker {
    hold_ms: u32,
    pressed_at: Option<u32>,
    last_tap_release: Option<u32>,
    hold_fired: bool,
    swallow_release: bool,
}

impl Default for ButtonTracker {
    fn default() -> Self {
        Self::with_hold(HOLD_MS)
    }
}

impl ButtonTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tracker with its own hold threshold in milliseconds.
    pub fn with_hold(hold_ms: u32) -> Self {
        Self {
            hold_ms,
            pressed_at: None,
            last_tap_release: None,
            hold_fired: false,
            swallow_release: false,
        }
    }

    /// Tracker for the panel control `id`: the firmware hold of that control when it is mapped
    /// ([`firmware::control_hold_ms`]), else [`HOLD_MS`].
    pub fn for_control(id: &str) -> Self {
        Self::with_hold(firmware::control_hold_ms(id).unwrap_or(HOLD_MS))
    }

    /// The hold threshold of this tracker in milliseconds.
    pub fn hold_ms(&self) -> u32 {
        self.hold_ms
    }

    /// Button went down. Returns `DoubleTap` when it follows a recent tap.
    pub fn press(&mut self, t: u32) -> Option<Gesture> {
        self.pressed_at = Some(t);
        self.hold_fired = false;
        let double = self
            .last_tap_release
            .is_some_and(|r| t.saturating_sub(r) <= DOUBLE_TAP_MS);
        self.last_tap_release = None;
        if double {
            self.swallow_release = true;
            Some(Gesture::DoubleTap)
        } else {
            self.swallow_release = false;
            None
        }
    }

    /// Advance time while the button is held. Returns `Hold` once.
    pub fn tick(&mut self, t: u32) -> Option<Gesture> {
        let down = self.pressed_at?;
        if !self.hold_fired && !self.swallow_release && t.saturating_sub(down) >= self.hold_ms {
            self.hold_fired = true;
            return Some(Gesture::Hold);
        }
        None
    }

    /// Button went up. Returns `Tap` for a short press that was not a double tap.
    pub fn release(&mut self, t: u32) -> Option<Gesture> {
        let down = self.pressed_at.take()?;
        if self.swallow_release || self.hold_fired {
            self.swallow_release = false;
            return None;
        }
        if t.saturating_sub(down) >= self.hold_ms {
            // Released after the threshold without a tick: still a hold.
            return Some(Gesture::Hold);
        }
        self.last_tap_release = Some(t);
        Some(Gesture::Tap)
    }

    /// Whether the button is currently down.
    pub fn is_down(&self) -> bool {
        self.pressed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_press_is_a_tap() {
        let mut b = ButtonTracker::new();
        assert_eq!(b.press(0), None);
        assert!(b.is_down());
        assert_eq!(b.release(100), Some(Gesture::Tap));
        assert!(!b.is_down());
    }

    #[test]
    fn long_press_fires_hold_once_and_no_tap() {
        let mut b = ButtonTracker::new();
        b.press(0);
        assert_eq!(b.tick(499), None);
        assert_eq!(b.tick(500), Some(Gesture::Hold));
        assert_eq!(b.tick(900), None, "hold fires once");
        assert_eq!(b.release(1000), None, "no tap after a hold");
    }

    #[test]
    fn release_after_threshold_without_tick_is_still_a_hold() {
        let mut b = ButtonTracker::new();
        b.press(0);
        assert_eq!(b.release(700), Some(Gesture::Hold));
    }

    #[test]
    fn two_quick_presses_make_a_double_tap() {
        let mut b = ButtonTracker::new();
        b.press(0);
        assert_eq!(b.release(80), Some(Gesture::Tap));
        assert_eq!(b.press(200), Some(Gesture::DoubleTap));
        assert_eq!(b.release(260), None, "double tap swallows its release");
    }

    #[test]
    fn slow_second_press_is_a_new_tap() {
        let mut b = ButtonTracker::new();
        b.press(0);
        b.release(80);
        assert_eq!(b.press(80 + DOUBLE_TAP_MS + 1), None);
        assert_eq!(b.release(500), Some(Gesture::Tap));
    }

    #[test]
    fn third_press_after_a_double_tap_starts_fresh() {
        let mut b = ButtonTracker::new();
        b.press(0);
        b.release(50);
        b.press(100);
        b.release(150);
        assert_eq!(b.press(200), None);
    }

    #[test]
    fn morph_buttons_need_a_one_second_hold() {
        let mut b = ButtonTracker::for_control("morph.wheel");
        assert_eq!(b.hold_ms(), HOLD_MS_MORPH);
        b.press(0);
        assert_eq!(b.tick(HOLD_MS), None, "half a second is still a press");
        assert_eq!(b.tick(HOLD_MS_MORPH - 1), None);
        assert_eq!(b.tick(HOLD_MS_MORPH), Some(Gesture::Hold));
    }

    #[test]
    fn morph_release_between_half_and_one_second_is_a_tap() {
        let mut b = ButtonTracker::for_control("morph.at");
        b.press(0);
        assert_eq!(b.release(900), Some(Gesture::Tap));
    }

    #[test]
    fn unmapped_controls_keep_the_default_hold() {
        assert_eq!(
            ButtonTracker::for_control("organ.preset").hold_ms(),
            HOLD_MS
        );
        assert_eq!(ButtonTracker::new().hold_ms(), HOLD_MS);
        assert_eq!(ButtonTracker::with_hold(350).hold_ms(), 350);
    }

    #[test]
    fn default_hold_is_the_modal_firmware_hold() {
        // The classifier default must be the most common firmware hold of the key table.
        let n = |h: u16| firmware::KEY_TIMINGS.iter().filter(|r| r.1 == h).count();
        let default = u16::try_from(HOLD_MS).unwrap();
        assert!(n(default) > n(350) && n(default) > n(1000));
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut b = ButtonTracker::new();
        assert_eq!(b.release(10), None);
        assert_eq!(b.tick(10), None);
    }
}
