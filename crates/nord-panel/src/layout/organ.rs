//! Organ section (photo crop x 415..709). Drawbar `n` sits at
//! `x = 485 + 25.4 * (n - 1)` photo px; its LED ladder is the 15 px column
//! just to the right of the slot.
//!
//! VERIFIED: every control centre listed in `data/photo_anchors_organ.csv` (crop
//! `NS4-Panel-organ.max-2000x2000.format-png.png`, x/y mapped affinely from the crop's section
//! outline onto [`super::section_rect`]); checked by `python tools/layout_overlay.py organ`,
//! max centre error 0.34 mm before the GUI label nudges (1.3 mm for the drawbar LED ladders,
//! whose height and offset stay as before so the ladder numerals clear the next slot).
//! Only the centres were verified: sizes are the old photo reads, except the faders, level
//! LED ladders and drawbar slots (heights from the crop). ASSUMED: the crop's section is
//! 1.29x taller in mm than the real one would be at the horizontal scale (see the spec).

use super::{px, Rect};

const DRAWBAR_X0: f32 = 485.0;
const DRAWBAR_PITCH: f32 = 25.4;

fn drawbar(id: &str) -> Option<Rect> {
    let (n, ladder) = if let Some(n) = id.strip_prefix("organ.drawbar_ladder.") {
        (n, true)
    } else {
        (id.strip_prefix("organ.drawbar.")?, false)
    };
    let n: u32 = n.parse().ok().filter(|n| (1..=9).contains(n))?;
    let cx = DRAWBAR_X0 + DRAWBAR_PITCH * (n - 1) as f32;
    Some(if ladder {
        px(cx + 4.0, 210.5, cx + 19.0, 270.5)
    } else {
        px(cx - 4.0, 236.0, cx + 4.0, 321.0)
    })
}

pub fn rect(id: &str) -> Option<Rect> {
    if let Some(r) = drawbar(id) {
        return Some(r);
    }
    Some(match id {
        "organ.on" => px(496.0, 85.5, 519.0, 98.5),
        "organ.fx_focus_led" => px(467.0, 92.0, 472.0, 97.0),
        "organ.fader.a" => px(416.5, 103.0, 430.5, 195.0),
        "organ.fader.b" => px(448.0, 103.0, 462.0, 195.0),
        "organ.ladder.a" => px(434.5, 107.5, 443.5, 187.0),
        "organ.ladder.b" => px(465.0, 107.5, 474.0, 187.0),
        "organ.focus_led.a" => px(420.0, 209.0, 426.0, 215.0),
        "organ.aux_led.a" => px(438.5, 199.0, 444.5, 205.0),
        "organ.focus_led.b" => px(450.0, 209.0, 456.0, 215.0),
        "organ.aux_led.b" => px(469.0, 199.0, 475.0, 205.0),
        "organ.layer.a" => px(419.0, 221.0, 442.0, 233.0),
        "organ.layer.b" => px(449.0, 221.0, 473.0, 233.0),
        "organ.octave_down" => px(422.5, 307.5, 444.5, 318.5),
        "organ.octave_up" => px(445.5, 307.5, 468.5, 318.5),
        "organ.zone_leds" => px(433.5, 330.0, 457.5, 336.0),
        "organ.model" => px(499.0, 160.0, 522.0, 172.0),
        "organ.model_leds" => px(504.5, 127.0, 517.5, 154.0),
        "organ.preset" => px(434.5, 271.0, 457.5, 283.0),
        "organ.vibrato_selector" => px(546.5, 130.5, 570.5, 143.5),
        "organ.vibrato_on.a" => px(560.5, 163.5, 571.5, 176.5),
        "organ.vibrato_on.b" => px(572.0, 163.5, 583.0, 176.5),
        "organ.perc_volume" => px(614.5, 134.5, 637.5, 147.5),
        "organ.perc_decay" => px(642.5, 134.5, 665.5, 147.5),
        "organ.perc_third" => px(670.5, 134.5, 693.5, 147.5),
        "organ.perc_on" => px(670.5, 163.5, 693.5, 176.5),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_drawbars_in_a_row_left_to_right() {
        let xs: Vec<f32> = (1..=9)
            .map(|n| rect(&format!("organ.drawbar.{n}")).unwrap().x)
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
        assert!(rect("organ.drawbar.0").is_none());
        assert!(rect("organ.drawbar.10").is_none());
    }

    #[test]
    fn ladder_sits_right_of_its_drawbar_slot() {
        let d = rect("organ.drawbar.4").unwrap();
        let l = rect("organ.drawbar_ladder.4").unwrap();
        assert!(l.x >= d.right() - 1e-3);
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(rect("piano.on").is_none());
    }
}
