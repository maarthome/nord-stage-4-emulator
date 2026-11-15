//! Per-layer Organ panel state: the drawbar ladders, percussion LEDs, model and vibrato LEDs show
//! the layer in focus, and a program load fills both layers (`mirror_organ`).
//!
//! VERIFIED against the demo video (`NEW NORD STAGE 4 - PROGRAMAS PRESETS DEMO (PART A)`,
//! tests `a14_*` below): with both Organ layers on, the panel shows layer B (`B3 Chords n Solo`
//! A:14 at 3:56, `B3 Jazz Split` A:24, `B3 Praise Split` A:34); with only layer A on (`The Gospel
//! II`, A:77) it shows layer A. So a load focuses the highest enabled layer.

use super::prelude::*;
use super::ui_state::{OrganPerc, ALL_ZONES};
use nord_core::cbin::OrganPresetDecoded;
use nord_core::engine::zone_leds_for;

impl Stage4PanelState {
    /// Stored drawbar preset of an organ layer (what its ladders light).
    pub(super) fn organ_drawbars_of(&self, layer: usize) -> [u8; 9] {
        if layer == 0 {
            self.organ_drawbars
        } else {
            self.organ_ui.drawbars_b
        }
    }

    pub(super) fn set_organ_drawbar(&mut self, layer: usize, index: usize, value: u8) {
        let bars = if layer == 0 {
            &mut self.organ_drawbars
        } else {
            &mut self.organ_ui.drawbars_b
        };
        bars[index.min(8)] = value.min(8);
    }

    pub(super) fn set_organ_drawbars(&mut self, layer: usize, bars: [u8; 9]) {
        for (i, v) in bars.into_iter().enumerate() {
            self.set_organ_drawbar(layer, i, v);
        }
    }

    /// Percussion buttons of an organ layer.
    pub(super) fn organ_perc_of(&self, layer: usize) -> OrganPerc {
        if layer == 0 {
            OrganPerc {
                on: self.perc_on,
                soft: self.perc_soft,
                fast: self.perc_fast,
                third: self.perc_third,
            }
        } else {
            self.organ_ui.perc_b
        }
    }

    pub(super) fn set_organ_perc(&mut self, layer: usize, p: OrganPerc) {
        if layer == 0 {
            (self.perc_on, self.perc_soft) = (p.on, p.soft);
            (self.perc_fast, self.perc_third) = (p.fast, p.third);
        } else {
            self.organ_ui.perc_b = p;
        }
    }

    /// `(model, B3 Bass)` of an organ layer (the ORGAN MODEL LED).
    pub(super) fn organ_model_of(&self, layer: usize) -> (OrganModel, bool) {
        if layer == 0 {
            (self.organ_model, self.organ_ui.b3_bass)
        } else {
            (self.organ_ui.model_b, self.organ_ui.b3_bass_b)
        }
    }

    /// Mirrors a decoded Organ block onto the panel: both layers' presets, the vibrato and
    /// percussion LEDs, the rotary, and the focus (highest enabled layer). The layer ON flags
    /// (`organ_a_active`, `organ_b_active`) are the caller's: they depend on the scene.
    pub(super) fn mirror_organ(&mut self, o: &OrganPresetDecoded) {
        self.organ_model = o.model;
        self.organ_ui.b3_bass = o.b3_bass_a;
        self.organ_ui.model_b = o.model_b;
        self.organ_ui.b3_bass_b = o.b3_bass_b;
        self.organ_drawbars = o.drawbars_a;
        self.organ_ui.drawbars_b = o.drawbars_b;
        self.set_organ_perc(
            0,
            OrganPerc {
                on: o.percussion_on,
                soft: o.percussion_soft,
                fast: o.percussion_fast,
                third: o.percussion_third,
            },
        );
        self.set_organ_perc(
            1,
            OrganPerc {
                on: o.percussion_on_b,
                soft: o.percussion_soft_b,
                fast: o.percussion_fast_b,
                third: o.percussion_third_b,
            },
        );
        // One stored type for the section: both layers show it.
        let ty = o.scanner_mode(true);
        self.set_organ_vib(0, ty, o.vibrato_on);
        self.set_organ_vib(1, ty, o.vibrato_on_b);
        self.organ_octave = o.layer_a_octave.clamp(-2, 2);
        self.organ_ui.octave_b = o.layer_b_octave.clamp(-2, 2);
        self.organ_ui.rotary_organ = o.rotary_enabled;
        self.rotary_drive = (o.rotary_drive / 10.0).clamp(0.0, 1.0);
        self.rotary_stop = o.rotary_speed == RotarySpeed::Stop;
        self.rotary_speed = if o.rotary_speed == RotarySpeed::Fast {
            RotarySpeed::Fast
        } else {
            RotarySpeed::Slow
        };
        // A program is a stored preset: back to Preset mode.
        self.organ_ui.drawbar_live = false;
        self.organ_ui.focus_layer = usize::from(self.organ_b_active);
    }

    /// KB ZONE LEDs of the Organ layers from their stored `kKbZone` codes (the first two of the
    /// seven, Organ A, B, ...) under the program's split state.
    pub(super) fn mirror_organ_zones(&mut self, codes: &[u8; 7], active: [bool; 3]) {
        self.organ_ui.kb_zone = [
            zone_leds_for(codes[0], active),
            zone_leds_for(codes[1], active),
        ];
        debug_assert!(self
            .organ_ui
            .kb_zone
            .iter()
            .all(|m| *m != 0 && *m <= ALL_ZONES));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `B3 Chords n Solo` (A:14), layers as decoded from the factory backup (values read with
    /// `ns4 convert --to-json`; the real panel at 3:56 of the demo video shows layer B's).
    fn a14() -> OrganPresetDecoded {
        let mut o = OrganPresetDecoded::default();
        o.drawbars_a = [8, 0, 8, 0, 2, 0, 0, 0, 0];
        o.drawbars_b = [8, 8, 8, 6, 3, 3, 1, 5, 0];
        (o.percussion_on, o.percussion_soft) = (false, true);
        (o.percussion_fast, o.percussion_third) = (true, true);
        (o.percussion_on_b, o.percussion_soft_b) = (true, false);
        (o.percussion_fast_b, o.percussion_third_b) = (false, true);
        (o.vibrato_on, o.vibrato_on_b) = (true, true);
        (o.layer_a_octave, o.layer_b_octave) = (2, 0);
        o
    }

    fn loaded(a: bool, b: bool) -> Stage4PanelState {
        let mut s = Stage4PanelState {
            organ_a_active: a,
            organ_b_active: b,
            ..Default::default()
        };
        s.mirror_organ(&a14());
        s
    }

    #[test]
    fn a14_both_layers_on_focuses_layer_b_whose_bars_the_ladders_show() {
        let s = loaded(true, true);
        assert_eq!(s.organ_ui.focus_layer, 1);
        // Real panel: ladder LEDs lit 8 8 8 6 3 3 1 5 0 (video frame 3:56, counted from the top).
        assert_eq!(
            s.organ_drawbars_of(s.organ_ui.focus_layer),
            [8, 8, 8, 6, 3, 3, 1, 5, 0]
        );
        assert_eq!(s.organ_drawbars_of(0), [8, 0, 8, 0, 2, 0, 0, 0, 0]);
    }

    #[test]
    fn a14_percussion_leds_are_those_of_the_focused_layer() {
        let s = loaded(true, true);
        // Real panel: ON and 3RD lit, SOFT and FAST dark.
        let p = s.organ_perc_of(s.organ_ui.focus_layer);
        assert_eq!((p.on, p.soft, p.fast, p.third), (true, false, false, true));
        assert!(!s.organ_perc_of(0).on, "layer A stores ON off");
    }

    #[test]
    fn only_layer_a_on_focuses_layer_a() {
        let s = loaded(true, false);
        assert_eq!(
            s.organ_ui.focus_layer, 0,
            "The Gospel II (A:77) shows layer A"
        );
        assert!(s.organ_perc_of(0).soft);
    }

    #[test]
    fn vibrato_octave_rotary_follow_the_program() {
        let s = loaded(true, true);
        assert_eq!(s.organ_vib_of(0), (ScannerMode::C3, true));
        assert_eq!(s.organ_vib_of(1), (ScannerMode::C3, true));
        assert_eq!((s.organ_octave_of(0), s.organ_octave_of(1)), (2, 0));
        assert!(s.organ_ui.rotary_organ && !s.rotary_stop);
        let mut o = a14();
        o.rotary_speed = RotarySpeed::Stop;
        o.vibrato_on_b = false;
        let mut s = Stage4PanelState::default();
        s.mirror_organ(&o);
        assert!(s.rotary_stop);
        assert!(!s.organ_vib_of(1).1);
    }

    #[test]
    fn a_load_returns_to_preset_mode_but_leaves_the_physical_bars_alone() {
        let mut s = Stage4PanelState::default();
        s.organ_ui.drawbar_live = true;
        s.organ_ui.live_bars = [1, 2, 3, 4, 5, 6, 7, 8, 0];
        s.mirror_organ(&a14());
        assert!(!s.organ_ui.drawbar_live);
        assert_eq!(s.organ_ui.live_bars, [1, 2, 3, 4, 5, 6, 7, 8, 0]);
    }

    #[test]
    fn zone_codes_become_led_sets() {
        let mut s = Stage4PanelState::default();
        // A:14 stores codes A = 0, B = 6; A:34 the reverse. The real panel (layer B focused)
        // lights zones 3+4 for A:14 and zones 1+2 for A:34.
        let mid = [false, true, false];
        s.mirror_organ_zones(&[0, 6, 5, 5, 5, 5, 5], mid);
        assert_eq!(s.organ_ui.kb_zone[1], 0b1100);
        s.mirror_organ_zones(&[6, 0, 5, 5, 5, 5, 5], mid);
        assert_eq!(s.organ_ui.kb_zone[1], 0b0011);
        s.mirror_organ_zones(&[5, 5, 5, 5, 5, 5, 5], [false; 3]);
        assert_eq!(s.organ_ui.kb_zone, [ALL_ZONES; 2]);
    }

    #[test]
    fn per_layer_setters_clamp_and_split_a_from_b() {
        let mut s = Stage4PanelState::default();
        s.set_organ_drawbar(1, 3, 200);
        assert_eq!(s.organ_drawbars_of(1)[3], 8);
        assert_eq!(s.organ_drawbars_of(0)[3], 0);
        s.set_organ_drawbars(0, [1; 9]);
        assert_eq!(s.organ_drawbars, [1; 9]);
        assert_eq!(s.organ_model_of(1), (OrganModel::B3, false));
    }
}
