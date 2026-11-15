//! Pitch stick bend range (PSTICK/RNG), per layer, in semitones.
//!
//! Manual OS v1.6x p.28: the Synth layers' bend range "can be set in semitone steps" by holding
//! PSTICK/RNG (Shift+Layer B) and picking a value from the displayed list. Organ (p.18) and Piano
//! (p.23) have no range list: PSTICK bends them "2 semitones up and down". Spec:
//! `docs/specs/midi-and-control.md` section 2a.

use super::*;

/// Default bend range of every layer.
/// VERIFIED: "bent 2 semitones up and down" = manual p.18 (Organ) and p.23 (Piano), checked by
/// `organ_and_piano_ignore_the_range_setting`; the Synth default is VERIFIED to match: the firmware
/// default index 1 of [`PSTICK_RANGE_LIST`] is `2 semi`.
pub const PSTICK_DEFAULT_RANGE: u8 = 2;
/// VERIFIED: the Pitch Stick Range choices in menu order, `Edt_Syn_kPitchStickRange` `0..=9`
/// (default index 1 = `2 semi`, matching the manual's "2 semitones"): the display strings
/// `1 semi` .. `-24 semi` are contiguous at `bank0_CODE.bin@0x2D3479`, the label `Pitch Stick
/// Range` follows at `0x2D34D8`, and the edit id range `0..9` (`docs/research/arm-edit-params.csv`)
/// has ten entries. Checked by `tests::range_list_has_the_ten_firmware_choices` and
/// `python tools/ns4_fw_choices.py target/fw_unpacked/bank0_CODE.bin`. The two negative entries
/// bend the opposite way (an inverted stick); the engine models only the positive ones, and the
/// hardware cannot select 6, 8, 9 or 11 semitones (the panel steps the list, `docs/research/assumed-constants-audit.md`).
pub const PSTICK_RANGE_LIST: [i8; 10] = [1, 2, 3, 4, 5, 7, 10, 12, -12, -24];
/// Smallest selectable range: one semitone.
/// VERIFIED: first entry of [`PSTICK_RANGE_LIST`].
pub const PSTICK_MIN_RANGE: u8 = 1;
/// Largest positive selectable range.
/// VERIFIED: last positive entry of [`PSTICK_RANGE_LIST`] (`12 semi`).
pub const PSTICK_MAX_RANGE: u8 = 12;

impl Stage4Engine {
    /// Sets the bend range of `layer` (clamped to `1..=12` semitones) and re-derives the bend.
    /// Only Synth layers honour it ([`Self::pstick_range_semitones`]). Allocation-free.
    pub fn set_pstick_range(&mut self, layer: LayerId, semitones: u8) {
        self.pedals.pstick_range[layer as usize] =
            semitones.clamp(PSTICK_MIN_RANGE, PSTICK_MAX_RANGE);
        self.refresh_pedal_routing();
    }

    /// Bend range in effect for `layer`: the PSTICK/RNG setting on Synth layers, a fixed
    /// [`PSTICK_DEFAULT_RANGE`] on Organ and Piano layers (no range list there, p.18, p.23).
    pub fn pstick_range_semitones(&self, layer: LayerId) -> u8 {
        match layer {
            LayerId::SynthA | LayerId::SynthB | LayerId::SynthC => {
                self.pedals.pstick_range[layer as usize].clamp(PSTICK_MIN_RANGE, PSTICK_MAX_RANGE)
            }
            _ => PSTICK_DEFAULT_RANGE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bend_up(e: &mut Stage4Engine) {
        e.handle_midi(MidiEvent::PitchBend {
            channel: 0,
            value: 16383,
        });
    }

    #[test]
    fn range_list_has_the_ten_firmware_choices() {
        assert_eq!(PSTICK_RANGE_LIST.len(), 10, "edit id range 0..=9");
        assert_eq!(PSTICK_RANGE_LIST[1] as u8, PSTICK_DEFAULT_RANGE, "default index 1");
        let positive: Vec<i8> = PSTICK_RANGE_LIST.iter().copied().filter(|r| *r > 0).collect();
        assert_eq!(positive.first().copied(), Some(PSTICK_MIN_RANGE as i8));
        assert_eq!(positive.last().copied(), Some(PSTICK_MAX_RANGE as i8));
        assert!(positive.windows(2).all(|w| w[0] < w[1]), "ascending");
    }

    #[test]
    fn setter_clamps_to_one_to_twelve_semitones() {
        let mut e = Stage4Engine::new(48_000.0);
        e.set_pstick_range(LayerId::SynthA, 0);
        assert_eq!(e.pedals.pstick_range[4], 1);
        e.set_pstick_range(LayerId::SynthA, 200);
        assert_eq!(e.pedals.pstick_range[4], 12);
        e.set_pstick_range(LayerId::SynthA, 7);
        assert_eq!(e.pstick_range_semitones(LayerId::SynthA), 7);
    }

    #[test]
    fn synth_bend_scales_with_the_range_and_is_per_layer() {
        let mut e = Stage4Engine::new(48_000.0);
        e.pedals.pstick[4] = true;
        e.pedals.pstick[5] = true;
        e.set_pstick_range(LayerId::SynthA, 5);
        bend_up(&mut e);
        assert!((e.synth_a.bend_cents - 500.0).abs() < 0.1);
        assert!((e.synth_b.bend_cents - 200.0).abs() < 0.1, "B keeps 2");
        e.set_pstick_range(LayerId::SynthA, 12);
        assert!(
            (e.synth_a.bend_cents - 1200.0).abs() < 0.5,
            "re-derived at once"
        );
    }

    #[test]
    fn organ_and_piano_ignore_the_range_setting() {
        let mut e = Stage4Engine::new(48_000.0);
        for l in [LayerId::OrganA, LayerId::PianoA] {
            e.pedals.pstick[l as usize] = true;
            e.set_pstick_range(l, 12);
        }
        bend_up(&mut e);
        assert!((e.organ_a.bend_cents() - 200.0).abs() < 0.1);
        assert!((e.piano_a.bend_cents() - 200.0).abs() < 0.1);
    }

    #[test]
    fn an_out_of_range_field_write_is_clamped_when_applied() {
        let mut e = Stage4Engine::new(48_000.0);
        e.pedals.pstick[4] = true;
        e.pedals.pstick_range[4] = 0;
        bend_up(&mut e);
        e.refresh_pedal_routing();
        assert!((e.synth_a.bend_cents - 100.0).abs() < 0.1);
        e.pedals.pstick_range[4] = 255;
        e.refresh_pedal_routing();
        assert!((e.synth_a.bend_cents - 1200.0).abs() < 0.5);
    }
}
