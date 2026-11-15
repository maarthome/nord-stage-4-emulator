//! Master level, wheels and the Rotary Speaker box (photo crop x 67..415).
//!
//! VERIFIED (level knob, Rotary drive / buttons / LEDs): centres in `data/photo_anchors_master.csv`
//! (organ crop, red strip), max error 0.2 mm before nudges, checked by
//! `python tools/layout_overlay.py master`. VERIFIED (centre +-1.5 mm): `master.pitch_stick` and
//! `master.mod_wheel`, from the straight-on HA88 top-down render (`super::topdown`).

use super::topdown::td;
use super::{px, Rect};

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        "master.level" => px(383.0, 105.5, 406.0, 128.5),
        // VERIFIED (centre, +-1.5 mm): slot of the pitch stick on the HA88 top-down render, bbox
        // 269 x 162 px = 32.1 x 19.4 mm (`data/photo_anchors_wheels.csv`, `layout_overlay.py wheels`).
        "master.pitch_stick" => td(1612.0, 940.0, 32.1, 19.4),
        // VERIFIED (centre): wheel slot bbox 160 x 410 px = 19.1 x 49.0 mm on the same render.
        "master.mod_wheel" => td(1910.0, 1230.0, 19.1, 49.0),
        "rotary.drive" => px(382.0, 176.5, 404.0, 198.5),
        "rotary.organ" => px(383.5, 225.5, 405.5, 237.5),
        "rotary.stop_mode" => px(383.5, 269.5, 405.5, 281.5),
        // Widened from 386..406 so the row also holds the SLOW / FAST names (LED then name).
        "rotary.speed_leds" => px(375.0, 298.5, 408.0, 304.5),
        "rotary.speed" => px(383.5, 310.5, 405.5, 322.5),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
