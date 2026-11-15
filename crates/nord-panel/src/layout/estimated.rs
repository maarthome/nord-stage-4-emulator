//! Entries whose rectangle is a guess rather than a measurement.
//!
//! ASSUMED: the photo (2000 px wide, about 0.69 mm per pixel) is too small
//! to read these reliably: single LEDs a few pixels across, LED groups
//! drawn as one rectangle, and functions whose physical control is unclear.
//! A higher-resolution photo, or a measurement on the instrument, confirms
//! or replaces each one. Everything not listed here is placed from visible
//! control outlines (about +-3 px).
//!
//! VERIFIED against the real-panel crops (and so removed from the list): the Master / Rotary,
//! Organ and Piano LEDs, see `docs/specs/control-inventory.md` section 7 (photo anchors in
//! `crates/nord-panel/data/photo_anchors_{master,organ,piano}.csv`).
//!
//! Shared entries ([`super::SHARED`]) inherit the accuracy of their base;
//! the "(direct)" rows there are listed here as well.

/// Ids with an estimated rectangle.
pub const ESTIMATED: &[&str] = &[
    // Layer LEDs, 4-6 px wide in the photo.
    // organ.aux_led.a/b: centre from the crop (isotropic mapping); the 4 px size is a guess.
    "organ.aux_led.a",
    "organ.aux_led.b",
    "synth.aux_led.a",
    "synth.aux_led.b",
    "synth.aux_led.c",
    "synth.focus_led.a",
    "synth.focus_led.b",
    "synth.focus_led.c",
    "synth.fx_focus_led",
    "synth.zone_leds",
    // LEDs and LED groups placed near their control.
    "fx.focus.leds",
    // One photo button, several functions (halves or shared rectangles).
    "amp.filter",
    "amp.res",
    "amp.to_rotary",
    "organ.vibrato_on.a",
    "organ.vibrato_on.b",
    "piano.ped_noise",
    "piano.string_res",
    "synth.voice.priority",
    // Control found in the photo but its function mapping is unclear.
    "synth.dial_list",
    "synth.filter.res",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_unique_and_pinned() {
        let mut seen = std::collections::HashSet::new();
        for id in ESTIMATED {
            assert!(seen.insert(*id), "duplicate {id}");
        }
        assert_eq!(ESTIMATED.len(), 21);
    }
}
