//! Mouse velocity of the on-screen keybed from where the pointer hits the key (D84).
//!
//! A real key is struck harder near its front tip (the lever arm is shortest, so the same finger
//! speed gives more key speed). The mouse has no force, so the hit position along the key stands in:
//! front tip = loud, back = soft. Only the mouse path uses this; the computer keyboard (D80) and
//! MIDI In keep their own velocity.

/// Softest mouse velocity (pointer at the back of the key).
/// ASSUMED: 20 keeps the softest layer audible; the hardware Felt Upright demo is played at a median
/// of about 43 (2026-11-15 listening test), so the useful soft range must be reachable.
pub const MIN_VELOCITY: u8 = 20;
/// Loudest mouse velocity (pointer at the front tip).
/// ASSUMED: the MIDI maximum.
pub const MAX_VELOCITY: u8 = 127;

/// Black-key height relative to the white key height (shared with the key geometry).
pub const BLACK_H: f32 = 0.62;

/// Velocity for a press at `rel_y` (0 = top edge of the keybed = back of the keys) on a key that is
/// `black` or white, in a keybed `height` tall.
///
/// ASSUMED: linear in the hit position along the key length (black keys are shorter, so their
/// front tip is `BLACK_H * height`); a curve would need a measured mouse-to-force law.
pub fn velocity_at(rel_y: f32, black: bool, height: f32) -> u8 {
    let len = if black { height * BLACK_H } else { height };
    if len <= 0.0 || !rel_y.is_finite() {
        return MIN_VELOCITY;
    }
    let t = (rel_y / len).clamp(0.0, 1.0);
    let span = f32::from(MAX_VELOCITY - MIN_VELOCITY);
    (f32::from(MIN_VELOCITY) + t * span).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_is_soft_and_front_tip_is_loud() {
        assert_eq!(velocity_at(0.0, false, 100.0), MIN_VELOCITY);
        assert_eq!(velocity_at(100.0, false, 100.0), MAX_VELOCITY);
    }

    #[test]
    fn velocity_is_monotonic_in_depth_and_within_bounds() {
        for black in [false, true] {
            let mut prev = 0u8;
            for i in 0..=200 {
                let v = velocity_at(i as f32 * 0.5, black, 100.0);
                assert!(v >= prev, "not monotonic at {i}");
                assert!((MIN_VELOCITY..=MAX_VELOCITY).contains(&v));
                prev = v;
            }
        }
    }

    #[test]
    fn out_of_range_and_bad_input_clamp() {
        assert_eq!(velocity_at(-5.0, false, 100.0), MIN_VELOCITY);
        assert_eq!(velocity_at(500.0, true, 100.0), MAX_VELOCITY);
        assert_eq!(velocity_at(f32::NAN, false, 100.0), MIN_VELOCITY);
        assert_eq!(velocity_at(10.0, false, 0.0), MIN_VELOCITY);
    }

    #[test]
    fn black_key_front_tip_is_loud_where_a_white_key_is_still_soft() {
        let h = 100.0;
        let tip = h * BLACK_H;
        assert_eq!(velocity_at(tip, true, h), MAX_VELOCITY);
        assert!(velocity_at(tip, false, h) < MAX_VELOCITY);
        // Same depth: the shorter black key reads louder.
        assert!(velocity_at(30.0, true, h) > velocity_at(30.0, false, h));
    }

    #[test]
    fn white_key_mid_depth_is_mid_velocity() {
        let v = velocity_at(50.0, false, 100.0);
        assert_eq!(v, 74); // 20 + 0.5 * 107 = 73.5, rounds to 74
    }
}
