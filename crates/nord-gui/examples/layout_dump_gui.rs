//! Prints the layout JSON with the rectangle the GUI really draws (layout plus the
//! `layout_tweaks` nudges) as `gui` on every control, in panel mm.
//!
//!     cargo run -p nord-gui --example layout_dump_gui > target/layout_check/layout_gui.json
use egui::pos2;
use nord_gui::panel::layout_px::{rect_of, PX_PER_MM};
use nord_panel::layout::{dump, Rect};

fn main() {
    let gui = |id: &str| {
        let r = rect_of(id, pos2(0.0, 0.0))?;
        Some(Rect::new(
            r.min.x / PX_PER_MM,
            r.min.y / PX_PER_MM,
            r.width() / PX_PER_MM,
            r.height() / PX_PER_MM,
        ))
    };
    println!("{}", dump::to_json(Some(&gui)));
}
