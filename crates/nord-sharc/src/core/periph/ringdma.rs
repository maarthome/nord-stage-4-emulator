//! History-ring restart of the PNO core 1 Leslie contexts (`DM(0x310C3000)`, D126): the pass
//! routine re-bases the two ring pointers every 8th pass and then writes `7` and `0x107` to this
//! register. The words it just used as ring descriptors (`DM(0xE31E8..)` and `DM(0xE35E0..)`) are
//! the start of a DMA chain; the model runs the first record of each chain.
//!
//! Evidence (`docs/research/b3-leslie-pno-lane.md`, `sharc-sys --census`, `sharc-ring`):
//!
//! | Fact | Tag |
//! |---|---|
//! | `DM(0x310C3000)` is written twice (`7`, then `0x107`) in every 8th pass (42 of 332), nothing else of the pass touches a data register | VERIFIED (census) |
//! | The chain heads hold the records `1, 0xA096634, 0xA0E31A6, 0x40` (left) and `1, 0xA0967FE, 0xA0E359E, 0x40` (right): modify 1, a table address, the second-buffer address `c[-8]` of the Leslie context, count 0x40 | VERIFIED (dump) |
//! | The record reads as "copy the 0x40 words of the table into the second buffer" (the table is 0x0A000000 plus a PNO data address) | ASSUMED (SHARC TCB layout not decoded) |
//!
//! Off unless `sharc-sys --ring-dma`. With the Leslie chain element not installed (the live lane:
//! `0x1256AD` runs in 0 of 76 passes) nothing reads the second buffer, so the lane is unchanged.
use super::Bus;

/// Restart register (VERIFIED, census).
pub const RESTART: u32 = 0x310C_3000;
/// First write of a restart and the write that starts the chain (VERIFIED, census).
pub const ARM_WORD: u32 = 0x7;
pub const GO_WORD: u32 = 0x107;
/// Chain heads of the two Leslie contexts in the PNO core 1 image (EMPIRICAL: image layout of
/// OS v1.66, `0xE31E8` left, `0xE35E0` right).
pub const CHAINS: [u32; 2] = [0xE_31E8, 0xE_35E0];
/// DMA view of internal data memory: word address + this offset (VERIFIED, dump of the chain).
pub const DMA_BASE: u32 = 0x0A00_0000;
/// Longest copy the model accepts (the records use 0x40).
pub const MAX_COUNT: u32 = 0x400;

/// The restart model.
#[derive(Debug, Default)]
pub struct RingDma {
    armed: bool,
    /// Restarts run (the `GO_WORD` writes that followed an `ARM_WORD`).
    pub restarts: u32,
    /// Words copied by the first records of the chains.
    pub copied: u64,
}

impl RingDma {
    /// Whether `addr` is the restart register.
    pub fn contains(addr: u32) -> bool {
        addr == RESTART
    }

    /// Register write: `7` arms, `0x107` after it runs the chains.
    pub fn write(&mut self, v: u32, bus: &mut dyn Bus) {
        match v {
            ARM_WORD => self.armed = true,
            GO_WORD if self.armed => {
                self.armed = false;
                self.restarts += 1;
                for head in CHAINS {
                    self.copied += u64::from(first_record(head, bus));
                }
            }
            _ => self.armed = false,
        }
    }
}

/// Runs the record `[modify, source, destination, count]` at `head`: modify 1, both addresses in
/// the DMA view and a count of at most [`MAX_COUNT`] copy `count` words. Returns the words moved.
fn first_record(head: u32, bus: &mut dyn Bus) -> u32 {
    let rec: [u32; 4] = std::array::from_fn(|k| bus.read32(head + k as u32));
    let [modify, src, dst, count] = rec;
    let in_view = |a: u32| a & 0xFF00_0000 == DMA_BASE;
    if modify != 1 || !in_view(src) || !in_view(dst) || count == 0 || count > MAX_COUNT {
        return 0;
    }
    for k in 0..count {
        let w = bus.read32((src & 0x00FF_FFFF) + k);
        bus.write32((dst & 0x00FF_FFFF) + k, w);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Mem(BTreeMap<u32, u32>);
    impl Bus for Mem {
        fn read32(&mut self, a: u32) -> u32 {
            self.0.get(&a).copied().unwrap_or(0)
        }
        fn write32(&mut self, a: u32, v: u32) {
            self.0.insert(a, v);
        }
    }

    fn chain(m: &mut Mem, head: u32, src: u32, dst: u32, count: u32) {
        for (k, w) in [1, DMA_BASE + src, DMA_BASE + dst, count]
            .into_iter()
            .enumerate()
        {
            m.write32(head + k as u32, w);
        }
    }

    #[test]
    fn go_after_arm_copies_the_table_into_the_second_buffer() {
        let mut m = Mem::default();
        chain(&mut m, CHAINS[0], 0x9_6634, 0xE_31A6, 3);
        chain(&mut m, CHAINS[1], 0x9_67FE, 0xE_359E, 2);
        for (k, v) in [11, 22, 33].into_iter().enumerate() {
            m.write32(0x9_6634 + k as u32, v);
        }
        m.write32(0x9_67FE, 44);
        let mut d = RingDma::default();
        d.write(GO_WORD, &mut m); // no arm: ignored
        assert_eq!((d.restarts, m.read32(0xE_31A6)), (0, 0));
        d.write(ARM_WORD, &mut m);
        d.write(GO_WORD, &mut m);
        assert_eq!((d.restarts, d.copied), (1, 5));
        assert_eq!(
            [0xE_31A6, 0xE_31A7, 0xE_31A8].map(|a| m.read32(a)),
            [11, 22, 33]
        );
        assert_eq!(m.read32(0xE_359E), 44);
        // the arm word is consumed: a second GO alone does nothing
        d.write(GO_WORD, &mut m);
        assert_eq!(d.restarts, 1);
    }

    #[test]
    fn records_outside_the_assumed_shape_move_nothing() {
        let mut m = Mem::default();
        chain(&mut m, CHAINS[0], 0x9_6634, 0xE_31A6, MAX_COUNT + 1);
        m.write32(CHAINS[1], 2); // modify 2 is a ring, not a copy
        let mut d = RingDma::default();
        d.write(ARM_WORD, &mut m);
        d.write(GO_WORD, &mut m);
        assert_eq!((d.restarts, d.copied), (1, 0));
        d.write(ARM_WORD, &mut m);
        d.write(5, &mut m); // any other word disarms
        d.write(GO_WORD, &mut m);
        assert_eq!(d.restarts, 1);
        assert!(RingDma::contains(RESTART) && !RingDma::contains(RESTART + 1));
    }
}
