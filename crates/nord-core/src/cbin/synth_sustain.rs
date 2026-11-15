//! Per-layer Synth SUSTPED bit (`Edt_Syn_kSustainEnable`, id `0xF8`) of a program.
//!
//! The flag is the 1-bit field at block offset `+71` of every Synth layer block (the `0x32`
//! Layer chain, field order checked against the firmware by `tools/ns4_fw_order_check.py`); the
//! blocks sit at section byte 327, bits 248 / 656 / 1064 (the blocks of [`super::kb_zone`]).
//! The firmware edit-parameter table gives the id the range 0..1 and the default **1**
//! (`docs/research/arm-edit-params.csv` row 307; piano 1, organ 0).
//!
//! Spec: `docs/specs/midi-and-control.md` (SUSTPED), evidence `docs/research/synth-sustped.md`.

use super::bitstream::BitReader;
use super::container::CbinError;

/// Number of Synth layers (A, B, C).
pub const SYNTH_LAYERS: usize = 3;

/// Block offset of the SUSTPED bit inside a Synth layer block.
/// VERIFIED (order and width): `tools/ns4_fw_order_check.py` (`Edt_Syn_kSustainEnable`, 71, 1).
/// EMPIRICAL (position): 381 / 383 / 383 of the 384 factory programs store 1 there, the firmware
/// default, `tools/synth_sustped.py`.
const SUSTAIN_BIT: usize = 71;

/// `(payload byte offset of the synth section, bit offset of each layer block)`.
const SECTION: usize = 327;
const BLOCKS: [usize; SYNTH_LAYERS] = [248, 656, 1064];

/// Firmware default of `Edt_Syn_kSustainEnable`. VERIFIED: `arm-edit-params.csv` (min 0, max 1,
/// default 1).
pub const DEFAULT_ON: bool = true;

/// Serde default and `Default` value: every Synth layer at the firmware default.
pub fn default_flags() -> [bool; SYNTH_LAYERS] {
    [DEFAULT_ON; SYNTH_LAYERS]
}

/// Reads the three SUSTPED bits from an 820-byte program payload.
pub fn read_flags(payload: &[u8]) -> Result<[bool; SYNTH_LAYERS], CbinError> {
    let mut out = default_flags();
    let tail = payload.get(SECTION..).ok_or(CbinError::TooShort {
        actual: payload.len(),
        min: SECTION + 1,
    })?;
    for (flag, block) in out.iter_mut().zip(BLOCKS) {
        let mut r = BitReader::new(tail);
        r.set_bit_pos(block + SUSTAIN_BIT)?;
        *flag = r.read_bits(1)? == 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_bit(buf: &mut [u8], bit: usize) {
        buf[bit / 8] |= 0x80 >> (bit % 8);
    }

    #[test]
    fn default_is_the_firmware_default_on() {
        assert_eq!(default_flags(), [true; 3]);
    }

    #[test]
    fn flags_are_read_from_the_layer_blocks() {
        let mut payload = vec![0u8; 820];
        assert_eq!(read_flags(&payload).unwrap(), [false; 3]);
        set_bit(&mut payload, SECTION * 8 + 656 + SUSTAIN_BIT);
        assert_eq!(read_flags(&payload).unwrap(), [false, true, false]);
        set_bit(&mut payload, SECTION * 8 + 248 + SUSTAIN_BIT);
        set_bit(&mut payload, SECTION * 8 + 1064 + SUSTAIN_BIT);
        assert_eq!(read_flags(&payload).unwrap(), [true; 3]);
    }

    #[test]
    fn a_short_payload_is_an_error() {
        assert!(read_flags(&[0u8; 100]).is_err());
    }
}
