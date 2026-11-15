//! Third set of audit rules (LED spacing). LEDs must not be glued to their buttons or to each
//! other; the real panel leaves a visible gap (photos `NS4_Panel-crop-left.jpg`,
//! `NS4-Panel-organ`).
//!
//! | Rule | Meaning |
//! |---|---|
//! | `B` | an LED bezel is closer than [`LED_BUTTON`] px to a button bezel (the button rectangle plus 1.5 px), LEDs printed on the button excepted |
//! | `D` | two LED bezels (or two ladder bars) are closer than [`LED_LED`] px (centre pitch under diameter + 2); LEDs of a real ladder keep [`LADDER_LED`] px |

use super::legibility_tests::Audit;
use crate::widgets::probe::Kind;
use egui::{Pos2, Rect};
use nord_panel::ControlKind;
use std::collections::BTreeSet;

/// Free space (px) between an LED bezel and the bezel of the button it belongs to.
pub(super) const LED_BUTTON: f32 = 2.5;
/// Free space (px) between two LED bezels.
pub(super) const LED_LED: f32 = 2.0;
/// Free space (px) between two LED bezels of a level ladder (the photo shows a tight pitch).
pub(super) const LADDER_LED: f32 = 1.5;
/// Button bezel around a control rectangle (`ctl::gesture_button` draws `expand(1.5)`).
const BUTTON_BEZEL: f32 = 1.5;
/// Rectangles above this area are containers (OLED, groups), not buttons.
const MAX_BUTTON_PX2: f32 = 3000.0;

/// Distance from `c` to the nearest point of `r` (0 inside).
fn dist_to_rect(c: Pos2, r: Rect) -> f32 {
    let q = Pos2::new(c.x.clamp(r.min.x, r.max.x), c.y.clamp(r.min.y, r.max.y));
    (q - c).length()
}

/// Circle of an LED record: centre and bezel radius.
fn circle(r: Rect) -> (Pos2, f32) {
    (r.center(), r.width() * 0.5)
}

/// Pixel rectangles of the panel's buttons (layout rectangles of `Button` controls).
fn buttons() -> Vec<(String, Rect)> {
    let mut seen = BTreeSet::new();
    nord_panel::all_controls()
        .iter()
        .filter(|c| c.kind == ControlKind::Button)
        .filter_map(|c| Some((c.id.clone(), super::layout_px::rect_of(&c.id, Pos2::ZERO)?)))
        .filter(|(_, r)| r.area() < MAX_BUTTON_PX2)
        .filter(|(_, r)| seen.insert((r.min.x as i32, r.min.y as i32, r.max.x as i32)))
        .collect()
}

/// Rectangles of the level ladders (LED columns next to faders).
fn ladders() -> Vec<Rect> {
    nord_panel::all_controls()
        .iter()
        .filter(|c| c.kind == ControlKind::LedLadder)
        .filter_map(|c| super::layout_px::rect_of(&c.id, Pos2::ZERO))
        .map(|r| r.expand(2.0))
        .collect()
}

/// Rule `B`: LEDs glued to a button bezel.
fn led_button(a: &Audit, out: &mut BTreeSet<String>) {
    let btns = buttons();
    for led in a.recs.iter().filter(|r| r.kind == Kind::Led) {
        let (c, rad) = circle(led.rect);
        for (id, b) in &btns {
            if b.contains(c) {
                continue;
            }
            let gap = dist_to_rect(c, b.expand(BUTTON_BEZEL)) - rad;
            if gap < LED_BUTTON - 1e-3 {
                out.insert(format!("B led@{:.0},{:.0} {id} gap {gap:.1}", c.x, c.y));
            }
        }
    }
}

/// Rule `D`: LEDs glued to each other.
fn led_led(a: &Audit, out: &mut BTreeSet<String>) {
    let ladders = ladders();
    let leds: Vec<_> = a
        .recs
        .iter()
        .filter(|r| r.kind == Kind::Led)
        .map(|r| circle(r.rect))
        .collect();
    for (i, &(ca, ra)) in leds.iter().enumerate() {
        for &(cb, rb) in &leds[i + 1..] {
            let gap = (ca - cb).length() - ra - rb;
            let ladder = ladders.iter().any(|l| l.contains(ca) && l.contains(cb));
            let need = if ladder { LADDER_LED } else { LED_LED };
            if gap < need - 1e-3 {
                let (mut p, mut q) = ((ca.x, ca.y), (cb.x, cb.y));
                if p > q {
                    std::mem::swap(&mut p, &mut q);
                }
                out.insert(format!(
                    "D led@{:.0},{:.0} / led@{:.0},{:.0} gap {gap:.1}",
                    p.0, p.1, q.0, q.1
                ));
            }
        }
    }
}

/// Rule `D` for ladder bars: vertically stacked segments keep [`LADDER_LED`] px between them.
fn segments(a: &Audit, out: &mut BTreeSet<String>) {
    let segs: Vec<Rect> = a
        .recs
        .iter()
        .filter(|r| r.kind == Kind::LedSeg)
        .map(|r| r.rect)
        .collect();
    // Round LEDs beside a ladder (SOFT REL / STR RES next to the piano ladder) keep their distance.
    for led in a.recs.iter().filter(|r| r.kind == Kind::Led) {
        let (c, rad) = circle(led.rect);
        for s in &segs {
            let gap = dist_to_rect(c, *s) - rad;
            if gap < LED_LED - 1e-3 {
                out.insert(format!(
                    "D led@{:.0},{:.0} / seg@{:.0},{:.0} gap {gap:.1}",
                    c.x, c.y, s.min.x, s.min.y
                ));
            }
        }
    }
    for (i, s) in segs.iter().enumerate() {
        for t in &segs[i + 1..] {
            let shared = s.max.x.min(t.max.x) - s.min.x.max(t.min.x);
            let gap = (t.min.y - s.max.y).max(s.min.y - t.max.y);
            if shared > 0.5 * s.width().min(t.width()) && gap < LADDER_LED - 1e-3 && gap > -0.5 {
                out.insert(format!("D seg@{:.0},{:.0} gap {gap:.1}", s.min.x, s.min.y));
            }
        }
    }
}

/// All violations of this file's rules as stable one-line keys.
pub(super) fn violations(a: &Audit) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    led_button(a, &mut v);
    led_led(a, &mut v);
    segments(a, &mut v);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::probe::Rec;
    use egui::{pos2, vec2};

    fn led(x: f32, y: f32) -> Rec {
        Rec {
            kind: Kind::Led,
            rect: Rect::from_center_size(pos2(x, y), vec2(5.0, 5.0)),
            title: String::new(),
        }
    }

    fn audit(recs: Vec<Rec>) -> Audit {
        Audit {
            labs: vec![],
            visible: Rect::EVERYTHING,
            recs,
        }
    }

    #[test]
    fn leds_closer_than_two_px_are_reported() {
        let mut v = BTreeSet::new();
        led_led(&audit(vec![led(500.0, 0.0), led(506.0, 0.0)]), &mut v);
        assert_eq!(v.len(), 1, "{v:?}");
        v.clear();
        led_led(&audit(vec![led(500.0, 0.0), led(507.5, 0.0)]), &mut v);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn ladder_segments_need_a_gap() {
        let seg = |y: f32| Rec {
            kind: Kind::LedSeg,
            rect: Rect::from_min_size(pos2(0.0, y), vec2(10.0, 5.0)),
            title: String::new(),
        };
        let mut v = BTreeSet::new();
        segments(&audit(vec![seg(0.0), seg(6.0)]), &mut v);
        assert_eq!(v.len(), 1, "{v:?}");
        v.clear();
        segments(&audit(vec![seg(0.0), seg(6.6)]), &mut v);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn distance_to_rect_is_zero_inside_and_euclidean_outside() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0));
        assert_eq!(dist_to_rect(pos2(5.0, 5.0), r), 0.0);
        assert!((dist_to_rect(pos2(13.0, 14.0), r) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn the_real_panel_has_buttons_and_ladders() {
        assert!(buttons().len() > 50);
        assert!(!ladders().is_empty());
    }
}
