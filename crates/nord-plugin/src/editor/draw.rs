//! One editor frame: read-back, the shared front panel, dimmed-control overlay, sink pump.
//!
//! The panel is the same `render_stage4_panel` the standalone app draws (`nord-gui`), laid out
//! at its full logical size (`layout_px`, 3120 x 840 with `LAYOUT_SCALE` 2). It is drawn into its
//! own egui layer. The editor shows [`view_region`]: every control section from the Master Level
//! knob and Rotary box to the Effects section, without the keybed and the pitch stick / mod wheel
//! (a MIDI keyboard brings its own). That region is scaled and centred to fit the whole window with
//! `Context::set_transform_layer`; egui applies the transform to shapes before tessellation and to
//! pointer input (the mechanism of egui's own `Scene` container), letterboxed in the chassis colour.
//!
//! The editor does not use egui's zoom factor: `egui-baseview` (rev ec70c3f, the version
//! `nih_plug_egui` pins) tessellates every frame with the window's native `pixels_per_point`
//! (`src/renderer/opengl/renderer.rs:109`) and ignores the zoom. Text laid out at the zoomed
//! scale was drawn at the native one: an `epaint` "pixels_per_point changed" warning every frame
//! and a doubled, cropped panel (seen on a Raspberry Pi 5, standalone, X11). The zoom is kept at
//! 1, and so is the shared panel's own fit (`view::apply_zoom`, via `fixed_zoom`).

use super::coverage::{Coverage, DIM_TOOLTIP};
use super::index::ParamIndex;
use super::notes::NoteSender;
use super::readback::sync_panel_from_params;
use super::sink::{pump, ParamSink};
use super::write::ParamWriter;
use crossbeam_channel::Receiver;
use nih_plug_egui::egui::{
    self, emath::TSTransform, Color32, Context, Id, LayerId, Pos2, Rect, Sense, Stroke, StrokeKind,
    Ui, UiBuilder, Vec2,
};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::EguiState;
use nord_gui::audio::{AudioCommand, AudioHandle};
use nord_gui::panel::view::{focus_region, region_origin, ViewState};
use nord_gui::panel::{layout_px, render_stage4_panel, Stage4PanelState};
use nord_gui::style::CHASSIS_DARK;
use nord_panel::Section;
use std::sync::Arc;

/// Logical panel size the shared drawing code is laid out for (`nord_gui` layout px).
pub const PANEL_W: f32 = layout_px::PANEL_WIDTH_PX;
pub const PANEL_H: f32 = layout_px::PANEL_HEIGHT_PX;
/// Default editor window size (logical px), about the aspect of [`view_region`]; fits a laptop.
pub const DEFAULT_SIZE: (u32, u32) = (1560, 240);
/// Smallest scale the editor shrinks to.
pub const MIN_SCALE: f32 = 0.1;

/// Everything the editor keeps between frames (GUI thread only).
pub struct EditorState {
    pub panel: Stage4PanelState,
    pub handle: AudioHandle,
    pub rx: Receiver<AudioCommand>,
    pub index: Arc<ParamIndex>,
    pub coverage: Coverage,
    pub notes: NoteSender,
    /// Dim overlays painted in the last frame (for tests and diagnostics).
    pub last_overlays: usize,
    /// Panel commands dropped for lack of a host parameter.
    pub unmapped: u64,
    /// Scale of the panel layer in the last frame (diagnostics and tests).
    pub last_scale: f32,
}

impl EditorState {
    pub fn new(index: Arc<ParamIndex>, notes: NoteSender) -> Self {
        let (handle, rx) = AudioHandle::new();
        let coverage = super::coverage::classify(&index);
        Self {
            panel: Stage4PanelState::default(),
            handle,
            rx,
            index,
            coverage,
            notes,
            last_overlays: 0,
            unmapped: 0,
            last_scale: 0.0,
        }
    }
}

/// Sections shown whole (each with the panel's focus margin).
const SHOWN_SECTIONS: [Section; 5] = [
    Section::Organ,
    Section::Piano,
    Section::Program,
    Section::Synth,
    Section::Effects,
];
/// Master-area controls kept in view: level knob and the Rotary Speaker box (not the pitch stick
/// or mod wheel).
const SHOWN_MASTER_IDS: [&str; 6] = [
    "master.level",
    "rotary.drive",
    "rotary.organ",
    "rotary.stop_mode",
    "rotary.speed_leds",
    "rotary.speed",
];
/// Room around the master controls for their labels and the Rotary box frame (layout px).
/// ASSUMED: by eye from the panel drawing; widen if a label is clipped.
const MASTER_MARGIN: f32 = 24.0;

/// The part of the panel the editor shows, in panel layout px (panel origin at 0,0).
pub fn view_region() -> Rect {
    let sections = SHOWN_SECTIONS
        .into_iter()
        .filter_map(|s| focus_region(Some(s)));
    let master = SHOWN_MASTER_IDS
        .iter()
        .filter_map(|id| layout_px::rect_of(id, Pos2::ZERO))
        .map(|r| r.expand(MASTER_MARGIN));
    sections
        .chain(master)
        .fold(Rect::NOTHING, |a, r| a.union(r))
}

/// What the editor shows, in panel-layer px. A focused section (click its header, keys 1-5, Esc
/// back) or the panorama: the shared panel then draws itself shifted so that region starts at
/// 0,0. Otherwise the control strip [`view_region`].
pub fn shown_region(view: &ViewState) -> Rect {
    match view.region() {
        Some(r) => Rect::from_min_size(Pos2::ZERO, r.size()),
        None => view_region(),
    }
}

/// Scale that makes `region` fit `avail` (both logical px).
pub fn fit_scale(region: Vec2, avail: Vec2) -> f32 {
    (avail.x / region.x).min(avail.y / region.y).max(MIN_SCALE)
}

/// Scale that makes [`view_region`] fit a window of `size` logical px.
pub fn scale_for(size: (u32, u32)) -> f32 {
    fit_scale(
        view_region().size(),
        Vec2::new(size.0 as f32, size.1 as f32),
    )
}

/// Panel-layer → window transform: `region` scaled to fit `avail`, centred.
pub fn panel_transform(region: Rect, avail: Rect) -> TSTransform {
    let s = fit_scale(region.size(), avail.size());
    let offset = ((avail.size() - region.size() * s) * 0.5).max(Vec2::ZERO);
    TSTransform::new((avail.min + offset).to_vec2() - region.min.to_vec2() * s, s)
}

/// Translucent overlay with a tooltip over every dimmed control; returns how many were drawn.
///
/// The overlay also takes the pointer so a dimmed control cannot be clicked or dragged.
pub fn draw_dim_overlay(ui: &mut Ui, origin: egui::Pos2, coverage: &Coverage) -> usize {
    let mut n = 0;
    for id in coverage.dimmed_ids() {
        let Some(rect) = layout_px::rect_of(id, origin) else {
            continue;
        };
        ui.painter()
            .rect_filled(rect, 3.0_f32, Color32::from_black_alpha(150));
        ui.painter().rect_stroke(
            rect,
            3.0_f32,
            Stroke::new(1.0_f32, Color32::from_black_alpha(90)),
            StrokeKind::Inside,
        );
        ui.interact(rect, Id::new(("dimmed", id)), Sense::click_and_drag())
            .on_hover_text(DIM_TOOLTIP);
        n += 1;
    }
    n
}

/// Draw one frame of the editor and forward the panel's edits to `writer`.
pub fn draw_editor<W: ParamWriter>(
    ctx: &Context,
    egui_state: &EguiState,
    st: &mut EditorState,
    writer: W,
) {
    sync_panel_from_params(&st.index, &mut st.panel);

    // Keep the zoom at 1 (module docs): the renderer would ignore anything else. The shared panel
    // fits its own zoom every frame (`nord_gui::panel::view::apply_zoom`); a fixed zoom of one
    // physical px per native px makes it ask for exactly 1 too, so the two never alternate (which
    // forced a repaint every frame and kept the scale mismatch).
    let native = ctx.native_pixels_per_point().unwrap_or(1.0);
    st.panel.view.fixed_zoom = Some(native);
    if (ctx.zoom_factor() - 1.0).abs() > 1e-3 {
        ctx.set_zoom_factor(1.0);
    }

    ResizableWindow::new("nord-stage-4-editor")
        .min_size(Vec2::splat(8.0_f32))
        .show(ctx, egui_state, |ui| {
            // Fit to the whole window, not the saved editor size: a window maximised or resized
            // by the window manager (standalone) is bigger than `egui_state.size()`.
            let avail = ui.ctx().screen_rect();
            ui.ctx()
                .layer_painter(LayerId::background())
                .rect_filled(avail, 0.0_f32, CHASSIS_DARK);
            let region = shown_region(&st.panel.view);
            let to_global = panel_transform(region, avail);
            // The panel gets its own layer on top of the window's, as egui's `Scene` does.
            let layer = LayerId::new(ui.layer_id().order, ui.id().with("nord-panel-layer"));
            ui.ctx().set_sublayer(ui.layer_id(), layer);
            let panel_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(PANEL_W, PANEL_H));
            let mut local = ui.new_child(UiBuilder::new().layer_id(layer).max_rect(panel_rect));
            // Only the shown region: the rest (keybed, wheels, other sections) stays hidden and
            // unclickable.
            local.set_clip_rect(region.intersect(to_global.inverse() * avail));
            render_stage4_panel(&mut local, &mut st.panel, &st.handle);
            // The panel origin moves with a focused section (`ViewState::origin`).
            let origin = region_origin(st.panel.view.region(), Pos2::ZERO);
            st.last_overlays = draw_dim_overlay(&mut local, origin, &st.coverage);
            ui.ctx().set_transform_layer(layer, to_global);
            st.last_scale = to_global.scaling;
        });

    let mut sink = ParamSink::new(writer, &st.notes);
    pump(&st.rx, &mut sink);
    st.unmapped += sink.unmapped;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::notes::note_queue;
    use crate::editor::sink::test_support::Recorder;
    use crate::params::Stage4Params;

    fn state() -> (
        Stage4Params,
        EditorState,
        crate::editor::notes::NoteReceiver,
    ) {
        let params = Stage4Params::default();
        let index = Arc::new(ParamIndex::new(&params));
        let (tx, rx) = note_queue(64);
        (params, EditorState::new(index, tx), rx)
    }

    #[test]
    fn view_region_leaves_out_keybed_and_wheels_but_keeps_all_sections() {
        let r = view_region();
        assert!(r.is_positive() && r.width() > 3.0 * r.height());
        let keybed = focus_region(Some(Section::Keybed)).unwrap();
        assert!(r.max.y <= keybed.min.y + 6.0, "{r:?} vs keybed {keybed:?}");
        for id in ["master.pitch_stick", "master.mod_wheel"] {
            let w = layout_px::rect_of(id, Pos2::ZERO).unwrap();
            assert!(!r.intersects(w), "{id} {w:?} inside {r:?}");
        }
        for s in SHOWN_SECTIONS {
            assert!(r.contains_rect(focus_region(Some(s)).unwrap()), "{s:?}");
        }
        assert!(r.min.x >= 0.0 && r.max.x <= PANEL_W && r.min.y >= 0.0);
    }

    #[test]
    fn scale_fits_the_region_and_keeps_the_aspect() {
        let r = view_region();
        let exact = (r.width() as u32, r.height() as u32);
        assert!((scale_for(exact) - 1.0).abs() < 0.01);
        // Wide window: height limits the scale; tall window: width does.
        assert!((scale_for((exact.0 * 4, exact.1)) - 1.0).abs() < 0.01);
        assert!((scale_for((exact.0, exact.1 * 4)) - 1.0).abs() < 0.01);
        assert_eq!(scale_for((1, 1)), MIN_SCALE);
        // The default window shows the controls at a usable size.
        assert!(scale_for(DEFAULT_SIZE) > 0.5, "{}", scale_for(DEFAULT_SIZE));
    }

    #[test]
    fn a_focused_section_fills_the_window() {
        let mut view = ViewState::default();
        assert_eq!(shown_region(&view), view_region());
        view.toggle(Section::Effects);
        let fx = focus_region(Some(Section::Effects)).unwrap();
        let shown = shown_region(&view);
        assert_eq!(shown.min, Pos2::ZERO);
        assert!((shown.size() - fx.size()).length() < 1e-3);
        // Layer Effects is much taller for its width than the strip: it gets a larger scale.
        let window = Vec2::new(2540.0, 380.0);
        assert!(fit_scale(shown.size(), window) > fit_scale(view_region().size(), window));
        view.exit();
        assert_eq!(shown_region(&view), view_region());
    }

    #[test]
    fn panel_transform_centres_the_region_in_the_window() {
        let r = view_region();
        let avail = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(r.width() * 2.0, 4000.0));
        let t = panel_transform(r, avail);
        assert!((t.scaling - 2.0).abs() < 0.01);
        let (tl, br) = (t * r.min, t * r.max);
        assert!((tl.x - 10.0).abs() < 1.0 && (br.x - avail.max.x).abs() < 1.0);
        assert!(
            ((tl.y - avail.min.y) - (avail.max.y - br.y)).abs() < 1.0,
            "centred"
        );
    }

    #[test]
    fn frames_never_zoom_and_fit_the_real_window() {
        let (_params, mut st, _rx) = state();
        // The saved editor size is bigger than the window: the window wins.
        let egui_state = EguiState::from_size(1560, 420);
        let window = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(780.0, 210.0))),
            ..Default::default()
        };
        let ctx = Context::default();
        ctx.set_zoom_factor(0.5);
        let _ = ctx.run(window(), |ctx| {
            draw_editor(ctx, &egui_state, &mut st, Recorder::default());
        });
        assert_eq!(ctx.zoom_factor(), 1.0);
        assert_eq!(st.panel.view.fixed_zoom, Some(1.0));
        // A second frame must not move the zoom again (the panel used to re-fit it).
        let _ = ctx.run(window(), |ctx| {
            draw_editor(ctx, &egui_state, &mut st, Recorder::default());
        });
        assert_eq!(ctx.zoom_factor(), 1.0);
        assert!(
            (st.last_scale - 780.0 / PANEL_W).abs() < 1e-3,
            "{}",
            st.last_scale
        );
    }

    #[test]
    fn a_headless_frame_draws_the_panel_and_one_overlay_per_dimmed_control() {
        let (_params, mut st, _rx) = state();
        let egui_state = EguiState::from_size(PANEL_W as u32, PANEL_H as u32);
        let ctx = Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            draw_editor(ctx, &egui_state, &mut st, Recorder::default());
        });
        assert!(out.shapes.len() > 1000, "only {} shapes", out.shapes.len());
        assert_eq!(st.last_overlays, st.coverage.dimmed_ids().count());
        assert!(st.last_overlays > 0, "some controls must be dimmed today");
    }

    #[test]
    fn a_panel_edit_in_a_frame_reaches_the_writer_and_the_note_queue() {
        let (_params, mut st, notes) = state();
        st.handle.send_command(AudioCommand::SetMasterVolume(0.33));
        st.handle.send_midi(nord_core::midi::MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 99,
        });
        let egui_state = EguiState::from_size(PANEL_W as u32, PANEL_H as u32);
        let ctx = Context::default();
        let mut rec = Recorder::default();
        // Re-borrow the recorder through a wrapper so the test can inspect it afterwards.
        struct Shared<'a>(&'a mut Recorder);
        impl ParamWriter for Shared<'_> {
            fn write(&mut self, w: &crate::editor::write::ParamWrite) -> bool {
                self.0.write(w)
            }
        }
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw_editor(ctx, &egui_state, &mut st, Shared(&mut rec));
        });
        assert_eq!(rec.writes.len(), 1);
        assert_eq!(rec.writes[0].family, "master_level");
        assert!(notes.pop().is_some());
    }

    #[test]
    fn dim_overlay_blocks_nothing_when_nothing_is_dimmed() {
        let ctx = Context::default();
        let empty = Coverage { entries: vec![] };
        let mut n = usize::MAX;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                n = draw_dim_overlay(ui, egui::pos2(0.0_f32, 0.0_f32), &empty);
            });
        });
        assert_eq!(n, 0);
    }
}
