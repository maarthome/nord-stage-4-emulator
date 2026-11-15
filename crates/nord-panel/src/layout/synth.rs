//! Synth section (photo crop x 1029..1401): faders, OLED with its three
//! dials, then the Arpeggiator/Gate, Voice, Vibrato, LFO, Oscillators,
//! Filter, Amp and Unison areas.
//!
//! VERIFIED: every control centre is the real centre listed in `data/photo_anchors_synth.csv`
//! (crop `NS4-Panel-synth.max-2000x2000.format-png.png`, read by eye, +-3 crop px = +-0.4 mm),
//! mapped affinely from the crop's panel outline (x 12..1988, y 12..1066) onto
//! [`super::section_rect`]; checked by the test `centres_match_the_photo_anchors` and by
//! `python tools/layout_overlay.py synth`. Only the centres were verified: sizes are the old
//! hero-photo reads, except the faders and level ladders (heights from the crop).
//! ASSUMED: the crop's section is 1.29x taller in mm than the real one would be at the
//! horizontal scale (see the spec).

use super::{px, Rect, KEYBED_TOP_PX, ORIGIN_PX};

/// Crop px of the panel outline (left, top, width, height).
const CROP: (f32, f32, f32, f32) = (12.0, 12.0, 1976.0, 1054.0);
/// Section frame in hero-photo px (left, width); the height is the control area.
const SECTION_X: (f32, f32) = (1029.0, 372.0);
const SECTION_H: f32 = KEYBED_TOP_PX - ORIGIN_PX.1;

/// Rectangle `w` x `h` hero px centred on crop pixel `(cx, cy)`.
fn c(cx: f32, cy: f32, w: f32, h: f32) -> Rect {
    let x = SECTION_X.0 + (cx - CROP.0) / CROP.2 * SECTION_X.1;
    let y = ORIGIN_PX.1 + (cy - CROP.1) / CROP.3 * SECTION_H;
    px(x - w / 2.0, y - h / 2.0, x + w / 2.0, y + h / 2.0)
}

pub fn rect(id: &str) -> Option<Rect> {
    Some(match id {
        // Header, layer strip.
        "synth.on" => c(668.0, 55.0, 23.0, 9.0),
        "synth.fx_focus_led" => c(467.0, 67.0, 5.0, 5.0),
        "synth.fader.a" => c(66.0, 285.0, 13.0, 91.0),
        "synth.fader.b" => c(228.0, 285.0, 14.0, 91.0),
        "synth.fader.c" => c(389.0, 285.0, 14.0, 91.0),
        "synth.ladder.a" => c(144.0, 280.0, 9.0, 80.0),
        "synth.ladder.b" => c(305.0, 280.0, 9.0, 80.0),
        "synth.ladder.c" => c(467.0, 280.0, 9.0, 80.0),
        "synth.focus_led.a" => c(59.0, 545.0, 6.0, 6.0),
        "synth.aux_led.a" => c(158.0, 506.0, 6.0, 6.0),
        "synth.focus_led.b" => c(220.0, 545.0, 6.0, 6.0),
        "synth.aux_led.b" => c(320.0, 506.0, 6.0, 6.0),
        "synth.focus_led.c" => c(381.0, 545.0, 6.0, 6.0),
        "synth.aux_led.c" => c(481.0, 506.0, 6.0, 6.0),
        "synth.layer.a" => c(100.0, 604.0, 23.0, 12.0),
        "synth.layer.b" => c(262.0, 604.0, 23.0, 12.0),
        "synth.layer.c" => c(423.0, 604.0, 24.0, 12.0),
        "synth.octave_down" => c(120.0, 955.0, 20.0, 13.0),
        "synth.octave_up" => c(243.0, 955.0, 20.0, 13.0),
        "synth.zone_leds" => c(182.0, 1037.0, 23.0, 6.0),
        "synth.kb_hold" => c(100.0, 790.0, 23.0, 13.0),
        "synth.arp.run" => c(262.0, 790.0, 23.0, 13.0),
        // OLED, dials, mode, waveform.
        "synth.oled" => c(846.0, 240.0, 84.0, 43.0),
        "synth.dial.left" => c(629.0, 572.0, 22.0, 22.0),
        "synth.dial.middle" => c(846.0, 572.0, 22.0, 22.0),
        "synth.dial.right" => c(1064.0, 572.0, 22.0, 22.0),
        "synth.mode" => c(1197.0, 266.0, 23.0, 13.0),
        "synth.waveform" => c(1229.0, 497.0, 14.0, 26.0),
        // Arpeggiator / Gate.
        "synth.arp.rate" => c(1398.0, 232.0, 22.0, 22.0),
        "synth.arp.mode" => c(1580.0, 285.0, 23.0, 13.0),
        "synth.arp.range" => c(1763.0, 232.0, 22.0, 22.0),
        "synth.arp.menu" => c(1913.0, 262.0, 14.0, 26.0),
        // Voice and Vibrato.
        "synth.voice.mode" => c(1397.0, 560.0, 23.0, 13.0),
        "synth.voice.glide" => c(1583.0, 510.0, 22.0, 22.0),
        "synth.vibrato.selector" => c(1778.0, 570.0, 23.0, 13.0),
        "synth.vibrato.menu" => c(1914.0, 548.0, 14.0, 26.0),
        // LFO.
        "synth.lfo.waveform" => c(452.0, 762.0, 23.0, 13.0),
        "synth.lfo.rate" => c(470.0, 918.0, 22.0, 22.0),
        "synth.lfo.mod_amt" => c(672.0, 777.0, 22.0, 22.0),
        "synth.lfo.dest" => c(672.0, 1010.0, 23.0, 13.0),
        // Oscillators.
        "synth.osc.pitch_smp" => c(893.0, 770.0, 23.0, 13.0),
        "synth.osc.envelope" => c(1118.0, 770.0, 23.0, 13.0),
        "synth.osc.ctrl" => c(893.0, 945.0, 24.0, 24.0),
        "synth.osc.env_amt" => c(1119.0, 945.0, 20.0, 20.0),
        // Filter.
        "synth.filter.type" => c(1331.0, 770.0, 23.0, 13.0),
        "synth.filter.envelope" => c(1518.0, 770.0, 23.0, 13.0),
        "synth.filter.env_amt" => c(1706.0, 762.0, 20.0, 22.0),
        "synth.filter.freq" => c(1363.0, 945.0, 24.0, 24.0),
        "synth.filter.res" => c(1593.0, 945.0, 20.0, 20.0),
        // Amp and Unison.
        "synth.amp.envelope" => c(1893.0, 770.0, 23.0, 13.0),
        "synth.unison" => c(1893.0, 1015.0, 23.0, 13.0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::section_rect;
    use crate::types::Section;

    #[test]
    fn three_dials_left_to_right_under_the_oled() {
        let o = rect("synth.oled").unwrap();
        let l = rect("synth.dial.left").unwrap();
        let m = rect("synth.dial.middle").unwrap();
        let r = rect("synth.dial.right").unwrap();
        assert!(l.x < m.x && m.x < r.x);
        assert!(l.y > o.bottom());
    }

    #[test]
    fn three_faders_for_layers_a_b_c() {
        let a = rect("synth.fader.a").unwrap();
        let c = rect("synth.fader.c").unwrap();
        assert!(a.x < c.x);
        assert!(rect("synth.fader.d").is_none());
        assert!(rect("organ.on").is_none());
    }

    /// Every anchor of `data/photo_anchors_synth.csv` is the rectangle's centre after the
    /// affine crop -> section map.
    #[test]
    fn centres_match_the_photo_anchors() {
        let csv = include_str!("../../data/photo_anchors_synth.csv");
        let sec = section_rect(Section::Synth);
        let mut n = 0;
        for line in csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split(',').collect();
            let (cx, cy): (f32, f32) = (f[1].parse().unwrap(), f[2].parse().unwrap());
            let r = rect(f[0]).unwrap_or_else(|| panic!("no rect for {}", f[0]));
            let mx = sec.x + (cx - CROP.0) / CROP.2 * sec.w;
            let my = sec.y + (cy - CROP.1) / CROP.3 * sec.h;
            let (rx, ry) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            assert!((rx - mx).abs() < 0.05 && (ry - my).abs() < 0.05, "{}", f[0]);
            n += 1;
        }
        assert!(n >= 50, "{n} anchors");
    }
}
