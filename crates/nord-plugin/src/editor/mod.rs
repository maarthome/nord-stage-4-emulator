//! The plugin's egui editor (Phase 6b B11, bean `ns4-hud1`).
//!
//! It draws the same front panel as the standalone app by calling the shared
//! `nord_gui::panel::render_stage4_panel`. The panel talks to the world through
//! `AudioCommand`s; here they are received on the GUI thread by a [`sink::ParamSink`], which
//! turns them into host parameter writes (automation-visible) and queues keybed MIDI for
//! `process()` through a lock-free [`notes`] queue. The editor never touches the engine.
//!
//! | File | Role |
//! |---|---|
//! | `draw.rs` | one frame: read-back, panel, dim overlay, sink pump; scaling |
//! | `sink.rs` | `PanelSink` trait and `ParamSink` |
//! | `map.rs`, `map_area.rs` | `AudioCommand` to `ParamWrite` mapping |
//! | `probe.rs` | one command per mapping arm; derives the mapped families |
//! | `coverage.rs` | live / passive / dimmed classification of the inventory |
//! | `menu_table.rs`, `map_settings.rs` | menu item <-> host parameter table, write side |
//! | `readback.rs`, `readback_menu.rs` | host parameter values to panel and menu state |
//! | `notes.rs` | bounded SPSC keybed queue and its engine dispatch |
//! | `index.rs`, `write.rs` | parameter lookup and the host writer |
//!
//! Spec: `docs/specs/daw-plugin.md`, guide: `docs/plugin-editor.md`.

pub mod coverage;
pub mod draw;
pub mod index;
pub mod map;
mod map_area;
mod map_fx;
mod map_pattern;
mod map_settings;
mod map_synth;
pub mod menu_table;
pub mod menu_ui_only;
pub mod notes;
pub mod probe;
pub mod readback;
pub mod readback_menu;
#[cfg(test)]
mod section_edit_tests;
pub mod sink;
pub mod write;

use crate::params::Stage4Params;
use draw::{draw_editor, EditorState};
use index::ParamIndex;
use nih_plug::prelude::Editor;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;
use write::GuiParamWriter;

/// Environment variable with the editor's start size, `WIDTHxHEIGHT` in logical px (e.g.
/// `2560x690`). Some window managers (X11 forwarding) do not pass a maximise on to the editor's
/// drawing surface, so this is the way to open it large.
pub const EDITOR_SIZE_ENV: &str = "NORD_EDITOR_SIZE";

/// Parses `WIDTHxHEIGHT` (also `X`, spaces allowed); each side 200..=16384.
pub fn parse_editor_size(v: &str) -> Option<(u32, u32)> {
    let (w, h) = v.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?);
    let ok = |n: u32| (200..=16384).contains(&n);
    (ok(w) && ok(h)).then_some((w, h))
}

/// Persistent editor window state with the default size, or the size in `NORD_EDITOR_SIZE`.
pub fn default_editor_state() -> Arc<EguiState> {
    let (w, h) = std::env::var(EDITOR_SIZE_ENV)
        .ok()
        .and_then(|v| parse_editor_size(&v))
        .unwrap_or(draw::DEFAULT_SIZE);
    EguiState::from_size(w, h)
}

/// Build the editor for `params`; keybed events go to `notes`.
pub fn create(params: Arc<Stage4Params>, notes: notes::NoteSender) -> Option<Box<dyn Editor>> {
    let egui_state = params.editor_state.clone();
    // The index holds raw parameter pointers; the state below keeps `params` alive for as long
    // as the editor (and therefore the index) exists.
    let index = Arc::new(ParamIndex::new(params.as_ref()));
    let keep_alive = params;
    let st = EditorState::new(index, notes);
    let size_state = egui_state.clone();
    create_egui_editor(
        egui_state,
        (st, keep_alive),
        |_ctx, _st| {},
        move |ctx, setter, st| {
            let index = st.0.index.clone();
            let writer = GuiParamWriter {
                ctx: setter.raw_context,
                index: &index,
            };
            draw_editor(ctx, &size_state, &mut st.0, writer);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::prelude::Params;

    #[test]
    fn editor_size_parses_and_rejects_nonsense() {
        assert_eq!(parse_editor_size("2560x690"), Some((2560, 690)));
        assert_eq!(parse_editor_size(" 1920 X 517 "), Some((1920, 517)));
        for bad in ["", "2560", "x690", "10x10", "2560x99999", "big"] {
            assert_eq!(parse_editor_size(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn default_editor_state_has_the_default_size() {
        assert_eq!(default_editor_state().size(), draw::DEFAULT_SIZE);
    }

    #[test]
    fn editor_size_round_trips_through_the_plugin_state() {
        let saved = Stage4Params {
            editor_state: EguiState::from_size(900, 243),
            ..Default::default()
        };
        let fields = saved.serialize_fields();
        assert!(fields.contains_key("editor-state"));

        let fresh = Stage4Params::default();
        assert_eq!(fresh.editor_state.size(), draw::DEFAULT_SIZE);
        fresh.deserialize_fields(&fields);
        assert_eq!(fresh.editor_state.size(), (900, 243));
    }

    #[test]
    fn create_returns_an_editor_for_default_params() {
        let params = Arc::new(Stage4Params::default());
        let (tx, _rx) = notes::note_queue(8);
        assert!(create(params, tx).is_some());
    }
}
