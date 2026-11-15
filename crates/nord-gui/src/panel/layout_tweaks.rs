//! Per-control position nudges applied on top of the `nord_panel::layout` tables.
//!
//! The layout rectangles are `ASSUMED` (+-3 px estimated from one 2000 px photo), so they are
//! not edited. Where two neighbours print their text into each other at the panel scale, the
//! control is nudged here instead, by `(dx, dy)` logical px, with the reason next to the entry.
//! `layout_px::rect_of` applies the table; `legibility_tests` proves the result is collision free.

use egui::{vec2, Vec2};

/// `(control id, dx, dy)`: shift of the control's rectangle in logical px.
pub const TWEAKS: &[(&str, f32, f32)] = &[
    // Real-panel positions (isotropic crop mapping) leave 7.5 px legends touching their
    // neighbours: these nudges stay within 2.5 mm of the crop.
    ("synth.zone_leds", 0.0, 3.0), // the KB ZONE legends sit between the buttons and the LEDs
    ("synth.lfo.waveform", 0.0, -2.25), // clears the GROUP legend from the RATE numerals
    ("synth.lfo.rate", 0.0, 2.5),  // the RATE numerals touched the LFO GROUP legend above
    ("synth.lfo.rate_mst_clk", 0.0, 2.5),
    ("synth.osc.ctrl", 0.0, 1.0), // the OSC CTRL numerals touched the ENV TO PITCH legend
    ("synth.filter.freq", 0.0, 1.0), // the FREQ numerals touched the GROUP legend
    // Program: the PROG VIEW legend touched PANIC above; buttons 5-8 / MON-COPY legends touched the
    // names above them; the SPLIT legend touched the LED strip (all <= 1.7 mm).
    ("program.prog_view", 0.0, 4.0),
    ("program.preset_name", 0.0, 4.0),
    ("program.button.5", 0.0, 3.0),
    ("program.button.6", 0.0, 3.0),
    ("program.button.7", 0.0, 3.0),
    ("program.button.8", 0.0, 3.0),
    ("program.menu.output", 0.0, 3.0),
    ("program.menu.pedal", 0.0, 3.0),
    ("program.menu.midi", 0.0, 3.0),
    ("program.menu.extern", 0.0, 3.0),
    ("program.mon_copy", 0.0, 3.0),
    ("program.paste", 0.0, 3.0),
    ("split.leds", 0.0, -2.0),
    ("split.on_set", 0.0, 1.5), // the ON/SET legend clears the LED strip under the SPLIT title
    // The Mod 1 numerals need the room the LAYER FX button legend used (round 4).
    ("fx.layer_effects_on", 0.0, -10.0),
    // The "VAR" legend under the amp model button printed into the TREBLE numeral `0`.
    ("amp.model", 0.0, -12.0),
    ("amp.variation", 0.0, -12.0),
    ("amp.filter", 0.0, -12.0),
    ("amp.to_rotary", 0.0, -12.0),
    // The variation lists need the width between the AMOUNT numerals and the ON button.
    ("mod1.selector", 8.0, 0.0),
    ("mod1.variation", 8.0, 0.0),
    ("mod2.selector", 9.0, 0.0),
    ("mod2.variation", 9.0, 0.0),
    // The FAST / CHOR / PING PONG legends of the knob columns touched the ON LEDs below them.
    ("comp.on", 0.0, 8.0),
    ("comp.global", 0.0, 8.0),
    ("reverb.on", 0.0, 6.0),
    ("reverb.global", 0.0, 6.0),
    ("delay.on", 0.0, 7.0),
    ("delay.global", 0.0, 7.0),
    // The Rotary DRIVE knob moves up so its name fits between the numerals and the ORGAN legend.
    ("rotary.drive", 0.0, -4.0),
    // The ORGAN / STOP MODE names and the SLOW / FAST row need a button pitch of 34 px at the
    // 7.5 px minimum font; the verified layout rects (PR #375) already give more.
    // LED spacing (rules `B`, `T`): buttons whose LED moved up 3.3 px need their legends clear.
    ("organ.zone_leds", 0.0, 1.5),
    ("organ.vibrato_on.a", 0.0, 4.5),
    ("organ.vibrato_on.b", 0.0, 4.5),
    ("piano.soft_release", 1.5, 0.0), // SOFT REL / STR RES LEDs touched the level ladder (rule `D`)
    ("piano.string_res", 1.5, 0.0),
    ("piano.ped_noise", 1.5, 0.0),
    ("piano.zone_leds", 0.0, 4.0), // the ZONE - / + legends printed into the zone LEDs
    // KB TOUCH / DYN COMP names and titles sit 1.2 mm lower so the PED NOISE legend clears them.
    ("piano.kb_touch", 0.0, 3.0),
    ("piano.dyn_comp", 0.0, 3.0),
    ("organ.on", 0.0, -1.0),
    ("organ.solo", 0.0, -1.0),
];

/// `(id prefix, dx, dy)`: shift of every control whose id starts with the prefix, added to its
/// own entry in [`TWEAKS`]. These open vertical room between the stacked printed boxes
/// (`nord_panel::layout::frames`): the 2000 px photo leaves too little between rows for 7.5 px
/// legends plus a frame border and its title.
pub const GROUP_TWEAKS: &[(&str, f32, f32)] = &[
    // The OLED dials' LIST names touched the OSCILLATORS title.
    ("synth.dial", 0.0, -4.0),
    // The B3 PERCUSSION box moves right to make room for the six VIB/CHORUS type names.
    ("organ.perc_", 3.0, 0.0),
    // Layer Effects: Mod 1 clear of the header's ON button legend, Mod 2 and Amp Sim/EQ down,
    // Comp and Reverb down under the Delay box.
    ("mod1.", 0.0, 6.0),
    ("mod2.", 0.0, 12.0),
    ("amp.", 0.0, 30.0),
    // Program area: small nudges (at most 4 px = 1.7 mm, inside the 2 mm photo tolerance) that keep
    // the legends of neighbouring controls apart.
    ("preset_library.", 0.0, 4.0),
    ("program.shift.right", 0.0, 4.0),
    ("program.live_mode", 0.0, 4.0),
    ("program.num_pad", 0.0, 4.0),
    ("program.dial", 0.0, 4.0),
    ("program.list", 0.0, 4.0),
    ("program.page_", 0.0, 4.0),
    ("program.bank_", 0.0, 4.0),
    ("comp.", 0.0, 14.0),
    ("reverb.dry_wet", 6.0, 0.0),
    ("reverb.", 8.0, 14.0),
];

/// Nudge of control `id` (zero when it has none).
pub fn nudge(id: &str) -> Vec2 {
    let own = TWEAKS
        .iter()
        .find(|(i, _, _)| *i == id)
        .map_or(Vec2::ZERO, |(_, dx, dy)| vec2(*dx, *dy));
    GROUP_TWEAKS
        .iter()
        .filter(|(prefix, _, _)| id.starts_with(prefix))
        .fold(own, |acc, (_, dx, dy)| acc + vec2(*dx, *dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_ids_have_no_nudge_and_known_ones_do() {
        assert_eq!(nudge("no.such.control"), Vec2::ZERO);
        let (id, dx, dy) = TWEAKS
            .iter()
            .copied()
            .find(|(i, _, _)| !GROUP_TWEAKS.iter().any(|(p, _, _)| i.starts_with(p)))
            .unwrap();
        assert_eq!(nudge(id), vec2(dx, dy));
    }

    #[test]
    fn every_tweaked_id_is_a_known_control() {
        for (id, _, _) in TWEAKS {
            assert!(
                nord_panel::layout::rect_for(id).is_some(),
                "{id} has no layout rect"
            );
        }
    }

    #[test]
    fn group_nudges_add_to_the_own_entry_and_match_known_ids() {
        // comp.on: own +8, group +14.
        assert_eq!(nudge("comp.on"), vec2(0.0, 22.0));
        assert_eq!(nudge("synth.arp.run"), Vec2::ZERO);
        assert_eq!(nudge("mod2.rate"), vec2(0.0, 12.0));
        for (prefix, _, _) in GROUP_TWEAKS {
            assert!(
                nord_panel::all_controls()
                    .iter()
                    .any(|c| c.id.starts_with(prefix)),
                "{prefix} matches no control"
            );
        }
    }

    #[test]
    fn nudges_are_small() {
        for (id, dx, dy) in TWEAKS {
            assert!(dx.abs() <= 40.0 && dy.abs() <= 40.0, "{id}");
        }
    }
}
