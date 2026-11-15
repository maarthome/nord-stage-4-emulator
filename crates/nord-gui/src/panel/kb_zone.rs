//! KB ZONE position model shared by the Organ and Piano strips (manual p.18, p.23, p.39).
//!
//! A layer's zone assignment is a set of the four green zone LEDs. The panel steps through five
//! positions with Shift + OCTAVE: `0` all four zones (the engine default, so an untouched panel
//! sends nothing), then the single zones `1..=4`. ASSUMED: the manual describes the zone LEDs
//! (p.39) but not the stepping order of the buttons; stepping over single zones is the smallest
//! model that reaches every split region. A hardware check would confirm it.

/// Number of positions: all zones, then zone 1..4.
pub const POSITIONS: u8 = 5;

/// Zone LED set (bit `u` = zone `u + 1`) of a position; out-of-range positions mean all zones.
pub const fn leds(pos: u8) -> u8 {
    match pos {
        1 => 0b0001,
        2 => 0b0010,
        3 => 0b0100,
        4 => 0b1000,
        _ => 0b1111,
    }
}

/// Position after a Shift + OCTAVE press (`up` = "+"), clamped to `0..POSITIONS`.
pub fn step(pos: u8, up: bool) -> u8 {
    if up {
        (pos + 1).min(POSITIONS - 1)
    } else {
        pos.saturating_sub(1)
    }
}

/// Position whose LED set is `mask`; a set no position produces (a program can store any of the
/// 12 firmware codes) counts as position 0, so the next press starts from "all zones".
pub fn position_of(mask: u8) -> u8 {
    (0..POSITIONS)
        .find(|&p| leds(p) == mask & 0b1111)
        .unwrap_or(0)
}

/// The four LED states of an LED set (bit `u` = zone `u + 1`).
pub fn lits_of(mask: u8) -> [bool; 4] {
    std::array::from_fn(|i| mask & (1 << i) != 0)
}

/// What the zone LEDs show: with the keyboard split off there are no zones, so every LED is dark.
///
/// VERIFIED against the demo video: the KB ZONE LEDs of the Organ and Piano are dark on every
/// program without a split (A:13, A:38, A:77) and light the focused layer's zones on those with one
/// (A:14: zones 3-4, A:34: zones 1-2).
pub fn shown(lits: [bool; 4], split_on: bool) -> [bool; 4] {
    if split_on {
        lits
    } else {
        [false; 4]
    }
}

/// The four LED states of a position.
pub fn lits(pos: u8) -> [bool; 4] {
    let l = leds(pos);
    std::array::from_fn(|i| l & (1 << i) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_zero_is_every_zone_like_the_engine_default() {
        assert_eq!(leds(0), 0b1111);
        assert_eq!(lits(0), [true; 4]);
        assert_eq!(leds(200), 0b1111);
    }

    #[test]
    fn single_zones_light_one_led_each() {
        for pos in 1..POSITIONS {
            assert_eq!(leds(pos).count_ones(), 1);
            assert_eq!(lits(pos).iter().filter(|b| **b).count(), 1);
        }
        assert_eq!(lits(3), [false, false, true, false]);
    }

    #[test]
    fn led_sets_map_back_to_positions_and_light_any_zone_pair() {
        for p in 0..POSITIONS {
            assert_eq!(position_of(leds(p)), p);
        }
        assert_eq!(
            position_of(0b1100),
            0,
            "a stored pair is no button position"
        );
        assert_eq!(lits_of(0b1100), [false, false, true, true]);
        assert_eq!(lits_of(0b1111), lits(0));
    }

    #[test]
    fn zone_leds_are_dark_without_a_split() {
        assert_eq!(shown(lits(0), false), [false; 4]);
        assert_eq!(shown(lits_of(0b1100), true), [false, false, true, true]);
    }

    #[test]
    fn stepping_clamps_at_both_ends() {
        assert_eq!(step(0, false), 0);
        assert_eq!(step(0, true), 1);
        assert_eq!(step(POSITIONS - 1, true), POSITIONS - 1);
        assert_eq!(step(3, false), 2);
    }
}
