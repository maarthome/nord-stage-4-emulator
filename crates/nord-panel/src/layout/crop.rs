//! Placement from the section crops (`.reference/crops/NS4-Panel-{organ,piano}...png`, git-ignored)
//! with ONE isotropic scale, registered onto the straight-on HA88 render ([`super::topdown`]).
//!
//! The Organ pass (#375) and Piano pass (#382) stretched the crops onto the whole section rect
//! (179 mm), which is 1.23..1.30x taller than the real 137 mm section (a 25 mm connector strip
//! sits above it and an 18 mm lip below). Here a crop pixel maps to render pixel
//! `(rx + k * cx, ry + k * cy)` and from there to mm exactly like the Program / keybed tables
//! (`topdown::td`), the same mm per pixel on both axes.
//!
//! VERIFIED: `k`, `rx`, `ry` come from a multi-scale normalised template match of the crop against
//! the render (`cv2.matchTemplate`, `TM_CCOEFF_NORMED`; organ 0.920 at k = 0.995, piano 0.919 at
//! k = 0.990; the unconstrained x/y scale search returned kx = ky within 0.01). The check that
//! the crop section's top edge lands on the render's (y 691 px, B-R colour step, 25.0 mm) is
//! [`tests::section_tops_land_on_the_render_edge`]; the section widths (organ 201.9 mm, piano 86.1
//! mm) match the render within 1 mm. Registration error about +-1 render px (0.12 mm) plus the
//! +-0.25 % scale resolution (+-0.5 mm at the far edge of a crop).

use super::topdown::{td, MM_PER_TD_PX};
use super::Rect;

/// A crop registered onto the HA88 render.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Crop {
    /// Render px of crop `x = 0`.
    pub rx: f32,
    /// Render px of crop `y = 0`.
    pub ry: f32,
    /// Render px per crop px (both axes).
    pub k: f32,
}

impl Crop {
    /// Render px of a crop point.
    pub fn to_render(self, cx: f32, cy: f32) -> (f32, f32) {
        (self.rx + self.k * cx, self.ry + self.k * cy)
    }

    /// Panel mm of a crop point (the same origin as [`td`]).
    #[cfg(test)]
    pub fn mm(self, cx: f32, cy: f32) -> (f32, f32) {
        let r = self.rect(cx, cy, 0.0, 0.0);
        (r.x, r.y)
    }

    /// `w` x `h` mm rectangle centred on crop point `(cx, cy)`.
    pub fn rect(self, cx: f32, cy: f32, w: f32, h: f32) -> Rect {
        let (x, y) = self.to_render(cx, cy);
        td(x, y, w, h)
    }

    /// Rectangle centred on `(cx, cy)` with a size measured in crop px.
    pub fn rect_px(self, cx: f32, cy: f32, w_px: f32, h_px: f32) -> Rect {
        let s = self.k * MM_PER_TD_PX;
        self.rect(cx, cy, w_px * s, h_px * s)
    }
}

/// `NS4-Panel-organ...png` (1961 x 1185; Master strip, Organ section).
pub(crate) const ORGAN: Crop = Crop {
    rx: 2166.5,
    ry: 678.75,
    k: 0.995,
};

/// `NS4-Panel-piano...png` (1800 x 1197; Piano section at x 15..747).
pub(crate) const PIANO: Crop = Crop {
    rx: 4112.3,
    ry: 674.2,
    k: 0.99,
};

/// `NS4-Panel-synth...png` (2000 x 1067; Synth section, outline x 12..1988 / y 12..1066).
///
/// VERIFIED: the outline's left / right edges map onto the section's x edges (hero px 1029..1401),
/// so `k = 372 * MM_PER_PX / 1976 / MM_PER_TD_PX = 1.08978`; the outline top (crop y 12) lands on
/// the render's plate top, 691 px = 25.0 mm (`ry = 691 - 12 k`), like Organ and Piano. It used to
/// sit at 26.0 mm (8.4 px lower). Checked by `centres_match_the_photo_anchors` (synth.rs) and
/// `python tools/layout_overlay.py synth`.
pub(crate) const SYNTH: Crop = Crop {
    rx: 5955.69,
    ry: 678.15,
    k: 1.08978,
};

/// Test helper: every `id,x,y` row of `csv` (crop px) has a rectangle centred on that point
/// within `tol` mm; returns the number of rows checked.
#[cfg(test)]
pub(crate) fn check_csv(csv: &str, crop: Crop, tol: f32) -> usize {
    let mut n = 0;
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let (x, y): (f32, f32) = (f[1].parse().unwrap(), f[2].parse().unwrap());
        let r = super::rect_for(f[0]).unwrap_or_else(|| panic!("{} has no rect", f[0]));
        let (mx, my) = crop.mm(x, y);
        let d = ((r.x + r.w / 2.0 - mx).powi(2) + (r.y + r.h / 2.0 - my).powi(2)).sqrt();
        assert!(d < tol, "{}: centre {d:.3} mm from its anchor", f[0]);
        n += 1;
    }
    n
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Render row of the layer sections' top edge (red -> panel colour step, five columns agree
    /// within 2 px), and its mm value on the shared render origin.
    pub const SECTION_TOP_PX: f32 = 691.0;

    #[test]
    fn section_tops_land_on_the_render_edge() {
        // Organ outline top y = 12 px, piano section top y = 17 px in their crops.
        let (_, y) = ORGAN.to_render(0.0, 12.0);
        assert!((y - SECTION_TOP_PX).abs() < 2.0, "organ {y}");
        let (_, y) = PIANO.to_render(0.0, 17.0);
        assert!((y - SECTION_TOP_PX).abs() < 2.0, "piano {y}");
        // Synth outline top y = 12 px.
        let (_, y) = SYNTH.to_render(0.0, 12.0);
        assert!((y - SECTION_TOP_PX).abs() < 0.5, "synth {y}");
    }

    #[test]
    fn scale_is_isotropic_and_close_to_the_render() {
        let a = ORGAN.rect_px(100.0, 100.0, 100.0, 100.0);
        assert!((a.w - a.h).abs() < 1e-4);
        assert!((a.w - 100.0 * 0.995 * MM_PER_TD_PX).abs() < 1e-3);
        let (mx, my) = PIANO.mm(0.0, 0.0);
        assert!((mx - (4112.3 - 400.0) * MM_PER_TD_PX).abs() < 1e-3);
        assert!((my - (674.2 - 482.0) * MM_PER_TD_PX).abs() < 1e-3);
    }

    #[test]
    fn rect_is_centred_on_the_point() {
        let r = PIANO.rect(300.0, 400.0, 10.0, 6.0);
        let (mx, my) = PIANO.mm(300.0, 400.0);
        assert!((r.x + r.w / 2.0 - mx).abs() < 1e-4);
        assert!((r.y + r.h / 2.0 - my).abs() < 1e-4);
    }
}
