//! The standalone app's single top bar: a Menu pull-down that opens the tool windows (library,
//! engine options, Nord OS file, keyboard / MIDI / meter) and one button per view (the whole
//! panel and each section). Replaces the stack of tool bars above and below the panel.

use crate::panel::view::{ViewState, NAV};
use egui::Ui;
use nord_panel::Section;

/// Logical height every view reserves for the bar (`ViewState::nav_h`).
///
/// ASSUMED: one row of default-size buttons plus the 4 px frame margins; a bar slightly taller
/// only costs a few px of panel at the bottom.
pub const NAV_H: f32 = 30.0;

/// Titles of the tool windows, in Menu order (also the menu entries).
pub const MENU_ITEMS: [&str; 4] = [
    "Library (.ns4b, programs)",
    "Engine options",
    "Nord OS file",
    "Keyboard, MIDI and meter",
];

/// Which tool windows are open.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ToolWindows {
    pub library: bool,
    pub engine: bool,
    pub os_file: bool,
    pub keys_midi: bool,
}

impl ToolWindows {
    /// The flags in [`MENU_ITEMS`] order.
    pub fn flags(&mut self) -> [&mut bool; 4] {
        [
            &mut self.library,
            &mut self.engine,
            &mut self.os_file,
            &mut self.keys_midi,
        ]
    }
}

/// Whether the view shows `target` (`None` = the whole panel), for the button highlight.
pub fn is_current(view: &ViewState, target: Option<Section>) -> bool {
    view.panorama.is_none() && view.focus == target
}

/// Draws the bar into `ui`. Returns `true` when a button changed the view.
pub fn show_bar(ui: &mut Ui, tools: &mut ToolWindows, view: &mut ViewState) -> bool {
    let before = *view;
    ui.horizontal(|ui| {
        ui.menu_button("Menu", |ui| {
            for (flag, label) in tools.flags().into_iter().zip(MENU_ITEMS) {
                ui.checkbox(flag, label);
            }
        });
        ui.separator();
        for (target, label) in NAV {
            if ui
                .selectable_label(is_current(view, target), label)
                .clicked()
            {
                view.go(target);
            }
        }
    });
    *view != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_items_match_the_flags_and_start_closed() {
        let mut t = ToolWindows::default();
        assert_eq!(t.flags().len(), MENU_ITEMS.len());
        assert!(t.flags().iter().all(|f| !**f));
        *t.flags()[2] = true;
        assert!(t.os_file && !t.library && !t.engine && !t.keys_midi);
    }

    #[test]
    fn current_view_highlights_one_button() {
        let mut v = ViewState::default();
        let lit = |v: &ViewState| NAV.iter().filter(|(t, _)| is_current(v, *t)).count();
        assert!(is_current(&v, None));
        assert_eq!(lit(&v), 1);
        v.go(Some(Section::Synth));
        assert!(is_current(&v, Some(Section::Synth)) && !is_current(&v, None));
        assert_eq!(lit(&v), 1);
    }

    #[test]
    fn a_headless_frame_draws_the_bar_without_changing_the_view() {
        let ctx = egui::Context::default();
        let mut tools = ToolWindows::default();
        let mut view = ViewState::default();
        let mut changed = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::TopBottomPanel::top("nav").show(ctx, |ui| {
                changed = show_bar(ui, &mut tools, &mut view);
            });
        });
        assert!(!changed);
        assert_eq!(view, ViewState::default());
    }
}
