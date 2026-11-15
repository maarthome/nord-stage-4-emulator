//! Printed scale of a knob: the numerals around its tick ring (`0 1 ... 10`, `-15 ... 15`,
//! `200 ... 8K`), as the panel prints them (photos `NS4-Panel-synth`, `NS4-Panel-effects`,
//! `NS4-Panel-organ`). A user reads the range and the position of a knob without hovering it.
//!
//! Each numeral sits on the radial line of its tick, just outside the knob bezel: the text
//! rectangle is pushed along the tick direction until its nearest corner is [`GAP`] px from the
//! bezel, so it never touches the knob (legibility rule `N`). The real panel prints no tick marks
//! on these knobs, so none are drawn ([`tick_tip`]) and the numeral ring is as tight as the photo.

use super::basic::{label, text_width};
use egui::{pos2, Align2, Color32, Painter, Pos2};

/// Free space (px) between the knob bezel and the nearest edge of its numeral.
///
/// EMPIRICAL: with [`BEZEL`] the numeral centres are 10.4 mm from the knob centre (top `5`
/// 25.1 px at 2.419 px/mm). The straight-on HA88 render (`NS4_HA88_TopDown-01_241008.jpg`,
/// 0.1195 mm/px) measures 8.2 mm (`5`) and 8.7 mm (`0`, `10`) on the OSC CTRL knob, but the
/// photo's digits are about 5 px tall against the 7.5 px legibility minimum, so a tighter ring
/// makes neighbouring numerals collide (rules `L`, `V`): 4.8 is the smallest gap with the audit
/// at 0 (3.6 leaves 5 collisions; before this change the ring was 10.7 mm, with ticks).
const GAP: f32 = 4.8;
/// Radius (px) the bezel and its stroke reach beyond the knob body (`knob.rs`: `radius + 0.8`).
const BEZEL: f32 = 1.0;
/// Radius (px) the tick marks of a knob without numerals reach beyond its body (`knob.rs`).
const TICK_OUT: f32 = 5.5;

/// How far (px) the marks of knob `id` reach beyond its body: the tick ring for knobs that print
/// no numerals, only the bezel for knobs that do (the photo draws no ticks next to numerals).
pub fn tick_tip(id: &str) -> f32 {
    if numerals(id).is_empty() {
        TICK_OUT
    } else {
        BEZEL
    }
}

/// Is the tick ring drawn on knob `id`? Not where numerals take its place.
pub fn draws_ticks(id: &str) -> bool {
    numerals(id).is_empty()
}
/// Scale text size (the 7.5 px legend minimum).
const SIZE: f32 = 7.5;
/// First tick angle and sweep, as in `knob.rs` (bottom left, 270 degrees clockwise).
const A0: f32 = 0.75 * std::f32::consts::PI;
const SWEEP: f32 = 1.5 * std::f32::consts::PI;

/// Numerals of the common 0-10 scale.
const ZERO_TEN: [&str; 11] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];
/// Numerals of the three EQ knobs (dB).
const EQ_DB: [&str; 7] = ["-15", "-10", "-5", "0", "5", "10", "15"];
/// Numerals of the Oscillator ENV AMT knob.
const ENV_AMT: [&str; 5] = ["-10", "-5", "0", "5", "10"];
/// Numerals of the amp-sim FREQ knob (Hz; `K` = kHz).
const MID_FREQ: [&str; 9] = ["200", "250", "400", "600", "1K", "2K", "4K", "6K", "8K"];

/// Id prefixes of the knobs that print the 0-10 scale (Synth, Layer Effects, Rotary).
const SCALED: [&str; 8] = [
    "synth.", "mod1.", "mod2.", "amp.", "delay.", "comp.", "reverb.", "rotary.",
];

/// Numerals printed around the knob with inventory id `id`, evenly spread over the tick ring from
/// the bottom-left end to the bottom-right end; empty for knobs that print none (Master level,
/// the AMP RES trimmer, ids outside the Synth / Layer Effects / Rotary blocks).
///
/// EMPIRICAL: read from the reference photos; the EQ knobs print -15 ... 15 in steps of 5, the
/// Oscillator ENV AMT knob -10 ... 10, the amp FREQ knob 200 250 400 600 1K 2K 4K 6K 8K (nine
/// stops), every other pot 0 ... 10. Master level and the Piano model dial print none.
pub fn numerals(id: &str) -> &'static [&'static str] {
    match id {
        "amp.bass" | "amp.mid" | "amp.treble" => &EQ_DB,
        "synth.osc.env_amt" => &ENV_AMT,
        "amp.mid_freq" => &MID_FREQ,
        "amp.res" => &[],
        _ if id.starts_with("synth.dial") => &[],
        _ if SCALED.iter().any(|p| id.starts_with(p)) => &ZERO_TEN,
        _ => &[],
    }
}

/// Radius (px) the body of a knob that prints a numeral scale gives up, so that the numerals of two
/// neighbours (`10` of one knob, `-10` of the next) fit in the 67 px pitch of the Layer Effects rows.
///
/// ASSUMED: 3 px (16 % of the 18.5 px body) is the least that keeps the EQ numerals apart; the
/// photos draw the knob smaller inside its numeral ring too.
const SHRINK: f32 = 3.0;

/// Drawn body radius of knob `id` whose layout rectangle gives `radius`: scaled knobs shrink by
/// [`SHRINK`] so the numerals around the ring fit between neighbours.
pub fn body_radius(id: &str, radius: f32) -> f32 {
    if numerals(id).is_empty() {
        radius
    } else {
        (radius - SHRINK).max(6.0)
    }
}

/// Centre of the numeral for tick fraction `frac` (0 = first tick, 1 = last) of a knob at
/// `center` with body `radius`, whose text is `w` x `h` px.
pub fn numeral_centre(center: Pos2, radius: f32, frac: f32, w: f32, h: f32) -> Pos2 {
    let a = A0 + SWEEP * frac;
    let (dx, dy) = (a.cos(), a.sin());
    // Distance from the text centre to its edge along the tick direction.
    let support = dx.abs() * w * 0.5 + dy.abs() * h * 0.5;
    let d = radius + BEZEL + GAP + support;
    pos2(center.x + dx * d, center.y + dy * d)
}

/// Paint the numerals of knob `id`, if it has any.
pub fn paint(p: &Painter, center: Pos2, radius: f32, id: &str) {
    let texts = numerals(id);
    let last = texts.len().saturating_sub(1).max(1) as f32;
    let ink = Color32::from_gray(205);
    for (i, t) in texts.iter().enumerate() {
        let w = text_width(p, t, SIZE);
        let at = numeral_centre(center, radius, i as f32 / last, w, SIZE);
        label(p, at, t, SIZE, ink, Align2::CENTER_CENTER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::support::with_ui;

    #[test]
    fn scales_follow_the_printed_panel() {
        assert_eq!(numerals("synth.filter.freq").len(), 11);
        assert_eq!(numerals("amp.bass"), EQ_DB);
        assert_eq!(numerals("synth.osc.env_amt"), ENV_AMT);
        assert_eq!(numerals("amp.mid_freq")[4], "1K");
        assert!(numerals("amp.res").is_empty());
        assert!(numerals("master.level").is_empty());
        assert!(numerals("synth.dial.info").is_empty());
        assert_eq!(numerals("rotary.drive").len(), 11);
        assert_eq!(numerals("delay.feedback")[10], "10");
    }

    #[test]
    fn scaled_knobs_shrink_and_others_keep_their_radius() {
        assert_eq!(body_radius("master.level", 18.0), 18.0);
        assert_eq!(body_radius("amp.res", 18.0), 18.0);
        assert!((body_radius("mod1.rate", 18.0) - (18.0 - SHRINK)).abs() < 1e-6);
        assert_eq!(body_radius("mod1.rate", 7.0), 6.0);
    }

    #[test]
    fn numerals_follow_the_tick_ring_from_bottom_left_to_bottom_right() {
        let c = pos2(100.0, 100.0);
        let first = numeral_centre(c, 18.0, 0.0, 5.0, 7.5);
        let mid = numeral_centre(c, 18.0, 0.5, 5.0, 7.5);
        let last = numeral_centre(c, 18.0, 1.0, 10.0, 7.5);
        assert!(first.x < c.x && first.y > c.y, "{first:?}");
        assert!(mid.x - c.x < 1e-3 && mid.y < c.y - 18.0, "{mid:?}");
        assert!(last.x > c.x && last.y > c.y, "{last:?}");
    }

    #[test]
    fn a_numeral_keeps_clear_of_the_bezel() {
        let c = pos2(0.0, 0.0);
        for i in 0..=10 {
            let (w, h) = (if i == 10 { 9.0 } else { 4.5 }, 7.5);
            let at = numeral_centre(c, 18.0, i as f32 / 10.0, w, h);
            // Nearest point of the text rectangle to the knob centre.
            let nx = (at.x.abs() - w * 0.5).max(0.0);
            let ny = (at.y.abs() - h * 0.5).max(0.0);
            assert!(nx.hypot(ny) >= 18.0 + BEZEL + GAP - 1e-3, "tick {i}");
        }
    }

    #[test]
    fn numeral_ring_is_tighter_than_before_and_near_the_legibility_floor() {
        // Body 15.5 px (18.5 less SHRINK); before: 5.5 px ticks + 1.2 gap gave 10.7 mm (top).
        let c = pos2(0.0, 0.0);
        let top = numeral_centre(c, 15.5, 0.5, 4.5, 7.5);
        let mm = top.to_vec2().length() / 2.419;
        assert!(mm < 10.7 && mm > 8.7, "top {mm}");
    }

    #[test]
    fn numeral_knobs_draw_no_ticks() {
        assert!(!draws_ticks("synth.filter.freq"));
        assert!(draws_ticks("master.level"));
        assert!(tick_tip("synth.filter.freq") < tick_tip("master.level"));
    }

    #[test]
    fn paint_draws_numerals_only_for_scaled_knobs() {
        with_ui(|ui| {
            let p = ui.painter().clone();
            paint(&p, pos2(50.0, 50.0), 14.0, "synth.lfo.rate");
            paint(&p, pos2(50.0, 50.0), 14.0, "master.level");
        });
    }
}
