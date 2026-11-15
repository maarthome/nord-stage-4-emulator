//! Bit layout of the Piano layer block (`base + 0..96`), checked against real factory programs and
//! the firmware program recall.
//!
//! Layout from the layer base (`104` for layer A, `200` for layer B, bits of the 147-byte piano
//! slice), MSB first:
//!
//! | rel | bits | field |
//! |---|---|---|
//! | 18 | 3 | category (`Edt_Pno_kBank`) |
//! | 21 | 5 | model id (`Edt_Pno_kEntry`) |
//! | 26 | 2 | variation (`Edt_Pno_kVariation`) |
//! | 28 | 32 | name hash |
//! | 60 | 2 | `Edt_Pno_kAcoustic`: bit 1 soft release, bit 0 string resonance (default 1) |
//! | 62 | 1 | pedal noise (`Edt_Pno_kPedalNoise`) |
//! | 63 | 2 | KB touch (`Edt_Pno_kKbdTouch`): 0 heavy, 1 medium, 2 light |
//! | 65 | 2 | unison (`Edt_Pno_kUnison`) |
//! | 67 | 2 | dynamic compressor |
//! | 70 | 3 | EQ / timbre |
//!
//! VERIFIED by the firmware program recall (`tools/arm_program_load.py` boot of Felt Upright with
//! single bits flipped, hooks on the store setter `0xC00D051C`, `docs/research/piano-touch-curves.md`
//! s4 and s7): flipping rel 60 sets `kAcoustic` 1 to 3, rel 61 sets it to 0, rel 62 clears
//! `kPedalNoise`, rel 63 and 64 are the touch value (0, 1, 2 reach `kKbdTouch` 0, 1, 2 for layer A,
//! the same bits `+96` for layer B), rel 65 sets `kUnison` to 2. The positions equal the `ns4decode`
//! map (`249-5` to `250-5`). Which bit of `kAcoustic` is soft release is EMPIRICAL: the default
//! value 1 is string resonance on, and over the 384 factory programs rel 60 is set 4 times (soft
//! release is rare) and rel 61 241 times.

use super::piano_preset::{PianoLayerDecoded, PianoPresetDecoded};
use crate::cbin::bitstream::BitWriter;
use crate::piano::VelocityTouch;

const LAYER_A: usize = 104;
const LAYER_B: usize = 200;

/// A payload with the tag and the given `(bit, width, value)` fields set.
fn payload(fields: &[(usize, usize, u32)]) -> [u8; 147] {
    let mut bytes = [0u8; 147];
    bytes[0] = PianoPresetDecoded::BLOCK_TAG;
    let mut w = BitWriter::new(&mut bytes);
    for &(bit, width, value) in fields {
        w.set_bit_pos(bit).unwrap();
        w.write_bits(value, width).unwrap();
    }
    bytes
}

#[test]
fn every_field_is_read_from_its_firmware_position_in_both_layers() {
    for (base, touch_bits, touch) in [
        (LAYER_A, 0u32, VelocityTouch::Heavy),
        (LAYER_A, 2, VelocityTouch::Light),
        (LAYER_B, 2, VelocityTouch::Light),
        (LAYER_B, 0, VelocityTouch::Heavy),
    ] {
        let bytes = payload(&[
            (base + 26, 2, 3),
            (base + 60, 2, 2),
            (base + 62, 1, 1),
            (base + 63, 2, touch_bits),
            (base + 65, 2, 1),
            (base + 67, 2, 3),
        ]);
        let dec = PianoPresetDecoded::unpack(&bytes).unwrap();
        let layer = if base == LAYER_A {
            dec.layer_a
        } else {
            dec.layer_b
        };
        assert_eq!(layer.variation, 3, "base {base}");
        assert!(
            layer.soft_release && !layer.string_res,
            "acoustic 2, base {base}"
        );
        assert_eq!(layer.acoustic_level, 2);
        assert!(layer.pedal_noise, "base {base}");
        assert_eq!(layer.touch, touch, "base {base} touch bits {touch_bits}");
        assert_eq!(layer.unison, 1, "base {base}");
        assert_eq!(layer.dyn_comp, 3, "base {base}");
    }
}

#[test]
fn touch_is_medium_for_one_and_the_default_packs_one() {
    let one = payload(&[(LAYER_A + 63, 2, 1)]);
    assert_eq!(
        PianoPresetDecoded::unpack(&one).unwrap().layer_a.touch,
        VelocityTouch::Medium
    );
    let packed = PianoPresetDecoded::default().pack().unwrap();
    let at = |bit: usize| (packed[bit / 8] >> (7 - bit % 8)) & 1;
    assert_eq!((at(LAYER_A + 63), at(LAYER_A + 64)), (0, 1), "Medium = 1");
    assert_eq!(
        (at(LAYER_A + 60), at(LAYER_A + 61)),
        (0, 1),
        "string resonance only"
    );
}

#[test]
fn every_touch_and_acoustic_value_round_trips_through_the_bits() {
    for touch in [
        VelocityTouch::Heavy,
        VelocityTouch::Medium,
        VelocityTouch::Light,
    ] {
        for (string_res, soft_release) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let layer = PianoLayerDecoded {
                touch,
                string_res,
                soft_release,
                variation: 2,
                ..Default::default()
            };
            let original = PianoPresetDecoded {
                layer_a: layer.clone(),
                layer_b: PianoLayerDecoded {
                    touch: VelocityTouch::Light,
                    ..layer
                },
                ..Default::default()
            };
            let back = PianoPresetDecoded::unpack(&original.pack().unwrap()).unwrap();
            assert_eq!(back.layer_a.touch, touch);
            assert_eq!(back.layer_a.string_res, string_res);
            assert_eq!(back.layer_a.soft_release, soft_release);
            assert_eq!(back.layer_a.variation, 2);
            assert_eq!(back.layer_b.touch, VelocityTouch::Light);
        }
    }
}

/// Layer A block (`slice[13..25]`) of a real RevK v1.66 factory program, hex, and the values read
/// at the positions of the table above.
struct Fixture {
    name: &'static str,
    hex: &'static str,
    variation: u8,
    acoustic: u8,
    pedal: bool,
    touch: u8,
    unison: u8,
    dyn_comp: u8,
    timbre: u8,
}

const fn fx(name: &'static str, hex: &'static str, f: [u8; 7]) -> Fixture {
    Fixture {
        name,
        hex,
        variation: f[0],
        acoustic: f[1],
        pedal: f[2] != 0,
        touch: f[3],
        unison: f[4],
        dyn_comp: f[5],
        timbre: f[6],
    }
}

fn block(hex: &str) -> [u8; 12] {
    let mut out = [0u8; 12];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

const FACTORY: [Fixture; 12] = [
    fx(
        "B3 Chords n Solo",
        "0b504004814254f000000000",
        [0, 0, 0, 0, 0, 0, 0],
    ),
    fx(
        "Ballad Layer Whl",
        "0b90504a111602920a000000",
        [0, 0, 1, 0, 0, 1, 4],
    ),
    fx(
        "Big Felt Pad Whl",
        "0b41404e986f07a411800000",
        [0, 1, 0, 0, 0, 2, 3],
    ),
    fx(
        "Astoria Grand",
        "0b5040c444ebb81600000000",
        [0, 1, 1, 0, 0, 0, 0],
    ),
    fx(
        "B3 Jazz Split",
        "0b50488d6e26eff088000000",
        [0, 0, 0, 1, 0, 1, 0],
    ),
    fx(
        "J3 Open Sky",
        "0b505143d96dd0d289000000",
        [0, 0, 1, 1, 0, 1, 2],
    ),
    fx(
        "Digi Keys Pad",
        "0b90608e87e1e8fa80000000",
        [0, 2, 1, 1, 0, 0, 0],
    ),
    fx(
        "Classic Wah Clav",
        "0b90d8220cbfa74101000000",
        [2, 0, 0, 2, 0, 0, 2],
    ),
    fx(
        "Bubbles Mk I",
        "0b90504a1116029300000000",
        [0, 0, 1, 2, 0, 0, 0],
    ),
    fx(
        "Shimmer Upright",
        "0b504846eb2f44c708800000",
        [0, 1, 1, 2, 0, 1, 1],
    ),
    fx(
        "Distorted Whl II",
        "0b50d087158709ab10000000",
        [0, 2, 1, 2, 0, 2, 0],
    ),
    fx(
        "Chamber Upright",
        "0b50494ddd32104f20000000",
        [0, 3, 1, 2, 1, 0, 0],
    ),
];

#[test]
fn factory_program_blocks_decode_with_the_verified_layout() {
    let want = [
        VelocityTouch::Heavy,
        VelocityTouch::Medium,
        VelocityTouch::Light,
    ];
    for f in FACTORY {
        let name = f.name;
        let mut slice = [0u8; 147];
        slice[0] = PianoPresetDecoded::BLOCK_TAG;
        slice[13..25].copy_from_slice(&block(f.hex));
        let dec = PianoPresetDecoded::unpack(&slice).unwrap();
        let a = &dec.layer_a;
        assert_eq!(a.variation, f.variation, "{name} variation");
        assert_eq!(a.acoustic_level, f.acoustic, "{name} acoustic");
        assert_eq!(a.string_res, f.acoustic & 1 != 0, "{name} string res");
        assert_eq!(a.soft_release, f.acoustic & 2 != 0, "{name} soft release");
        assert_eq!(a.pedal_noise, f.pedal, "{name} pedal noise");
        assert_eq!(a.touch, want[usize::from(f.touch)], "{name} touch");
        assert_eq!(a.unison, f.unison, "{name} unison");
        assert_eq!(a.dyn_comp, f.dyn_comp, "{name} dyn comp");
        assert_eq!(a.timbre_raw, Some(f.timbre), "{name} timbre bits");
        // The block re-encodes to the same field values.
        let again = PianoPresetDecoded::unpack(&dec.pack().unwrap())
            .unwrap()
            .layer_a;
        assert_eq!(again.touch, a.touch, "{name} round trip");
        assert_eq!(again.acoustic_level, a.acoustic_level, "{name} round trip");
        assert_eq!(again.variation, a.variation, "{name} round trip");
    }
}
