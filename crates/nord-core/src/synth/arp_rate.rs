//! Arpeggiator / Gate rate dial of the Synth: stored 7-bit value to tempo, and the step length.
//!
//! The Rate knob stores `0..=127` (layer bits `+175`, 7 bits). The firmware turns it into a clock
//! rate with the table `R[v]` (`bank0_CODE@0x2BC3F0` = `0xC02BBF98 + 0x458`, 128 ints, the Glue
//! update `0xC011A51C`) and plays one pattern record per **grid point** of the clock, `2**18` phase
//! units apart, at `R / 60 * 2**19` phase units per second. The panel shows `D = R / 2` quarter
//! notes per minute; one grid point is `15 / D` s and one **step** is two grid points (the
//! compiler keeps two records per step), so a step lasts `30 / D` s: a 1/8 note at `D`, exactly as
//! the manual (p.36) says. See `docs/research/synth-arp-patterns.md` sections 8 and 9
//! (`tools/ns4_arp_gate.py clock`, `tools/ns4_arp_player.py`).

/// Number of positions of the Rate dial (7 bits).
pub const ARP_RATE_STEPS: usize = 128;
/// Slowest displayed tempo in quarter notes per minute (dial 0).
pub const ARP_MIN_BPM: f32 = 16.0;
/// Fastest displayed tempo (dial 127).
pub const ARP_MAX_BPM: f32 = 1200.0;
/// Pattern records (grid points) per step.
///
/// VERIFIED: the compiler `0xC02424DC` writes two records per step (`[obj+0x24] = steps * 2`,
/// `0xC011C520`) and the player `0xC023B3EC` plays one record per grid point; checked by
/// `python tools/ns4_arp_player.py CODE gates` (firmware events == one record per grid point for 6
/// patterns on two grids).
pub const RECORDS_PER_STEP: u32 = 2;

/// Firmware rate table `R[v]`: twice the displayed BPM (`D = R / 2`).
///
/// VERIFIED: `bank0_CODE@0x2BC3F0` (`0xC02BBF98 + 0x458`), 128 little-endian ints;
/// `python tools/ns4_arp_gate.py clock CODE` replays the Glue rate update `0xC011A51C` for all 128
/// dial values and the clock rate equals `R[v] / 60 * 2**19 / 24000` for each (0 differences); the
/// list `R / 2` is `ns4decode`'s `ArpRate` BPM list (16 bpm at dial 0 up to 1200). The second value
/// of each duplicate pair (dial 1, 29, 34, 39, 43, 48) is a value the dial never writes ("N.A."
/// there); the tests below pin the table and these positions.
pub const ARP_RATE_TABLE: [u16; ARP_RATE_STEPS] = [
    32, 32, 36, 40, 48, 52, 56, 60, //
    68, 72, 76, 84, 88, 92, 96, 100, //
    108, 112, 116, 120, 124, 128, 132, 136, //
    140, 144, 148, 152, 156, 156, 160, 164, //
    168, 172, 172, 176, 180, 184, 188, 188, //
    192, 196, 200, 200, 204, 208, 212, 216, //
    216, 220, 224, 228, 232, 236, 240, 244, //
    248, 252, 256, 260, 264, 268, 276, 280, //
    284, 292, 296, 304, 308, 316, 324, 332, //
    340, 348, 356, 364, 372, 380, 392, 400, //
    408, 420, 432, 440, 452, 464, 476, 488, //
    504, 516, 532, 548, 564, 580, 596, 616, //
    636, 656, 676, 700, 724, 752, 784, 820, //
    856, 900, 944, 988, 1040, 1092, 1148, 1204, //
    1264, 1324, 1392, 1456, 1524, 1596, 1668, 1744, //
    1820, 1900, 1980, 2060, 2144, 2228, 2312, 2400, //
];

/// Displayed tempo `D` (quarter notes per minute) of a stored Rate value (`raw` is clamped to 127).
pub fn arp_bpm_from_raw(raw: u32) -> f32 {
    f32::from(ARP_RATE_TABLE[(raw as usize).min(ARP_RATE_STEPS - 1)]) / 2.0
}

/// The stored Rate value of the dial position whose tempo is nearest to `bpm`.
///
/// Of two values with the same tempo (the duplicate pairs of the table) the first is returned:
/// that is the one the dial writes.
pub fn arp_raw_from_bpm(bpm: f32) -> u32 {
    let mut best = 0usize;
    let mut best_err = f32::INFINITY;
    for raw in 0..ARP_RATE_STEPS {
        let err = (arp_bpm_from_raw(raw as u32) - bpm).abs();
        if err < best_err {
            best = raw;
            best_err = err;
        }
    }
    best as u32
}

/// Length of one arpeggiator / gate step in seconds at the displayed tempo `bpm`.
///
/// `30 / D`: two grid points of `15 / D` s (VERIFIED, [`RECORDS_PER_STEP`] and the table above).
/// `bpm` is clamped to the dial range `16..=1200`.
pub fn step_seconds_at(bpm: f32) -> f32 {
    RECORDS_PER_STEP as f32 * 15.0 / bpm.clamp(ARP_MIN_BPM, ARP_MAX_BPM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_ends_and_length_match_the_firmware() {
        assert_eq!(ARP_RATE_TABLE.len(), 128);
        assert_eq!(ARP_RATE_TABLE[0], 32);
        assert_eq!(ARP_RATE_TABLE[127], 2400);
        assert_eq!(arp_bpm_from_raw(0), ARP_MIN_BPM);
        assert_eq!(arp_bpm_from_raw(127), ARP_MAX_BPM);
        // The factory default of the Rate dial (registry default 54) is 120 BPM.
        assert_eq!(arp_bpm_from_raw(54), 120.0);
    }

    #[test]
    fn table_never_decreases_and_all_entries_are_even() {
        for pair in ARP_RATE_TABLE.windows(2) {
            assert!(pair[0] <= pair[1]);
        }
        assert!(ARP_RATE_TABLE.iter().all(|r| r % 2 == 0));
    }

    #[test]
    fn duplicate_pairs_sit_where_ns4decode_has_no_value() {
        let dups: Vec<usize> = (1..ARP_RATE_STEPS)
            .filter(|&i| ARP_RATE_TABLE[i] == ARP_RATE_TABLE[i - 1])
            .collect();
        assert_eq!(dups, [1, 29, 34, 39, 43, 48]);
    }

    #[test]
    fn raw_from_bpm_inverts_every_value_the_dial_writes() {
        for raw in 0..ARP_RATE_STEPS as u32 {
            let back = arp_raw_from_bpm(arp_bpm_from_raw(raw));
            let canonical = (0..=raw)
                .find(|&r| arp_bpm_from_raw(r) == arp_bpm_from_raw(raw))
                .unwrap();
            assert_eq!(back, canonical, "raw {raw}");
        }
    }

    #[test]
    fn raw_from_bpm_picks_the_nearest_dial_position_and_clamps() {
        assert_eq!(arp_raw_from_bpm(0.0), 0);
        assert_eq!(arp_raw_from_bpm(100_000.0), 127);
        assert_eq!(arp_raw_from_bpm(121.0), 54);
        assert_eq!(arp_raw_from_bpm(135.0), arp_raw_from_bpm(134.0));
    }

    #[test]
    fn step_length_is_two_grid_points_of_the_replayed_clock() {
        // Replay-derived goldens (tools/ns4_arp_gate.py clock: a grid point every `2**18` phase
        // units at `R / 60 * 2**19` units per second; tools/ns4_arp_player.py: one record per
        // grid point, two per step): dial 0 / 54 / 127.
        for (raw, grid_point_sec) in [(0, 0.9375_f32), (54, 0.125), (127, 0.0125)] {
            let step = step_seconds_at(arp_bpm_from_raw(raw));
            assert!(
                (step - 2.0 * grid_point_sec).abs() < 1e-6,
                "dial {raw}: {step}"
            );
        }
    }

    #[test]
    fn step_length_is_an_eighth_note_at_the_displayed_tempo() {
        // The manual (p.36): the arpeggiator plays 1/8 notes at the shown quarter-note BPM.
        assert!((step_seconds_at(120.0) - 0.25).abs() < 1e-6);
        assert!((step_seconds_at(60.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn step_length_clamps_to_the_dial_range() {
        assert_eq!(step_seconds_at(1.0), step_seconds_at(ARP_MIN_BPM));
        assert_eq!(step_seconds_at(1.0e6), step_seconds_at(ARP_MAX_BPM));
        assert!((step_seconds_at(ARP_MAX_BPM) - 0.025).abs() < 1e-6);
    }
}
