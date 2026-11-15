//! Program load -> Piano panel state: both layers' extras and the focused layer's shared buttons.
//!
//! The type, timbre, touch and acoustics buttons are one set on the panel and show the layer in
//! focus; ASSUMED (as for the Organ, `organ_layer.rs`) that a load focuses the highest enabled
//! layer. Layer levels and enables are the caller's (`apply_program`: they depend on the scene).

use super::prelude::*;
use nord_core::cbin::PianoPresetDecoded;

impl Stage4PanelState {
    /// Mirrors a decoded Piano block onto the panel.
    pub(super) fn mirror_piano(&mut self, p: &PianoPresetDecoded) {
        self.piano_a_level = p.layer_a.level;
        self.piano_b_level = p.layer_b.level;
        for (l, d) in [&p.layer_a, &p.layer_b].into_iter().enumerate() {
            self.piano_ui.soft_release[l] = d.soft_release;
            self.piano_ui.unison[l] = d.unison.min(3);
            self.piano_ui.dyn_comp[l] = d.dyn_comp.min(3);
            self.piano_ui.octave[l] = d.octave.clamp(-2, 2);
        }
        let focus = usize::from(self.piano_b_active);
        self.piano_ui.focus = focus;
        let d = if focus == 1 { &p.layer_b } else { &p.layer_a };
        self.piano_cat = d.category;
        self.piano_timbre = d.timbre;
        self.piano_touch = d.touch;
        self.string_res = d.string_res;
        self.pedal_noise = d.pedal_noise;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn felt_upright() -> PianoPresetDecoded {
        // `Felt Upright` (A:13) as decoded from the factory backup; the real panel (demo video,
        // 3:00) lights STRING RES, PED NOISE and KB TOUCH MED.
        let mut p = PianoPresetDecoded::default();
        p.layer_a.category = PianoCategory::Upright;
        p.layer_a.string_res = true;
        p.layer_a.pedal_noise = true;
        p.layer_a.level = 0.7;
        p
    }

    #[test]
    fn a_load_fills_the_shared_buttons_from_the_focused_layer() {
        let mut s = Stage4PanelState::default();
        s.mirror_piano(&felt_upright());
        assert_eq!(s.piano_cat, PianoCategory::Upright);
        assert!(s.string_res && s.pedal_noise);
        assert_eq!(s.piano_touch, VelocityTouch::Medium);
        assert_eq!((s.piano_ui.focus, s.piano_a_level), (0, 0.7));
    }

    #[test]
    fn layer_b_focus_when_only_b_is_on_and_extras_per_layer() {
        let mut p = felt_upright();
        p.layer_b.category = PianoCategory::Clav;
        p.layer_b.unison = 2;
        p.layer_b.dyn_comp = 9;
        p.layer_b.soft_release = true;
        p.layer_b.octave = -1;
        let mut s = Stage4PanelState {
            piano_b_active: true,
            ..Default::default()
        };
        s.mirror_piano(&p);
        assert_eq!(s.piano_ui.focus, 1);
        assert_eq!(s.piano_cat, PianoCategory::Clav);
        assert_eq!(s.piano_ui.unison, [0, 2]);
        assert_eq!(s.piano_ui.dyn_comp, [0, 3], "clamped to 3");
        assert_eq!(s.piano_ui.soft_release, [false, true]);
        assert_eq!(s.piano_ui.octave, [0, -1]);
    }

    #[test]
    fn acoustics_leds_are_for_the_acoustic_types() {
        use crate::panel::piano_logic::acoustic_leds;
        assert!(acoustic_leds(PianoCategory::Upright) && acoustic_leds(PianoCategory::Grand));
        assert!(
            !acoustic_leds(PianoCategory::Electric),
            "Classic Wurly, A:22"
        );
    }
}
