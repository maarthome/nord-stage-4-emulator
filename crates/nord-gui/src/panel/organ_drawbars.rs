//! Organ drawbars (manual p.19-21): nine drawbars with their 8-LED ladders.
//!
//! The ladder shows the stored preset value and is unlit in Drawbar Live (manual p.21) and while the
//! Organ section is off (D78). The
//! sliders are the physical drawbars: in Preset mode they edit the preset, in Drawbar Live only
//! the physical positions (`OrganEdit::LiveDrawbars`).

use super::layout_px::rect_or_empty;
use super::organ_widgets::{drawbar_ladder, drawbar_slot};
use super::prelude::*;
use crate::audio::OrganEdit;

const SLOT_IDS: [&str; 9] = [
    "organ.drawbar.1",
    "organ.drawbar.2",
    "organ.drawbar.3",
    "organ.drawbar.4",
    "organ.drawbar.5",
    "organ.drawbar.6",
    "organ.drawbar.7",
    "organ.drawbar.8",
    "organ.drawbar.9",
];

const LADDER_IDS: [&str; 9] = [
    "organ.drawbar_ladder.1",
    "organ.drawbar_ladder.2",
    "organ.drawbar_ladder.3",
    "organ.drawbar_ladder.4",
    "organ.drawbar_ladder.5",
    "organ.drawbar_ladder.6",
    "organ.drawbar_ladder.7",
    "organ.drawbar_ladder.8",
    "organ.drawbar_ladder.9",
];

/// Distance (px) from the slot end to the footage number: clears the cap at the pushed-out end
/// (half cap 4.5 px plus its shadow, audit rule `F`) and leaves the glyphs a 2 px margin.
const FOOTAGE_DROP: f32 = 13.0;

/// Footages 16', 5 1/3', 8', 4', 2 2/3', 2', 1 3/5', 1 1/3', 1'.
const FOOTAGES: [&str; 9] = [
    "16'", "5 1/3'", "8'", "4'", "2 2/3'", "2'", "1 3/5'", "1 1/3'", "1'",
];

/// Register names printed over the ladders (manual p.19: the original register of each drawbar).
const NAMES: [&str; 9] = [
    "BASS16", "STR16", "FLUTE8", "OBOE8", "TRMP8", "STR8", "FLUTE4", "STR4", "2 2/3",
];

/// Distance (px) from the ladder top to the centre of its register name: clears the control
/// margin of the legibility audit (2.5 px) and the half height of the text.
const NAME_RISE: f32 = 7.0;
/// Distance (px) of a ladder numeral from the LED column: past the LED bezel (5.2 px at the
/// ladder pitch) and inside the ladder rectangle, so the numeral belongs to the ladder.
const NUMERAL_DX: f32 = 8.3;

/// Register name over the ladder and the numerals 1-8 beside its LEDs (photo
/// `NS4_HA88_Organ-01`: numbers beside each ladder, the name above it).
fn ladder_labels(p: &Painter, ladder: Rect, name: &str) {
    let ink = Color32::from_gray(215);
    label(
        p,
        pos2(ladder.center().x, ladder.top() - NAME_RISE),
        name,
        7.5,
        ink,
        Align2::CENTER_CENTER,
    );
    let pitch = ladder.height() / 8.0;
    for n in 1..=8 {
        label(
            p,
            pos2(
                ladder.center().x + NUMERAL_DX,
                ladder.top() + (n as f32 - 0.5) * pitch,
            ),
            &n.to_string(),
            7.5,
            ink,
            Align2::CENTER_CENTER,
        );
    }
}

/// Drawbars 3, 4, 6 and 9 (8', 4', 2', 1') have white caps, the others black.
fn is_white(i: usize) -> bool {
    matches!(i, 2 | 3 | 5 | 8)
}

/// Sends the stored preset of layer `l` to the engine.
pub(super) fn send_drawbars(state: &Stage4PanelState, audio: &AudioHandle, l: usize) {
    let bars = state.organ_drawbars_of(l);
    audio.send_command(if l == 0 {
        AudioCommand::SetDrawbars(bars)
    } else {
        AudioCommand::Organ(OrganEdit::Drawbars {
            layer: l as u8,
            bars,
        })
    });
}

/// LEDs lit on a ladder: the stored value; none in Drawbar Live and none while the Organ section
/// is off.
///
/// EMPIRICAL: the hardware frame of `A:13 Felt Upright` (Part A demo clip, 2:32; a piano-only
/// program, Organ section off) shows no lit ladder LED although the drawbars stand at the stored
/// positions. Compared by eye against the GUI screenshot of the same program (all ladders lit);
/// the ladders of an organ program in the clip are not visible, so the other LEDs of a switched
/// off section are ASSUMED unchanged.
pub(super) fn ladder_value(value: u8, live: bool, section_on: bool) -> u8 {
    if live || !section_on {
        0
    } else {
        value.min(8)
    }
}

pub(super) fn render_drawbars(
    ui: &mut Ui,
    painter: &Painter,
    origin: egui::Pos2,
    state: &mut Stage4PanelState,
    audio: &AudioHandle,
) {
    let live = state.organ_ui.drawbar_live;
    let focus = state.organ_ui.focus_layer;
    // The ladders show the stored preset of the focused layer. The sliders are physical: they stay
    // where they are on a program load (demo video: every cap sits at the bottom on every program).
    let before = state.organ_ui.live_bars;
    let mut changed = false;
    for i in 0..9 {
        let slot = rect_or_empty(SLOT_IDS[i], origin);
        if drawbar_slot(
            ui,
            slot,
            SLOT_IDS[i],
            &mut state.organ_ui.live_bars[i],
            is_white(i),
        ) {
            changed = true;
        }
        let shown = state
            .telemetry
            .drawbar(focus == 1, i, state.organ_drawbars_of(focus)[i]);
        let lit = ladder_value(shown, live, state.organ_section_on());
        let ladder = rect_or_empty(LADDER_IDS[i], origin);
        drawbar_ladder(ui, ladder, LADDER_IDS[i], lit);
        ladder_labels(painter, ladder, NAMES[i]);
        label(
            painter,
            pos2(slot.center().x + 4.0, slot.bottom() + FOOTAGE_DROP),
            FOOTAGES[i],
            5.5,
            Color32::WHITE,
            Align2::CENTER_CENTER,
        );
    }
    if changed {
        if !live {
            // Preset mode: a moved bar writes its value into the focused layer's preset (every
            // layer under SECTION EDIT).
            let moved = (0..9).find(|&i| state.organ_ui.live_bars[i] != before[i]);
            for l in state.edit_layers(focus, 2) {
                if let Some(i) = moved {
                    state.set_organ_drawbar(l, i, state.organ_ui.live_bars[i]);
                }
                send_drawbars(state, audio, l);
            }
        }
        audio.send_command(AudioCommand::Organ(OrganEdit::LiveDrawbars(
            state.organ_ui.live_bars,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::support::shapes_painted;

    #[test]
    fn drawbars_paint_shapes() {
        let n = shapes_painted(|ui, p, rect, state, audio| {
            render_drawbars(ui, p, rect.min, state, audio);
        });
        assert!(n > 100, "drawbars painted only {n} shapes");
    }

    #[test]
    fn ladder_is_unlit_in_drawbar_live() {
        assert_eq!(ladder_value(5, false, true), 5);
        assert_eq!(ladder_value(5, true, true), 0);
        assert_eq!(ladder_value(200, false, true), 8);
    }

    #[test]
    fn ladder_is_unlit_while_the_organ_section_is_off() {
        assert_eq!(ladder_value(8, false, false), 0);
        assert_eq!(ladder_value(8, true, false), 0);
    }

    #[test]
    fn registers_are_named_like_the_manual_and_footages_match() {
        assert_eq!(NAMES.len(), FOOTAGES.len());
        assert_eq!(NAMES[0], "BASS16");
        assert_eq!(NAMES[8], "2 2/3");
    }

    #[test]
    fn every_ladder_prints_its_name_and_eight_numerals() {
        let (labs, _) = crate::panel::legibility_tests::labels_of(None, egui::vec2(3300.0, 900.0));
        for name in NAMES {
            assert!(labs.iter().any(|l| l.text == name), "{name}");
        }
        for n in 1..=8 {
            let count = labs.iter().filter(|l| l.text == n.to_string()).count();
            assert!(count >= 9, "numeral {n}: {count}");
        }
    }

    #[test]
    fn cap_colours_follow_the_footage() {
        let white: Vec<_> = (0..9).filter(|&i| is_white(i)).collect();
        assert_eq!(white, vec![2, 3, 5, 8]);
    }
}
