//! JSON dump of the whole layout, for `tools/layout_overlay.py` (the photo validation tool).
//!
//! `cargo run -p nord-panel --example layout_dump` prints it; the GUI example
//! (`cargo run -p nord-gui --example layout_dump_gui`) adds the rectangle that is really drawn
//! (layout plus the GUI-side nudges) to every control as `gui`.
//!
//! No serde: the schema is flat and written by hand.

use std::fmt::Write;

use super::{
    frame_rect, rect_for, section_rect, shared_base, Rect, ESTIMATED, FRAMES, MM_PER_PX,
    ORIGIN_PX,
};
use crate::inventory::all_controls;
use crate::types::Section;

/// Sections in panel order (name, section).
const SECTIONS: [(&str, Section); 7] = [
    ("master", Section::Master),
    ("organ", Section::Organ),
    ("piano", Section::Piano),
    ("program", Section::Program),
    ("synth", Section::Synth),
    ("effects", Section::Effects),
    ("keybed", Section::Keybed),
];

/// Lowercase section name used in the JSON and by the tools.
pub fn section_name(section: Section) -> &'static str {
    SECTIONS
        .iter()
        .find(|(_, s)| *s == section)
        .map_or("?", |(n, _)| n)
}

fn rect_json(r: Rect) -> String {
    format!(
        "{{\"x\":{:.3},\"y\":{:.3},\"w\":{:.3},\"h\":{:.3}}}",
        r.x, r.y, r.w, r.h
    )
}

/// The layout as JSON. `gui` maps a control id to the rectangle (mm) that is really drawn.
pub fn to_json(gui: Option<&dyn Fn(&str) -> Option<Rect>>) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"mm_per_px\":{:.6},\"origin_px\":[{},{}],\"sections\":{{",
        MM_PER_PX, ORIGIN_PX.0, ORIGIN_PX.1
    );
    for (i, (name, sec)) in SECTIONS.iter().enumerate() {
        let sep = if i > 0 { "," } else { "" };
        let _ = write!(s, "{sep}\"{name}\":{}", rect_json(section_rect(*sec)));
    }
    s.push_str("},\"frames\":[");
    let mut first = true;
    for f in FRAMES {
        let Some(r) = frame_rect(f) else { continue };
        let sep = if first { "" } else { "," };
        first = false;
        let _ = write!(
            s,
            "{sep}{{\"id\":\"{}\",\"section\":\"{}\",\"title\":\"{}\",\"rect\":{}}}",
            f.id,
            section_name(f.section),
            f.title.replace('"', "'"),
            rect_json(r)
        );
    }
    s.push_str("],\"controls\":[");
    first = true;
    for c in all_controls() {
        let Some(r) = rect_for(&c.id) else { continue };
        let sep = if first { "" } else { "," };
        first = false;
        let _ = write!(
            s,
            "{sep}{{\"id\":\"{}\",\"section\":\"{}\",\"label\":\"{}\",\"estimated\":{},\"shared\":{},\"rect\":{}",
            c.id,
            section_name(c.section),
            c.label.replace('"', "'"),
            ESTIMATED.contains(&c.id.as_str()),
            shared_base(&c.id).is_some(),
            rect_json(r)
        );
        if let Some(g) = gui.and_then(|f| f(&c.id)) {
            let _ = write!(s, ",\"gui\":{}", rect_json(g));
        }
        s.push('}');
    }
    s.push_str("]}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_lists_sections_frames_and_every_control() {
        let j = to_json(None);
        assert!(j.starts_with("{\"mm_per_px\":"));
        assert!(j.contains("\"organ\":{\"x\":"));
        assert!(j.contains("\"id\":\"master.level\""));
        assert!(j.contains("\"estimated\":true"));
        assert!(!j.contains("\"gui\""));
        assert_eq!(
            j.matches("\"label\":").count(),
            all_controls().len(),
            "one entry per inventory control"
        );
        assert_eq!(j.matches('{').count(), j.matches('}').count());
    }

    #[test]
    fn gui_callback_adds_the_drawn_rect() {
        let j = to_json(Some(&|id| {
            (id == "master.level").then(|| Rect::new(1.0, 2.0, 3.0, 4.0))
        }));
        assert_eq!(j.matches("\"gui\":").count(), 1);
        assert!(j.contains("\"gui\":{\"x\":1.000,\"y\":2.000,\"w\":3.000,\"h\":4.000}"));
    }

    #[test]
    fn section_names_are_lowercase() {
        assert_eq!(section_name(Section::Effects), "effects");
    }
}
