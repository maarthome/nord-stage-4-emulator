//! Per-control position nudges applied on top of the `nord_panel::layout` tables.
//!
//! The layout rectangles are `ASSUMED` (+-3 px estimated from one 2000 px photo), so they are
//! not edited. Where two neighbours print their text into each other at the panel scale, the
//! control is nudged here instead, by `(dx, dy)` logical px, with the reason next to the entry.
//! `layout_px::rect_of` applies the table; `legibility_tests` proves the result is collision free.

use egui::{vec2, Vec2};

/// `(control id, dx, dy)`: shift of the control's rectangle in logical px.
pub const TWEAKS: &[(&str, f32, f32)] = &[
    // The LFO RATE name collided with the "GROUP" Shift legend of the LFO waveform button above.
    ("synth.lfo.rate", 0.0, 12.0),
    ("synth.lfo.rate_mst_clk", 0.0, 12.0),
    // The FREQ name collided with the "GROUP" Shift legend of the filter TYPE button above.
    ("synth.filter.freq", 0.0, 8.0),
    // The two KB ZONE legends under OCTAVE / SHIFT printed into the zone LEDs.
    ("synth.zone_leds", 0.0, 9.0),
    // The OSC CTRL name printed into the ENV TO PITCH legend of the PITCH/SMP button above.
    ("synth.osc.ctrl", 0.0, 5.0),
    // "ENV AMT" / "RES" touched the VELOCITY legend of the ENVELOPE button above.
    ("synth.osc.env_amt", 0.0, 4.0),
    ("synth.filter.res", 0.0, 4.0),
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
    ("split.leds", 0.0, -4.0),
    // Layer Effects: the printed lists RM / TREM / A-PAN and CHOR / FLANG / PHAS are wider than the
    // 7.5 px minimum font allows between the AMOUNT numerals and the selector LEDs; the knob and
    // the selector each give 1 mm (2.5 px).
    ("mod1.amount", -2.5, 0.0),
    ("mod2.amount", -2.5, 2.5), // and 1 mm down with RATE: the numerals `6` / `4` touched the MOD 2 title
    ("mod2.rate", 0.0, 2.5),
    ("mod1.selector", 2.5, 0.0),
    ("mod2.selector", 2.5, 0.0),
    ("mod1.variation", 2.5, 0.0), // shared rects move with their base
    ("mod2.variation", 2.5, 0.0),
    // The TREBLE numeral `10` ends against the ON rocker; the top numerals of the EQ row (`0`) need the
    // room the model button's VAR legend printed into (the row moves 1.65 mm down, the model up 0.8 mm).
    ("amp.treble", -1.5, 4.0),
    ("amp.bass", 0.0, 4.0),
    ("amp.mid", 0.0, 4.0),
    ("amp.model", 0.0, -2.0),
    ("amp.variation", 0.0, -2.0),
    ("amp.filter", 0.0, -2.0),
    ("amp.to_rotary", 0.0, -2.0),
    // The AMP SIM/EQ title printed into the numerals of the top knob row below it.
    ("amp.drive", 0.0, 4.0),
    ("amp.mid_freq", 0.0, 4.0),
    ("amp.res", 0.0, 4.0),
    // The ARP/POLY/GATE legend printed into the RATE `8` and RANGE `2` numerals.
    ("synth.arp.range", 5.0, 0.0),
    // The Rotary DRIVE knob moves up so its name fits between the numerals and the ORGAN legend.
    ("rotary.drive", 0.0, -4.0),
    // The ORGAN / STOP MODE names and the SLOW / FAST row need a button pitch of 34 px at the
    // 7.5 px minimum font; the verified layout rects (PR #375) already give more.
    // LED spacing (rules `B`, `T`): buttons whose LED moved up 3.3 px need their legends clear.
    ("organ.zone_leds", 0.0, 1.5),
    ("organ.vibrato_on.a", 0.0, 2.0),
    ("organ.vibrato_on.b", 0.0, 2.0),
    ("piano.soft_release", 1.5, 0.0), // SOFT REL / STR RES LEDs touched the level ladder (rule `D`)
    ("piano.string_res", 1.5, 0.0),
    ("piano.ped_noise", 1.5, 0.0),
    ("organ.on", 0.0, -1.0),
    ("organ.solo", 0.0, -1.0),
];

/// `(id prefix, dx, dy)`: shift of every control whose id starts with the prefix, added to its
/// own entry in [`TWEAKS`]. These open vertical room between the stacked printed boxes
/// (`nord_panel::layout::frames`): the 2000 px photo leaves too little between rows for 7.5 px
/// legends plus a frame border and its title.
pub const GROUP_TWEAKS: &[(&str, f32, f32)] = &[
    // Rotary Speaker box: its controls sit on the frame's vertical centre line (the frame was
    // built around the button column and grew to the left; now the column moves 11 px left).
    ("rotary.", -11.0, 0.0),
    // Synth: the Arpeggiator/Gate row moves up (free space under the header), the LFO / Oscillators
    // / Filter / Amp / Unison row moves down (free space above the section's lower edge).
    ("synth.arp.rate", 0.0, -12.0),
    ("synth.arp.mode", 0.0, -12.0),
    ("synth.arp.range", 0.0, -12.0),
    ("synth.arp.menu", 4.0, -12.0),
    ("synth.filter.env_amt", -4.0, 0.0),
    ("synth.arp.pattern", 0.0, -12.0),
    ("synth.arp.group", 0.0, -12.0),
    // The three OLED dials move up so their INFO / LIST names clear the LFO box below.
    ("synth.dial", 0.0, -10.0),
    // The Vibrato box moves right so its LEDs clear the Voice box (the glide scale numbers).
    ("synth.vibrato.", 5.0, 0.0),
    ("synth.lfo.", 0.0, 16.0),
    ("synth.osc.", 0.0, 16.0),
    ("synth.filter.", 0.0, 16.0),
    ("synth.amp.", 0.0, 16.0),
    ("synth.unison", 0.0, 16.0),
    // The B3 PERCUSSION box moves right to make room for the six VIB/CHORUS type names.
    ("organ.perc_", 3.0, 0.0),
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
        // synth.lfo.rate: own +12, group +16.
        assert_eq!(nudge("synth.lfo.rate"), vec2(0.0, 28.0));
        assert_eq!(nudge("synth.arp.run"), Vec2::ZERO);
        assert_eq!(nudge("mod2.rate"), Vec2::ZERO);
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
