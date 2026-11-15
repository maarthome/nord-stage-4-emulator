//! The table of printed frames (`FRAMES`), grouped by section; see `frames.rs` for the types.

use super::frames::{FrameDef, FrameKind};
use crate::types::Section;

const fn boxed(
    id: &'static str,
    section: Section,
    title: &'static str,
    members: &'static [&'static str],
    pad: [f32; 4],
) -> FrameDef {
    FrameDef {
        id,
        section,
        title,
        kind: FrameKind::Box,
        members,
        pad,
        title_x: 0.5,
    }
}

const fn bare(
    id: &'static str,
    section: Section,
    title: &'static str,
    members: &'static [&'static str],
    pad: [f32; 4],
) -> FrameDef {
    FrameDef {
        id,
        section,
        title,
        kind: FrameKind::Title,
        members,
        pad,
        title_x: 0.5,
    }
}

use Section::{Effects, Organ, Piano, Program, Synth};

/// Every frame, grouped by section.
pub const FRAMES: &[FrameDef] = &[
    // Organ.
    boxed(
        "organ.frame.model",
        Organ,
        "ORGAN MODEL",
        &["organ.model", "organ.model_leds"],
        [20.0, 10.0, 16.0, 6.0],
    ),
    boxed(
        "organ.frame.vib",
        Organ,
        "VIB/CHORUS",
        &[
            "organ.vibrato_selector",
            "organ.vibrato_on.a",
            "organ.vibrato_on.b",
        ],
        [3.0, 20.0, 26.0, 6.0],
    ),
    boxed(
        "organ.frame.perc",
        Organ,
        "B3 PERCUSSION",
        &[
            "organ.perc_volume",
            "organ.perc_decay",
            "organ.perc_third",
            "organ.perc_on",
        ],
        [6.0, 22.0, 7.0, 8.0],
    ),
    // Piano.
    bare(
        "piano.frame.acoustics",
        Piano,
        "ACOUSTICS",
        &["piano.soft_release"],
        [0.0, 10.0, 0.0, 0.0],
    ),
    bare(
        "piano.frame.unison",
        Piano,
        "UNISON",
        &["piano.unison"],
        [0.0, 21.0, 0.0, 0.0],
    ),
    bare(
        "piano.frame.kb_touch",
        Piano,
        "KB TOUCH",
        &["piano.kb_touch"],
        [0.0, 21.0, 0.0, 0.0],
    ),
    bare(
        "piano.frame.dyn_comp",
        Piano,
        "DYN COMP",
        &["piano.dyn_comp"],
        [0.0, 21.0, 0.0, 0.0],
    ),
    boxed(
        "piano.frame.select",
        Piano,
        "PIANO SELECT",
        &["piano.type_leds", "piano.type", "piano.select_dial"],
        [0.0, 8.0, 0.0, 8.0],
    ),
    // Synth.
    boxed(
        "synth.frame.mode",
        Synth,
        "MODE",
        &["synth.mode"],
        [4.9, 21.0, 5.0, 9.6],
    ),
    boxed(
        "synth.frame.arp",
        Synth,
        "ARPEGGIATOR / GATE",
        &[
            "synth.arp.rate",
            "synth.arp.mode",
            "synth.arp.range",
            "synth.arp.menu",
        ],
        [5.5, 10.5, 5.3, 8.0],
    ),
    boxed(
        "synth.frame.voice",
        Synth,
        "VOICE",
        &["synth.voice.mode", "synth.voice.glide"],
        [4.9, 10.5, 7.7, 8.5],
    ),
    boxed(
        "synth.frame.vibrato",
        Synth,
        "VIBRATO",
        &["synth.vibrato.selector", "synth.vibrato.menu"],
        [5.4, 15.6, 5.1, 6.2],
    ),
    boxed(
        "synth.frame.lfo",
        Synth,
        "LFO",
        &[
            "synth.lfo.waveform",
            "synth.lfo.rate",
            "synth.lfo.mod_amt",
            "synth.lfo.dest",
        ],
        [5.3, 7.9, 5.5, 12.0],
    ),
    boxed(
        "synth.frame.osc",
        Synth,
        "OSCILLATORS",
        &[
            "synth.osc.pitch_smp",
            "synth.osc.envelope",
            "synth.osc.ctrl",
            "synth.osc.env_amt",
        ],
        [8.5, 10.9, 7.9, 14.0],
    ),
    boxed(
        "synth.frame.filter",
        Synth,
        "FILTER",
        &[
            "synth.filter.type",
            "synth.filter.envelope",
            "synth.filter.env_amt",
            "synth.filter.freq",
            "synth.filter.res",
        ],
        [4.5, 8.0, 5.3, 14.0],
    ),
    boxed(
        "synth.frame.amp",
        Synth,
        "AMP",
        &["synth.amp.envelope"],
        [4.1, 10.9, 4.5, 11.3],
    ),
    boxed(
        "synth.frame.unison",
        Synth,
        "UNISON",
        &["synth.unison"],
        [4.1, 19.0, 4.5, 1.3],
    ),
    // Layer Effects.
    boxed(
        "fx.frame.mod1",
        Effects,
        "MOD 1",
        &["mod1.rate", "mod1.amount", "mod1.selector", "mod1.on"],
        [9.4, 11.9, 3.3, 4.3],
    )
    .at(0.28),
    boxed(
        "fx.frame.mod2",
        Effects,
        "MOD 2",
        &["mod2.rate", "mod2.amount", "mod2.selector", "mod2.on"],
        [9.4, 9.4, 3.3, 4.0],
    )
    .at(0.31),
    boxed(
        "fx.frame.amp",
        Effects,
        "AMP SIM/EQ",
        &[
            "amp.drive",
            "amp.mid_freq",
            "amp.model",
            "amp.on",
            "amp.bass",
            "amp.mid",
            "amp.treble",
        ],
        [8.5, 9.4, 3.3, 9.2],
    )
    .at(0.28),
    boxed(
        "fx.frame.delay",
        Effects,
        "DELAY",
        &[
            "delay.tempo",
            "delay.feedback",
            "delay.effects",
            "delay.filter",
            "delay.dry_wet",
            "delay.tap_set",
            "delay.on",
        ],
        [10.1, 7.2, 4.8, 10.0],
    ),
    boxed(
        "fx.frame.comp",
        Effects,
        "COMP",
        &["comp.amount", "comp.on"],
        [10.6, 11.1, 8.4, 13.0],
    ),
    boxed(
        "fx.frame.reverb",
        Effects,
        "REVERB",
        &[
            "reverb.bright_dark",
            "reverb.type",
            "reverb.dry_wet",
            "reverb.on",
        ],
        [4.3, 16.9, 4.8, 10.2],
    ),
    // Program area (the red central block). VERIFIED against the HA88 top-down render: the pads
    // reproduce the printed boxes (MORPH ASSIGN x 4905..5348 y 697..885 px, PRESET LIBRARY
    // x 5208..5655 y 925..1098, PROGRAM x 5130..5738 y 1478..1826; SPLIT / MST CLK / TRANSP are
    // title tabs over bare controls). The PROGRAM box holds only the eight buttons, not the OLED.
    boxed(
        "program.frame.morph",
        Program,
        "MORPH ASSIGN",
        &["morph.wheel", "morph.at", "morph.ctrlped"],
        [4.0, 14.0, 4.0, 12.0],
    ),
    boxed(
        "program.frame.split",
        Program,
        "SPLIT",
        &["split.on_set", "split.leds"],
        [0.5, 12.0, 0.5, 8.0],
    ),
    boxed(
        "program.frame.mst_clk",
        Program,
        "MST CLK",
        &["mst_clk.tap_set"],
        [1.7, 14.0, 1.7, 9.0],
    ),
    boxed(
        "program.frame.transp",
        Program,
        "TRANSP",
        &["transp.on_set"],
        [4.0, 14.0, 4.0, 8.5],
    ),
    boxed(
        "program.frame.preset_library",
        Program,
        "PRESET LIBRARY",
        &[
            "preset_library.organ",
            "preset_library.piano",
            "preset_library.synth",
        ],
        [4.0, 12.0, 4.0, 8.0],
    ),
    boxed(
        "program.frame.program",
        Program,
        "PROGRAM",
        &[
            "program.button.1",
            "program.button.4",
            "program.button.5",
            "program.button.8",
        ],
        [3.6, 14.9, 4.2, 9.5],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_a_frame_for_every_framed_section() {
        for s in [
            Section::Organ,
            Section::Piano,
            Section::Synth,
            Section::Effects,
            Section::Program,
        ] {
            assert!(FRAMES.iter().any(|f| f.section == s), "{s:?}");
        }
        assert_eq!(FRAMES.len(), 29);
    }

    #[test]
    fn titles_are_printed_in_capitals() {
        for f in FRAMES {
            assert_eq!(f.title, f.title.to_uppercase(), "{}", f.id);
        }
    }
}
