//! Layer Effects section: FX Focus column, Mod 1, Mod 2, Amp Sim/EQ on the left; Delay, Comp and
//! Reverb on the right.
//!
//! VERIFIED (centres): placed from `.reference/crops/NS4-Panel-effects.max-2000x2000.format-png.png`
//! (1799 x 1192 px, git-ignored) mapped ISOTROPICALLY to panel mm: 0.11743 mm per crop px on both
//! axes, crop (0, 0) at panel (922.55, 22.85) mm. The mapping comes from registering the crop onto the
//! straight-on HA88 render ([`super::topdown`], 0.119488 mm/px): FFT cross-correlation peak 0.76 at
//! 1.0175 crop px per render px. The crop shows the section header but not the 26 mm connector strip
//! above it nor the 18 mm lip below, so it must not be stretched onto the section rectangle.
//! The centres are `data/photo_anchors_effects.csv`; checked by
//! `python tools/layout_overlay.py effects --source layout` (centre error 0 mm by construction, about
//! 0.5 mm reading error, +-1.5 mm absolute y). Sizes keep the older hero-photo reads (`ASSUMED`, +-1 mm).

use super::Rect;

/// Millimetres per crop pixel (`0.119488 / 1.0175`).
const MM_PER_CROP_PX: f32 = 0.11743;

/// Panel position in mm of crop pixel (0, 0).
const CROP_ORIGIN_MM: (f32, f32) = (922.55, 22.85);

/// Rectangle of `w` x `h` mm centred on crop pixel `(cx, cy)`.
fn cp(cx: f32, cy: f32, w: f32, h: f32) -> Rect {
    let (mx, my) = (
        CROP_ORIGIN_MM.0 + cx * MM_PER_CROP_PX,
        CROP_ORIGIN_MM.1 + cy * MM_PER_CROP_PX,
    );
    Rect::new(mx - w / 2.0, my - h / 2.0, w, h)
}

/// Knob, button and rocker sizes in mm (hero-photo reads).
const KNOB: f32 = 15.2;
const BTN: (f32, f32) = (15.9, 9.0);

pub fn rect(id: &str) -> Option<Rect> {
    let knob = |x: f32, y: f32| cp(x, y, KNOB, KNOB);
    let btn = |x: f32, y: f32| cp(x, y, BTN.0, BTN.1);
    let rocker = |x: f32, y: f32, h: f32| cp(x, y, 8.8, h);
    Some(match id {
        // Header and FX Focus column.
        "fx.layer_effects_on" => btn(662.0, 60.0),
        "fx.focus.organ" => btn(97.0, 323.0),
        "fx.focus.piano" => btn(97.0, 575.0),
        "fx.focus.synth" => btn(97.0, 828.0),
        // Mod 1 / Mod 2.
        "mod1.rate" => knob(308.0, 257.0),
        "mod1.amount" => knob(548.0, 257.0),
        "mod1.selector" => btn(757.0, 331.0),
        "mod1.on" => rocker(943.0, 312.0, 17.3),
        "mod2.rate" => knob(308.0, 540.0),
        "mod2.amount" => knob(548.0, 540.0),
        "mod2.selector" => btn(757.0, 607.0),
        "mod2.on" => rocker(943.0, 590.0, 16.6),
        // Amp Sim / EQ.
        "amp.drive" => knob(308.0, 808.0),
        "amp.mid_freq" => knob(548.0, 808.0),
        "amp.model" => btn(795.0, 885.0),
        "amp.on" => rocker(943.0, 1050.0, 16.6),
        "amp.bass" => knob(303.0, 1052.0),
        "amp.mid" => knob(549.0, 1052.0),
        "amp.treble" => knob(793.0, 1052.0),
        // Delay.
        "delay.tempo" => knob(1135.0, 283.0),
        "delay.feedback" => knob(1585.0, 215.0),
        "delay.effects" => btn(1348.0, 368.0),
        "delay.filter" => btn(1568.0, 488.0),
        "delay.dry_wet" => knob(1357.0, 585.0),
        "delay.tap_set" => btn(1130.0, 565.0),
        "delay.on" => cp(1587.0, 622.0, 15.9, 8.3),
        // Comp.
        "comp.amount" => knob(1133.0, 865.0),
        "comp.on" => cp(1133.0, 1073.0, 15.9, 8.3),
        // Reverb.
        "reverb.bright_dark" => btn(1346.0, 873.0),
        "reverb.type" => btn(1555.0, 918.0),
        "reverb.dry_wet" => knob(1365.0, 1060.0),
        "reverb.on" => cp(1587.0, 1073.0, 15.9, 8.3),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod2_is_below_mod1() {
        let a = rect("mod1.rate").unwrap();
        let b = rect("mod2.rate").unwrap();
        assert!(b.y > a.bottom());
    }

    #[test]
    fn delay_is_right_of_the_mod_rockers() {
        let on = rect("mod1.on").unwrap();
        let d = rect("delay.tempo").unwrap();
        assert!(d.x > on.right());
        assert!(rect("synth.on").is_none());
    }
}
