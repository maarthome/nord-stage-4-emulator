//! Prints the panel layout (sections, frames, control rectangles in mm) as JSON.
//!
//!     cargo run -p nord-panel --example layout_dump > target/layout_check/layout.json
fn main() {
    println!("{}", nord_panel::layout::dump::to_json(None));
}
