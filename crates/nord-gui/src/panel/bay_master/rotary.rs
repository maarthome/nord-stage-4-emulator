//! The Rotary Speaker box of the Master bay (manual p.53): DRIVE, ORGAN (+ CLOSE MIC on Shift),
//! STOP MODE (+ ANGLE on Shift) and the SLOW/STOP - FAST button with its two LEDs.
//!
//! The control rectangles come from `nord_panel::layout`; the frame and the printed legends are
//! laid out here. The buttons are only ~37 px wide, so the frame grows leftwards into the empty
//! Master area (the Organ section starts right of it) and the wide legends are centred on the
//! frame, not on the button.

use super::super::ctl::{gesture_button, mark, Face};
use super::super::ctl_led_tag::tag_led;
use super::super::ctl_legend::shift_legend;
use super::super::layout_px::rect_or_empty;
use super::super::prelude::*;
use crate::audio::OrganEdit;
use nord_panel::behaviour::Trigger;

/// Frame extent left of the button column centre (px). The buttons sit on the frame's vertical
/// centre line (`layout_tweaks`: every `rotary.` control moves 11 px left).
const FRAME_LEFT: f32 = 40.0;
/// Frame extent right of the button column centre (px), capped by the section edge.
const FRAME_RIGHT: f32 = 41.0;
/// Free space kept between the frame and the Master / Organ section boundary (px).
const SECTION_GAP: f32 = 3.0;
/// Distance from the bottom of the DRIVE knob rectangle to its name (px): below the `0` and `10`.
const DRIVE_NAME_DROP: f32 = 9.5;
/// Space from the frame top to the DRIVE knob top (room for title and the DRIVE name).
const TITLE_ROOM: f32 = 34.0;

pub(super) fn send_rotary(state: &Stage4PanelState, audio: &AudioHandle) {
    audio.send_command(AudioCommand::Organ(OrganEdit::Rotary {
        fast: matches!(state.rotary_speed, RotarySpeed::Fast),
        stop_mode: state.rotary_stop,
    }));
}

/// Rectangle of the Rotary Speaker group frame for a panel at `o`.
pub(super) fn frame_rect(o: egui::Pos2) -> Rect {
    let (top, bottom) = (
        rect_or_empty("rotary.drive", o),
        rect_or_empty("rotary.speed", o),
    );
    let cx = rect_or_empty("rotary.organ", o).center().x;
    let section = super::super::view::section_px(nord_panel::Section::Master, o);
    let right = (cx + FRAME_RIGHT).min(section.right() - SECTION_GAP);
    Rect::from_min_max(
        pos2(cx - FRAME_LEFT, top.top() - TITLE_ROOM),
        pos2(right, bottom.bottom() + 12.0),
    )
}

/// Height of the light tab behind the title (px).
const TAB_H: f32 = 13.0;

/// The light tab at the top of the box with the title in dark print inside it.
/// VERIFIED: the real panel prints `ROTARY SPEAKER` on a light blue-grey plate that spans the box
/// top with rounded upper corners (photo `NS4_Panel-crop-left.jpg`, local reference).
fn paint_tab(painter: &Painter, frame: Rect) {
    let tab = Rect::from_min_size(frame.min, vec2(frame.width(), TAB_H));
    let round = egui::CornerRadius {
        nw: 4,
        ne: 4,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(tab, round, crate::style::MODULE_HEADER_BAR);
    label_fit(
        painter,
        tab.center(),
        "ROTARY SPEAKER",
        5.0,
        tab.width() - 6.0,
        Color32::from_rgb(20, 24, 30),
        Align2::CENTER_CENTER,
    );
}

/// Draw the Rotary Speaker box and apply its input.
pub(super) fn render_rotary(
    ui: &mut Ui,
    painter: &Painter,
    o: egui::Pos2,
    state: &mut Stage4PanelState,
    audio: &AudioHandle,
) {
    let frame = frame_rect(o);
    #[cfg(test)]
    crate::widgets::probe::record(
        ui.ctx(),
        crate::widgets::probe::Kind::GroupInside,
        frame,
        "ROTARY SPEAKER",
    );
    painter.rect_filled(frame, 4.0, Color32::from_rgb(110, 20, 24));
    painter.rect_stroke(
        frame,
        4.0,
        Stroke::new(1.0_f32, GROUP_BORDER),
        StrokeKind::Inside,
    );
    paint_tab(painter, frame);

    let drive = rect_or_empty("rotary.drive", o);
    if knob(
        ui,
        drive.center(),
        drive.width().min(drive.height()) * 0.42,
        "rotary.drive",
        &mut state.rotary_drive,
        0.0,
        1.0,
        "",
    ) {
        audio.send_command(AudioCommand::SetRotaryDrive(state.rotary_drive));
    }
    mark(ui, "rotary.drive");
    // Printed below the knob, under the numerals around its ring (photo `NS4-Panel-organ`).
    label(
        painter,
        pos2(drive.center().x, drive.bottom() + DRIVE_NAME_DROP),
        "DRIVE",
        6.0,
        Color32::from_gray(235),
        Align2::CENTER_CENTER,
    );
    render_rotary_buttons(ui, painter, o, frame, state, audio);
}

fn render_rotary_buttons(
    ui: &mut Ui,
    painter: &Painter,
    o: egui::Pos2,
    frame: Rect,
    state: &mut Stage4PanelState,
    audio: &AudioHandle,
) {
    let rc = |id: &str| rect_or_empty(id, o);

    // ORGAN (route the organ through the rotary speaker) with CLOSE MIC on Shift.
    let organ = rc("rotary.organ");
    let face = Face {
        tip: "ORGAN: route the Organ through the rotary speaker. Shift+tap: CLOSE MIC",
        ..Face::black()
    };
    match gesture_button(
        ui,
        &mut state.organ_ui.trackers,
        organ,
        "rotary.organ",
        true,
        face,
    ) {
        Some(Trigger::Primary) => {
            state.organ_ui.rotary_organ = !state.organ_ui.rotary_organ;
            audio.send_command(AudioCommand::Organ(OrganEdit::RotaryOrgan(
                state.organ_ui.rotary_organ,
            )));
        }
        Some(Trigger::Shifted) => {
            state.organ_ui.close_mic = !state.organ_ui.close_mic;
            audio.send_command(AudioCommand::Organ(OrganEdit::CloseMic(
                state.organ_ui.close_mic,
            )));
        }
        _ => {}
    }
    tag_led(painter, organ, "ORGAN", state.organ_ui.rotary_organ);
    shift_legend(
        ui,
        organ,
        "rotary.close_mic",
        "CLOSE MIC",
        Some(state.organ_ui.close_mic),
        None,
    );

    // STOP MODE with the stop ANGLE picker (and the mic angle toggle) on Shift.
    let stop = rc("rotary.stop_mode");
    let face = Face {
        tip: "STOP MODE: the slow position becomes the brake",
        ..Face::black()
    };
    match gesture_button(
        ui,
        &mut state.organ_ui.trackers,
        stop,
        "rotary.stop_mode",
        true,
        face,
    ) {
        Some(Trigger::Primary) => {
            state.rotary_stop = !state.rotary_stop;
            send_rotary(state, audio);
        }
        Some(Trigger::Shifted) => {
            state.organ_ui.stop_angle_open = !state.organ_ui.stop_angle_open;
        }
        _ => {}
    }
    tag_led(painter, stop, "STOP MODE", state.rotary_stop);
    shift_legend(
        ui,
        stop,
        "rotary.stop_angle",
        "ANGLE",
        Some(state.organ_ui.rotary_stop_angle != 0),
        None,
    );
    // The mic separation toggle is a row of the popup; the id is painted with the legend.
    mark(ui, "rotary.angle");
    super::super::rotor_angle::draw(ui, painter, stop, state, audio);
    super::rotary_speed::render_speed(ui, painter, o, frame, state, audio);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotary_command_follows_speed_and_stop_state() {
        let (audio, rx) = AudioHandle::new();
        let mut s = Stage4PanelState {
            rotary_speed: RotarySpeed::Fast,
            rotary_stop: true,
            ..Default::default()
        };
        send_rotary(&s, &audio);
        assert!(matches!(
            rx.try_recv(),
            Ok(AudioCommand::Organ(OrganEdit::Rotary {
                fast: true,
                stop_mode: true
            }))
        ));
        s.rotary_stop = false;
        send_rotary(&s, &audio);
        assert!(matches!(
            rx.try_recv(),
            Ok(AudioCommand::Organ(OrganEdit::Rotary {
                stop_mode: false,
                ..
            }))
        ));
    }

    #[test]
    fn tab_is_painted_inside_the_frame_top() {
        let o = pos2(0.0, 0.0);
        let frame = frame_rect(o);
        let n = crate::panel::support::shapes_painted(|_, p, _, _, _| paint_tab(p, frame));
        assert!(n >= 2, "tab painted {n} shapes");
        assert!(
            frame.height() > TAB_H * 4.0,
            "tab leaves room for the controls"
        );
    }

    #[test]
    fn frame_is_wide_enough_for_its_labels_and_stays_in_the_master_section() {
        let o = pos2(0.0, 0.0);
        let f = frame_rect(o);
        let sec = super::super::super::view::section_px(nord_panel::Section::Master, o);
        assert!(f.width() >= 70.0, "frame {f:?}");
        assert!(sec.contains_rect(f), "frame {f:?} leaves {sec:?}");
        let speed = rect_or_empty("rotary.speed", o);
        assert!(f.contains_rect(speed.expand(2.0)));
    }
}
