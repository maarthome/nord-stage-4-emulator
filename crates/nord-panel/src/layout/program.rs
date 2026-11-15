//! Central Program area: OLED, Program buttons, Morph Assign, Split, Mst Clk, Transpose,
//! Preset Library, the right-hand Solo / Section Edit / Mon-Copy column and the two SHIFT rockers.
//!
//! VERIFIED (centres): every entry is placed from the HA88 top-down render, see [`super::topdown`];
//! the pixel centres are the ones in `data/photo_anchors_program.csv`, checked by
//! `python tools/layout_overlay.py program --source layout` (centre error 0 mm by construction,
//! origin uncertainty +-1.5 mm in y). Sizes are the older hero-photo reads (`ASSUMED`, +-1 mm),
//! except the OLED (498 x 256 px = 59.5 x 30.6 mm, measured) and the LED strip of SPLIT (140 px).

use super::topdown::td;
use super::Rect;

/// Centre x of the four Program button columns and y of its two rows (top-down px).
const BUTTON_X: [f32; 4] = [5215.0, 5361.0, 5505.0, 5650.0];
const BUTTON_Y: [f32; 2] = [1599.0, 1751.0];

/// Program button `n` (1..=8): two rows of four.
fn program_button(id: &str) -> Option<Rect> {
    let n: u32 = id
        .strip_prefix("program.button.")?
        .parse()
        .ok()
        .filter(|n| (1..=8).contains(n))?;
    let col = ((n - 1) % 4) as usize;
    let row = ((n - 1) / 4) as usize;
    Some(td(BUTTON_X[col], BUTTON_Y[row], 15.2, 8.3))
}

pub fn rect(id: &str) -> Option<Rect> {
    if let Some(r) = program_button(id) {
        return Some(r);
    }
    Some(match id {
        "program.oled" => td(5432.0, 1262.0, 59.5, 30.6),
        "program.dial" => td(5023.0, 1204.0, 12.4, 12.4),
        "program.page_down" => td(4956.0, 1379.0, 15.2, 8.3),
        "program.page_up" => td(5091.0, 1379.0, 15.2, 8.3),
        "program.shift.left" => td(5865.0, 1717.0, 9.7, 16.6),
        "program.shift.right" => td(8210.0, 1720.0, 9.7, 16.6),
        "program.prog_view" => td(5850.0, 970.0, 15.9, 9.0),
        "program.live_mode" => td(4994.0, 1542.0, 15.2, 8.3),
        "program.store" => td(4997.0, 1009.0, 15.9, 9.0),
        "program.solo" => td(5850.0, 1156.0, 15.9, 9.0),
        "program.section_edit" => td(5850.0, 1336.0, 15.9, 9.0),
        "program.mon_copy" => td(5850.0, 1497.0, 15.9, 9.0),
        "program.layer_scene_ii" => td(4994.0, 1738.0, 15.2, 8.3),
        "program.midi_led" => td(5137.0, 998.0, 3.5, 3.5),
        "program.extern_led" => td(5137.0, 1072.0, 3.5, 3.5),
        "morph.wheel" => td(4991.0, 815.0, 15.9, 8.3),
        "morph.at" => td(5126.0, 815.0, 15.9, 8.3),
        "morph.ctrlped" => td(5262.0, 815.0, 15.9, 8.3),
        "split.on_set" => td(5468.0, 836.0, 15.2, 9.0),
        "split.leds" => td(5468.0, 749.0, 16.7, 3.5),
        "mst_clk.tap_set" => td(5659.0, 814.0, 15.9, 9.0),
        "transp.on_set" => td(5850.0, 815.0, 15.9, 9.0),
        "preset_library.organ" => td(5296.0, 1031.0, 15.2, 8.3),
        "preset_library.piano" => td(5431.0, 1031.0, 15.2, 8.3),
        "preset_library.synth" => td(5567.0, 1031.0, 15.2, 8.3),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_buttons_form_two_rows_of_four() {
        let b: Vec<Rect> = (1..=8)
            .map(|n| rect(&format!("program.button.{n}")).unwrap())
            .collect();
        assert!(b[0].x < b[1].x && (b[0].y - b[3].y).abs() < 1e-3);
        assert!(b[4].y > b[0].y && (b[0].x - b[4].x).abs() < 1e-3);
        assert!(rect("program.button.9").is_none());
        assert!(rect("program.button.0").is_none());
    }

    #[test]
    fn oled_is_wider_than_tall() {
        let o = rect("program.oled").unwrap();
        assert!(o.w > o.h);
        assert!(rect("synth.oled").is_none());
    }

    /// The OLED is 4:3-ish landscape: 498 x 256 px on the HA88 render = 59.5 x 30.6 mm.
    #[test]
    fn oled_measures_59_5_by_30_6_mm() {
        let o = rect("program.oled").unwrap();
        assert!((o.w - 59.5).abs() < 0.1 && (o.h - 30.6).abs() < 0.1);
    }

    /// Row pitch 152 px = 18.2 mm, column pitch 145 px = 17.3 mm (top-down render).
    #[test]
    fn program_button_pitch_matches_the_render() {
        let (b1, b2, b5) = (
            rect("program.button.1").unwrap(),
            rect("program.button.2").unwrap(),
            rect("program.button.5").unwrap(),
        );
        assert!((b2.x - b1.x - 17.4).abs() < 0.3);
        assert!((b5.y - b1.y - 18.2).abs() < 0.3);
    }
}
