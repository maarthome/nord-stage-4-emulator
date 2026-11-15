//! `ns4 preset-check --bits`: where the decoders collapse raw bits.
//!
//! For every preset whose `pack(decode(file))` differs from the file, the differing payload bits
//! are grouped into runs of adjacent bits. Each run is reported with the number of presets that
//! show it and the raw value in the file against the value the decoder re-packs (OQ31).

use crate::cbin::{CbinFile, PresetSection, ProgramDecoded, SectionPreset};
use std::collections::BTreeMap;

/// One run of adjacent differing bits in a payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BitRun {
    /// Payload bit position of the run (MSB-first, bit 0 = MSB of byte 0).
    pub start: usize,
    /// Run length in bits.
    pub len: usize,
    /// Raw bits of the file over the run.
    pub file: u32,
    /// Bits `pack(decode(file))` writes over the run.
    pub packed: u32,
}

fn bit(bytes: &[u8], pos: usize) -> u32 {
    u32::from((bytes[pos / 8] >> (7 - pos % 8)) & 1)
}

/// Runs of adjacent differing bits between `file` and `packed`.
pub fn diff_runs(file: &[u8], packed: &[u8]) -> Vec<BitRun> {
    let mut runs: Vec<BitRun> = Vec::new();
    let mut open: Option<BitRun> = None;
    for pos in 0..file.len().min(packed.len()) * 8 {
        let (f, p) = (bit(file, pos), bit(packed, pos));
        if f == p {
            runs.extend(open.take());
            continue;
        }
        let run = open.get_or_insert(BitRun {
            start: pos,
            len: 0,
            file: 0,
            packed: 0,
        });
        run.len += 1;
        run.file = (run.file << 1) | f;
        run.packed = (run.packed << 1) | p;
    }
    runs.extend(open);
    runs
}

/// Collapsed-bit runs of one preset entry (`None`: undecodable or already lossless).
pub fn entry_runs(bytes: &[u8]) -> Option<(PresetSection, Vec<BitRun>)> {
    let file = CbinFile::parse(bytes).ok()?;
    let preset = SectionPreset::from_file(&file).ok()?;
    let packed = preset.pack().ok()?;
    let runs = diff_runs(&file.payload, &packed);
    (!runs.is_empty()).then(|| (preset.section(), runs))
}

/// Accumulates runs over many entries and prints one line per (section, bit run, values).
#[derive(Debug, Default)]
pub struct BitTally(BTreeMap<(String, BitRun), usize>);

impl BitTally {
    /// Adds the runs of one entry.
    pub fn add(&mut self, bytes: &[u8]) {
        if let Some((section, runs)) = entry_runs(bytes) {
            for r in runs {
                *self.0.entry((format!("{section:?}"), r)).or_default() += 1;
            }
        }
    }

    /// Adds the runs of one program entry (decode + pack against the file payload).
    pub fn add_program(&mut self, bytes: &[u8]) {
        let Ok(file) = CbinFile::parse(bytes) else {
            return;
        };
        let Ok(prog) = ProgramDecoded::from_cbin_file(&file) else {
            return;
        };
        let Ok(packed) = prog.to_cbin_file(file.header.bank_index, file.header.location_index)
        else {
            return;
        };
        for r in diff_runs(&file.payload, &packed.payload) {
            *self.0.entry(("Program".to_string(), r)).or_default() += 1;
        }
    }

    /// One line per distinct run: section, byte and bit positions, values, preset count.
    pub fn lines(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|((s, r), n)| {
                format!(
                    "{s} bit {} (byte {} bit {}) len {}: file {:#b} -> packed {:#b} in {n} presets",
                    r.start,
                    r.start / 8,
                    r.start % 8,
                    r.len,
                    r.file,
                    r.packed
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_group_adjacent_bits_and_keep_both_values() {
        let file = [0b0000_0110, 0b1000_0000];
        let packed = [0b0000_0000, 0b0000_0000];
        let runs = diff_runs(&file, &packed);
        assert_eq!(
            runs,
            vec![
                BitRun {
                    start: 5,
                    len: 2,
                    file: 0b11,
                    packed: 0
                },
                BitRun {
                    start: 8,
                    len: 1,
                    file: 1,
                    packed: 0
                },
            ]
        );
        assert!(diff_runs(&file, &file).is_empty());
    }

    #[test]
    fn lossless_presets_report_nothing_and_junk_is_skipped() {
        let f = SectionPreset::init(PresetSection::Piano)
            .to_file(0, 1, 0)
            .unwrap();
        assert!(entry_runs(&f.to_bytes()).is_none());
        let mut t = BitTally::default();
        t.add(b"junk");
        assert!(t.lines().is_empty());
    }

    #[test]
    fn a_lossless_program_adds_no_run_and_a_diff_is_tallied() {
        use crate::cbin::ProgramDecoded;
        let file = ProgramDecoded::default().to_cbin_file(0, 1).unwrap();
        let mut t = BitTally::default();
        t.add_program(&file.to_bytes());
        t.add_program(b"junk");
        assert!(t.lines().is_empty());
        let runs = diff_runs(&[0xF0], &[0x00]);
        t.0.insert(("Program".to_string(), runs[0].clone()), 2);
        let line = &t.lines()[0];
        assert!(
            line.starts_with("Program bit 0 (byte 0 bit 0) len 4"),
            "{line}"
        );
        assert!(line.ends_with("in 2 presets"), "{line}");
    }
}
