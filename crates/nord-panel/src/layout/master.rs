//! Master level, wheels and the Rotary Speaker box (photo crop x 67..415).
//!
//! ASSUMED: read from the 2000 px photo; higher-resolution photo confirms.

use super::{px, Rect};

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        "master.level" => px(383.0, 105.5, 406.0, 128.5),
        "master.pitch_stick" => px(256.0, 150.0, 300.0, 175.0),
        "master.mod_wheel" => px(314.0, 176.0, 341.0, 248.0),
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
