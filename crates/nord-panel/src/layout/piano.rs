//! Piano section (photo crop x 709..841).
//!
//! VERIFIED: every control centre listed in `data/photo_anchors_piano.csv` (crop
//! `NS4-Panel-piano.max-2000x2000.format-png.png`, x/y mapped affinely from the crop's section
//! outline onto [`super::section_rect`]); checked by `python tools/layout_overlay.py piano`,
//! max centre error < 0.1 mm before the GUI label nudges (the tables here are generated from the
//! crop centres, so the check proves the transcription, not the crop read: that read is about
//! +-4 crop px = 0.5 mm). Faders and level LED ladders take their heights from the crop; the other
//! sizes are the old photo reads. ASSUMED: the crop's section is about 1.23x taller in mm than the
//! real one would be at the horizontal scale (see the spec). Shared ids (`piano.solo`,
//! `piano.sustped`, ...) use the rect of the control they hang off.

use super::{px, Rect};

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        "piano.on" => px(795.5, 85.4, 817.5, 98.4),
        "piano.fx_focus_led" => px(764.3, 92.1, 769.3, 97.1),
        "piano.fader.a" => px(712.5, 103.5, 726.5, 194.5),
        "piano.fader.b" => px(744.4, 103.5, 758.4, 194.5),
        "piano.ladder.a" => px(730.3, 107.0, 739.3, 187.0),
        "piano.ladder.b" => px(762.5, 107.0, 771.5, 187.0),
        "piano.focus_led.a" => px(715.0, 209.0, 721.0, 215.0),
        "piano.focus_led.b" => px(746.8, 209.0, 752.8, 215.0),
        "piano.aux_led.a" => px(734.5, 199.4, 740.5, 205.4),
        "piano.aux_led.b" => px(766.7, 199.4, 772.7, 205.4),
        "piano.layer.a" => px(714.7, 220.0, 737.7, 233.0),
        "piano.layer.b" => px(746.6, 220.0, 769.6, 233.0),
        "piano.octave_down" => px(717.2, 306.6, 740.2, 318.6),
        "piano.octave_up" => px(742.4, 306.6, 764.4, 318.6),
        "piano.zone_leds" => px(730.0, 329.6, 753.0, 335.6),
        "piano.type" => px(797.0, 257.2, 820.0, 270.2),
        "piano.type_leds" => px(779.5, 225.7, 837.5, 252.7),
        "piano.select_dial" => px(799.3, 301.1, 817.3, 319.1),
        "piano.soft_release" => px(780.7, 137.6, 803.7, 150.6),
        "piano.kb_touch" => px(780.7, 195.9, 803.7, 208.9),
        "piano.dyn_comp" => px(813.1, 195.9, 836.1, 208.9),
        "piano.unison" => px(813.1, 137.6, 836.1, 150.6),
        "piano.timbre" => px(716.4, 258.1, 729.4, 288.1),
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
