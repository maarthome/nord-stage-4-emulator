//! Piano section, placed from the piano crop with the isotropic mapping of [`super::crop`]
//! (crop `NS4-Panel-piano.max-2000x2000.format-png.png`).
//!
//! VERIFIED: every control centre listed in `data/photo_anchors_piano.csv` (crop px, read by eye,
//! about +-4 crop px = 0.5 mm) is the centre of the rectangle here, checked by
//! [`tests::centres_match_the_photo_anchor_csv`] and `python tools/layout_overlay.py piano`;
//! the absolute position follows the HA88 render, see [`super::crop`]. The old affine placement
//! put these up to 18 mm too low. Faders and level LED ladders take their heights from the crop;
//! the other sizes are the old photo reads (ASSUMED). Shared ids (`piano.solo`, `piano.sustped`,
//! ...) use the rect of the control they hang off.

use super::crop::PIANO as C;
use super::Rect;

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        "piano.on" => C.rect(555.0, 65.0, 15.2, 9.0),
        "piano.fx_focus_led" => C.rect(335.0, 77.0, 3.5, 3.5),
        "piano.fader.a" => C.rect_px(73.0, 321.0, 82.0, 408.0),
        "piano.fader.b" => C.rect_px(250.0, 321.0, 82.0, 408.0),
        "piano.ladder.a" => C.rect_px(158.0, 312.0, 52.0, 360.0),
        "piano.ladder.b" => C.rect_px(336.0, 312.0, 52.0, 360.0),
        "piano.focus_led.a" => C.rect(65.0, 603.0, 4.2, 4.2),
        "piano.focus_led.b" => C.rect(241.0, 603.0, 4.2, 4.2),
        "piano.aux_led.a" => C.rect(173.0, 560.0, 4.2, 4.2),
        "piano.aux_led.b" => C.rect(351.0, 560.0, 4.2, 4.2),
        "piano.layer.a" => C.rect(110.0, 668.0, 15.9, 9.0),
        "piano.layer.b" => C.rect(287.0, 668.0, 15.9, 9.0),
        "piano.octave_down" => C.rect(124.0, 1054.0, 15.9, 8.3),
        "piano.octave_up" => C.rect(261.0, 1054.0, 15.2, 8.3),
        "piano.zone_leds" => C.rect(195.0, 1144.0, 15.9, 4.1),
        "piano.type" => C.rect(566.0, 835.0, 15.9, 9.0),
        "piano.type_leds" => C.rect(566.0, 725.0, 40.1, 14.0),
        "piano.select_dial" => C.rect(565.0, 1043.0, 12.5, 12.5),
        "piano.soft_release" => C.rect(476.0, 299.0, 15.9, 9.0),
        "piano.kb_touch" => C.rect(476.0, 560.0, 15.9, 9.0),
        "piano.dyn_comp" => C.rect(655.0, 560.0, 15.9, 9.0),
        "piano.unison" => C.rect(655.0, 299.0, 15.9, 9.0),
        "piano.timbre" => C.rect(92.0, 877.0, 9.0, 20.8),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_is_square() {
        let d = rect("piano.select_dial").unwrap();
        assert!((d.w - d.h).abs() < 1e-3);
    }

    #[test]
    fn centres_match_the_photo_anchor_csv() {
        let csv = include_str!("../../data/photo_anchors_piano.csv");
        let n = super::super::crop::check_csv(csv, super::C, 0.1);
        assert!(n >= 30, "{n} anchors checked");
    }

    #[test]
    fn photo_anchor_csv_covers_every_piano_control() {
        let csv = include_str!("../../data/photo_anchors_piano.csv");
        let ids: Vec<&str> = csv
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap())
            .collect();
        for c in crate::all_controls()
            .iter()
            .filter(|c| c.id.starts_with("piano."))
        {
            assert!(ids.contains(&c.id.as_str()), "{} has no photo anchor", c.id);
        }
        for id in &ids {
            assert!(crate::layout::rect_for(id).is_some(), "{id} has no rect");
        }
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(rect("organ.on").is_none());
    }
}
