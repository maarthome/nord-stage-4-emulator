//! Placement from the straight-on HA88 top-down render (`.reference/NS4_HA88_TopDown-01_241008.jpg`,
//! 11600 x 3866 px, git-ignored).
//!
//! VERIFIED: body x 400..11196 px = 1290 mm gives 0.1195 mm/px on both axes (white-key pitch
//! 197.7 px = 23.63 mm against the standard 23.5 mm, 52 white + 36 black keys counted);
//! the panel top edge is at y 482 px (rail edge 476..504 px along the width, so the absolute
//! y origin is uncertain by about +-1.5 mm; relative positions are exact to ~0.3 mm).
//! Checked by `python tools/layout_overlay.py program|keybed|wheels` against the centres in
//! `data/photo_anchors_{program,keybed,wheels}.csv` (same pixel coordinates as the tables).

use super::Rect;

/// Top-down px of the body's left edge and the panel's top edge.
const ORIGIN: (f32, f32) = (400.0, 482.0);

/// Millimetres per top-down pixel (`1290 mm / 10796 px`).
pub(crate) const MM_PER_TD_PX: f32 = 1290.0 / 10796.0;

/// Rectangle of `w` x `h` mm centred on top-down pixel `(cx, cy)`.
pub(crate) fn td(cx: f32, cy: f32, w: f32, h: f32) -> Rect {
    td_box(
        cx - w / 2.0 / MM_PER_TD_PX,
        cy - h / 2.0 / MM_PER_TD_PX,
        w,
        h,
    )
}

/// Rectangle of `w` x `h` mm with its top-left at top-down pixel `(x, y)`.
pub(crate) fn td_box(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(
        (x - ORIGIN.0) * MM_PER_TD_PX,
        (y - ORIGIN.1) * MM_PER_TD_PX,
        w,
        h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_maps_to_mm() {
        let r = td(5215.0, 1599.0, 15.2, 8.3);
        let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert!((cx - (5215.0 - 400.0) * MM_PER_TD_PX).abs() < 1e-3);
        assert!((cy - (1599.0 - 482.0) * MM_PER_TD_PX).abs() < 1e-3);
    }

    #[test]
    fn body_width_is_1290_mm() {
        assert!((10796.0 * MM_PER_TD_PX - 1290.0).abs() < 1e-3);
    }
}
