//! The five FM band tables (strip types 49..53), derived numbers committed as CSV.
//!
//! Evidence `docs/research/fm-banks-1-2.md` s1: the ARM strip apply `0xC01CE7D8` sends 361 float
//! words to `DM(0xE0900 .. 0xE0A68)` of core 2 for a pending type 49..53 (VERIFIED run,
//! `tools/synth_fm_tables.py capture`). Only the numbers are committed (D56), no firmware bytes.

use std::sync::OnceLock;

/// Table words per type (`0xE0900 .. 0xE0A68`).
pub const TABLE_WORDS: usize = 361;
/// First strip type with an FM table.
pub const FIRST_TYPE: u8 = 49;
/// Number of FM table types (49..=53).
pub const TYPES: usize = 5;
/// Word address of the first table word.
pub const TABLE_BASE: u32 = 0xE0900;

const CSV: &str = include_str!("../../../../../docs/research/data/synth-fm-tables.csv");

/// One type's table as raw words, indexed by `address - 0xE0900`.
pub type FmTable = [u32; TABLE_WORDS];

fn hex(field: &str) -> Option<u32> {
    u32::from_str_radix(field.trim().trim_start_matches("0x"), 16).ok()
}

/// Parses the committed CSV (`type,dm,word,value`); `None` when it is malformed.
fn parse(text: &str) -> Option<Vec<FmTable>> {
    let mut out = vec![[0u32; TABLE_WORDS]; TYPES];
    let mut seen = [0usize; TYPES];
    for line in text.lines().skip(1) {
        let mut f = line.split(',');
        let ty: usize = f.next()?.trim().parse().ok()?;
        let dm = hex(f.next()?)?;
        let word = hex(f.next()?)?;
        let t = ty.checked_sub(usize::from(FIRST_TYPE))?;
        let k = dm.checked_sub(TABLE_BASE)? as usize;
        *out.get_mut(t)?.get_mut(k)? = word;
        seen[t] += 1;
    }
    seen.iter().all(|&n| n == TABLE_WORDS).then_some(out)
}

/// Table of strip type `ty` (49..=53), parsed once.
pub fn fm_table(ty: u8) -> Option<&'static FmTable> {
    static TABLES: OnceLock<Option<Vec<FmTable>>> = OnceLock::new();
    let all = TABLES.get_or_init(|| parse(CSV)).as_ref()?;
    all.get(usize::from(ty.checked_sub(FIRST_TYPE)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_tables_of_361_words_load_and_the_ramp_starts_as_captured() {
        for ty in FIRST_TYPE..FIRST_TYPE + TYPES as u8 {
            assert!(fm_table(ty).is_some());
        }
        assert_eq!(fm_table(49).unwrap()[0], 0x3e95_5555);
        assert_eq!(fm_table(49).unwrap()[1], 0x3e93_3333);
        assert!(fm_table(48).is_none() && fm_table(54).is_none());
    }

    #[test]
    fn parse_rejects_a_short_table() {
        assert!(parse("type,dm,word,value\n49,0xe0900,0x3f800000,1.0\n").is_none());
    }

    #[test]
    fn ramp_of_type_49_falls_by_one_240th_per_word() {
        let t = fm_table(49).unwrap();
        let (a, b) = (f32::from_bits(t[0]), f32::from_bits(t[1]));
        assert!(((a - b) - 1.0 / 240.0).abs() < 1e-6);
    }
}
