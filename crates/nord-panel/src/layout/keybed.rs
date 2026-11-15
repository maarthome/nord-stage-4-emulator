//! Keybed and the row of split-point LEDs above it.
//!
//! VERIFIED (HA88 top-down render, see [`super::topdown`]): 52 white + 36 black keys counted;
//! white keys span x 667..10938 px = 1227.4 mm (pitch 23.63 mm) and y 2005..3235 px = 182.0..329.1
//! mm (`python tools/layout_overlay.py keybed`, `data/photo_anchors_keybed.csv`).
//! The split LEDs are **11** small LEDs on the red rim below the sections (y 1864 px = 165 mm,
//! above the keys, not on them), at the white keys B1 E2 B2 E3 B3 E4 B4 E5 B5 E6 B6 (x 2422, 3017,
//! 3813, 4408, 5201, ~5790, 6595, 7189, 7985, 8582, 9378 px). One rect covers the row.
//! ASSUMED: the Compact (73) has the same row on its own keys (no straight-on render of it).

use super::topdown::{td, td_box, MM_PER_TD_PX};
use super::Rect;

/// White-key indices (A0 = 0) under the 11 split LEDs, left to right.
pub const SPLIT_LED_WHITE_KEYS: [u8; 11] = [8, 11, 15, 18, 22, 25, 29, 32, 36, 39, 43];

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        // 11 LEDs from x 2422 to 9378 px, 2.6 mm tall (LED diameter 22 px), centred on y 1864 px.
        "keybed.split_leds" => td((2422.0 + 9378.0) / 2.0, 1864.0, 835.0, 2.6),
        "keybed.keys" => td_box(667.0, 2005.0, 10271.0 * MM_PER_TD_PX, 1230.0 * MM_PER_TD_PX),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_wide_and_below_the_led_strip() {
        let k = rect("keybed.keys").unwrap();
        let l = rect("keybed.split_leds").unwrap();
        assert!(k.w > 1000.0);
        assert!(l.bottom() <= k.y + 1e-3);
        assert!(rect("master.level").is_none());
    }

    /// 52 white keys of 23.6 mm: the keys rectangle is 1227 mm wide.
    #[test]
    fn keys_are_52_white_keys_of_23_6_mm() {
        let k = rect("keybed.keys").unwrap();
        assert!((k.w / 52.0 - 23.6).abs() < 0.1);
        assert!((k.x + k.w / 2.0 - 645.0).abs() < 3.0, "centred on the body");
    }

    /// The LED row is above the keys, in the red control area (y < 179.1 mm).
    #[test]
    fn split_led_row_is_in_the_control_area() {
        let l = rect("keybed.split_leds").unwrap();
        assert!(l.bottom() < super::super::PANEL_HEIGHT_MM);
        assert_eq!(SPLIT_LED_WHITE_KEYS.len(), 11);
        assert!(SPLIT_LED_WHITE_KEYS.windows(2).all(|w| w[1] - w[0] >= 3));
    }
}
