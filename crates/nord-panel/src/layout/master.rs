//! Master level, wheels and the Rotary Speaker box (photo crop x 67..415).
//!
//! VERIFIED (level knob, Rotary drive / buttons / LEDs): centres in `data/photo_anchors_master.csv`
//! (organ crop, red strip, isotropic mapping of [`super::crop`]), checked by
//! [`tests::centres_match_the_photo_anchor_csv`] and `python tools/layout_overlay.py master`.
//! VERIFIED (centre +-1.5 mm): `master.pitch_stick` and `master.mod_wheel`, from the straight-on HA88 top-down render (`super::topdown`).

use super::crop::ORGAN as C;
use super::topdown::td;
use super::Rect;

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        "master.level" => C.rect(132.0, 172.0, 15.9, 15.9),
        // VERIFIED (centre, +-1.5 mm): slot of the pitch stick on the HA88 top-down render, bbox
        // 269 x 162 px = 32.1 x 19.4 mm (`data/photo_anchors_wheels.csv`, `layout_overlay.py wheels`).
        "master.pitch_stick" => td(1612.0, 940.0, 32.1, 19.4),
        // VERIFIED (centre): wheel slot bbox 160 x 410 px = 19.1 x 49.0 mm on the same render.
        "master.mod_wheel" => td(1910.0, 1230.0, 19.1, 49.0),
        "rotary.drive" => C.rect(123.0, 487.0, 15.2, 15.2),
        "rotary.organ" => C.rect(132.0, 684.0, 15.2, 8.3),
        "rotary.stop_mode" => C.rect(132.0, 880.0, 15.2, 8.3),
        // Widened from 386..406 so the row also holds the SLOW / FAST names (LED then name).
        "rotary.speed_leds" => C.rect(114.0, 996.0, 22.8, 4.2),
        "rotary.speed" => C.rect(132.0, 1062.0, 15.2, 8.3),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centres_match_the_photo_anchor_csv() {
        let csv = include_str!("../../data/photo_anchors_master.csv");
        let n = super::super::crop::check_csv(csv, super::C, 0.1);
        assert_eq!(n, 6);
    }

    #[test]
    fn known_and_unknown_ids() {
        assert!(rect("master.level").is_some());
        assert!(rect("organ.on").is_none());
    }

    #[test]
    fn wheel_is_taller_than_wide() {
        let w = rect("master.mod_wheel").unwrap();
        assert!(w.h > w.w);
    }
}
