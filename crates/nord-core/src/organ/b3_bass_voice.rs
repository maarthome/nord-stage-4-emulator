//! Voice records of the B3 Bass generator (`DM 0x954E6..`, `0x9559C..`, `0xB4246..`).
//!
//! Research: `docs/research/organ-farfisa-bass.md` s15, tool `tools/organ_bass_voice.py`. The
//! generator `0x121BBE` runs ten sections per call over per-key records that the ARM key-down
//! writes: pointer pairs, a coefficient pair (about `-0.003 .. -0.022`, rising with the key), shift
//! counts and mask words. The first five key-downs after an idle period fill the slots
//! `j = 0..4` (`20 + 4 + 12` words each at `0x954E6 + 20 j`, `0x9559C + 4 j`, `0xB4246 + 12 j`).
//!
//! The table holds, for every key `24..=108`, the 36 words of one fresh-session capture (the key
//! pressed as press `j = (key - 24) mod 5` of its group of five), relocated to slot 0. The words
//! carry a per-press jitter that the ARM draws (pointer offsets into the ring and coefficient
//! scatter), which is **not decoded**: the same key at another slot has other jitter words.
//! Tag: key dependence VERIFIED (ARM capture), the slot reuse and jitter ASSUMED (the distribution
//! matches, the values do not).

const CSV: &str = include_str!("../../../../docs/research/data/bass-voice-slots.csv");
/// Slots filled by the first five presses.
pub(super) const SLOTS: usize = 5;
/// Words per record: `A` 20, `B` 4, `C` 12.
pub(super) const WORDS: usize = 36;
const FIRST_KEY: usize = 24;

/// Per-key record words relocated to slot 0.
#[derive(Debug, Clone)]
pub(super) struct VoiceTable {
    rows: Vec<Option<[u32; WORDS]>>,
}

/// DM address of word `i` of the record in `slot`.
pub(super) fn word_addr(slot: usize, i: usize) -> u32 {
    let (base, n, k) = match i {
        0..=19 => (0x9_54E6, 20, i),
        20..=23 => (0x9_559C, 4, i - 20),
        _ => (0xB_4246, 12, i - 24),
    };
    base + (n * slot + k) as u32
}

impl VoiceTable {
    /// Parses the derived table (setup time only).
    pub(super) fn new() -> Self {
        let mut rows: Vec<Option<[u32; WORDS]>> = vec![None; 85];
        for line in CSV.lines().skip(1) {
            let Some((key, words)) = line.split_once(',') else {
                continue;
            };
            let Some(row) = key
                .parse::<usize>()
                .ok()
                .and_then(|k| k.checked_sub(FIRST_KEY))
                .and_then(|k| rows.get_mut(k))
            else {
                continue;
            };
            let mut w = [0u32; WORDS];
            for (slot, x) in w.iter_mut().zip(words.split(' ')) {
                *slot = u32::from_str_radix(x, 16).unwrap_or(0);
            }
            *row = Some(w);
        }
        Self { rows }
    }

    /// Record words of `key`, if the key is in `24..=108`.
    pub(super) fn words(&self, key: usize) -> Option<&[u32; WORDS]> {
        key.checked_sub(FIRST_KEY)
            .and_then(|k| self.rows.get(k))
            .and_then(Option::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_covers_every_key_and_addresses_follow_the_slot_layout() {
        let t = VoiceTable::new();
        assert!((24..=108).all(|k| t.words(k).is_some()));
        assert!(t.words(23).is_none() && t.words(109).is_none());
        assert_eq!(word_addr(0, 0), 0x9_54E6);
        assert_eq!(word_addr(1, 0), 0x9_54FA);
        assert_eq!(word_addr(2, 20), 0x9_55A4);
        assert_eq!(word_addr(1, 24), 0xB_4252);
        // the first word of each record is the constant 1.0 of the section gain
        assert_eq!(t.words(60).unwrap()[0], 0x3F80_0000);
    }

    #[test]
    fn coefficients_grow_with_the_key() {
        // word 4 is the first coefficient (negative float): its magnitude rises from key 24 to 108
        let t = VoiceTable::new();
        let c = |k: usize| f32::from_bits(t.words(k).unwrap()[4]).abs();
        let low: f32 = (24..30).map(c).sum::<f32>() / 6.0;
        let high: f32 = (102..108).map(c).sum::<f32>() / 6.0;
        assert!(high > 1.5 * low, "{low} {high}");
    }
}
