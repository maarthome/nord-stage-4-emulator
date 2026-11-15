//! Organ section, placed from the organ crop with the isotropic mapping of [`super::crop`]
//! (crop `NS4-Panel-organ.max-2000x2000.format-png.png`). Drawbar `n` sits at crop
//! `x = 655 + 146.81 * (n - 1)`, its LED ladder 76 crop px to the right of the slot.
//!
//! VERIFIED: every control centre listed in `data/photo_anchors_organ.csv` (crop px, read by eye,
//! about +-4 crop px = 0.5 mm) is the centre of the rectangle here, checked by
//! [`tests::centres_match_the_photo_anchor_csv`] and `python tools/layout_overlay.py organ`
//! (max error 0.0x mm before the GUI nudges); the absolute position follows the HA88 render,
//! see [`super::crop`]. The old affine placement put these up to 18 mm too low.
//! Sizes: the faders, level LED ladders, drawbar slots and drawbar ladders take their heights
//! from the crop (VERIFIED +-1 mm); the other sizes are the old photo reads (ASSUMED).

use super::crop::ORGAN as C;
use super::Rect;

const DRAWBAR_X0: f32 = 655.0;
const LADDER_X0: f32 = 731.0;
const DRAWBAR_PITCH: f32 = 146.81;

fn drawbar(id: &str) -> Option<Rect> {
    let (n, ladder) = if let Some(n) = id.strip_prefix("organ.drawbar_ladder.") {
        (n, true)
    } else {
        (id.strip_prefix("organ.drawbar.")?, false)
    };
    let n: u32 = n.parse().ok().filter(|n| (1..=9).contains(n))?;
    let step = DRAWBAR_PITCH * (n - 1) as f32;
    Some(if ladder {
        C.rect_px(LADDER_X0 + step, 725.0, 86.0, 357.0)
    } else {
        C.rect_px(DRAWBAR_X0 + step, 895.0, 36.0, 385.0)
    })
}

pub fn rect(id: &str) -> Option<Rect> {
    if let Some(r) = drawbar(id) {
        return Some(r);
    }
    Some(match id {
        "organ.on" => C.rect(783.0, 60.0, 15.9, 9.0),
        "organ.fx_focus_led" => C.rect(564.0, 73.0, 3.5, 3.5),
        "organ.fader.a" => C.rect_px(300.0, 316.0, 82.0, 406.0),
        "organ.fader.b" => C.rect_px(480.0, 316.0, 82.0, 406.0),
        "organ.ladder.a" => C.rect_px(389.0, 307.0, 52.0, 359.0),
        "organ.ladder.b" => C.rect_px(565.0, 307.0, 52.0, 359.0),
        "organ.focus_led.a" => C.rect(295.0, 597.0, 4.2, 4.2),
        "organ.aux_led.a" => C.rect(404.0, 553.0, 4.2, 4.2),
        "organ.focus_led.b" => C.rect(470.0, 597.0, 4.2, 4.2),
        "organ.aux_led.b" => C.rect(580.0, 553.0, 4.2, 4.2),
        "organ.layer.a" => C.rect(340.0, 663.0, 15.9, 8.3),
        "organ.layer.b" => C.rect(516.0, 663.0, 16.6, 8.3),
        "organ.octave_down" => C.rect(356.0, 1047.0, 15.2, 7.6),
        "organ.octave_up" => C.rect(493.0, 1047.0, 15.9, 7.6),
        "organ.zone_leds" => C.rect(427.0, 1136.0, 16.6, 4.2),
        "organ.model" => C.rect(803.0, 392.0, 15.9, 8.3),
        "organ.model_leds" => C.rect(804.0, 278.0, 9.0, 18.7),
        "organ.preset" => C.rect(428.0, 886.0, 15.9, 8.3),
        "organ.vibrato_selector" => C.rect(1078.0, 262.0, 16.6, 9.0),
        "organ.vibrato_on.a" => C.rect(1122.0, 410.0, 7.6, 7.5),
        "organ.vibrato_on.b" => C.rect(1189.0, 410.0, 7.6, 7.5),
        "organ.perc_volume" => C.rect(1470.0, 280.0, 15.9, 9.0),
        "organ.perc_decay" => C.rect(1631.0, 280.0, 15.9, 9.0),
        "organ.perc_third" => C.rect(1792.0, 280.0, 15.9, 9.0),
        "organ.perc_on" => C.rect(1792.0, 410.0, 15.9, 9.0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centres_match_the_photo_anchor_csv() {
        let csv = include_str!("../../data/photo_anchors_organ.csv");
        let n = super::super::crop::check_csv(csv, super::C, 0.15);
        assert!(n >= 40, "{n} anchors checked");
    }

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
