//! Debug and refill helpers of the SY scenarios.
//!
//! ```text
//! dump ADDR COUNT     print COUNT DM words from ADDR as hex on standard error
//! regs NAME...        print the named R/I/M/L/B registers on standard error (debug)
//! refill VOICE [fill=WORD]
//!                     stand-in for the external-port DMA that refills the 128-word stream staging
//!                     buffer of voice VOICE from SDRAM (docs/research/sy-smp-audio.md section 5)
//! ```
//!
//! The ring refill unpacker `0x1C1228` reads the packed sample stream through `I1`, a circular
//! buffer of `L1 = 0x80` words based at `DM(rec + 0x2F)` (`B1 = DM(I3 + 9)`, `I3 = rec + 0x26`; the
//! firmware sets `rec + 0x2F = 0x93FF7`), with the read pointer in `DM(rec + 0x1C)`. The hardware
//! DMA keeps that buffer ahead of the reader; with an empty buffer the header word is 0, the
//! width index is 2 and the width indexed pointer table `DM(0x95C7C + 2)` jumps into the trap
//! `0x1C12DD`. `refill` writes `fill` into every word of the buffer before each interrupt. The
//! default fill is a block header that selects the 15 bit handler (`0x1C12E7`) and also decodes as
//! a valid header when the reader finds it where a block starts, so any alignment works.
//!
//! Provenance: the header fields are read from the listing `0x1C1245..0x1C1270` (bit 23, count
//! at bits 22..19, bit 18, handler selector at bits 17..14, length at bits 13..0), VERIFIED by
//! reading; the default fill is ASSUMED (a synthetic page, not a recorded SDRAM stream), so the
//! audio decoded from it is meaningless and only the control flow is tested.
use crate::exec::Runner;
use crate::script::REC_BASE;
use crate::script::REC_STRIDE;

/// ASSUMED: header word `count - 1 = 14` (bits 22..19), selector 0, length `0x40`; selects the
/// 15 bit unpack handler (`DM(0x95C7C + 30) = 0x1C12E7`).
pub const DEFAULT_FILL: u32 = (14 << 19) | 0x40;
/// Offset of the staging buffer base in a voice record (`DM(rec + 0x2F)`, VERIFIED by the dump of
/// the record after the prepare state: `0x93FF7`).
const BASE_FIELD: u32 = 0x2F;
/// Words of the circular staging buffer (`L1 = 0x80` at `0x1C11AF`).
const BUFFER_WORDS: u32 = 0x80;
use crate::script::{parse_addr, parse_number, register_code, Cmd};

/// Parses `dump`; `None` when `cmd` is not one of this module's commands.
pub fn parse(cmd: &str, pos: &[String]) -> Option<Result<Cmd, String>> {
    match cmd {
        "dump" => Some(match (pos.first(), pos.get(1)) {
            (Some(a), Some(n)) => match (parse_addr(a), parse_number(n)) {
                (Ok(addr), Some(count)) => Ok(Cmd::Dump {
                    addr,
                    count: count as u32,
                }),
                _ => Err("dump: needs ADDR COUNT".into()),
            },
            _ => Err("dump: needs ADDR COUNT".into()),
        }),
        "regs" => Some(
            pos.iter()
                .map(|n| {
                    register_code(n)
                        .map(|c| (n.clone(), c))
                        .ok_or(format!("regs: bad register {n:?}"))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Cmd::Regs),
        ),
        "refill" => Some(match pos.first().and_then(|v| parse_number(v)) {
            Some(voice) if voice < 56 => Ok(Cmd::Refill {
                voice: voice as u32,
                fill: DEFAULT_FILL,
            }),
            _ => Err("refill: needs a voice 0..55".into()),
        }),
        _ => None,
    }
}

impl Runner {
    /// Prints the named registers.
    pub fn regs(&self, regs: &[(String, u8)]) {
        let line: Vec<String> = regs
            .iter()
            .map(|(n, c)| format!("{n}={:08x}", self.core.read_ureg(*c)))
            .collect();
        eprintln!("regs {}", line.join(" "));
    }

    /// Fills the staging buffer of every refilled voice (called before each interrupt).
    pub fn run_refill(&mut self) -> Result<(), String> {
        for (voice, fill) in self.refill.clone() {
            let base = self
                .core
                .dm_read(REC_BASE + REC_STRIDE * voice + BASE_FIELD)
                .map_err(|a| format!("refill: bad address 0x{a:x}"))?;
            if base == 0 {
                continue; // the voice has no staging buffer yet (before the prepare state)
            }
            for k in 0..BUFFER_WORDS {
                self.core
                    .dm_write(base + k, fill)
                    .map_err(|a| format!("refill: bad address 0x{a:x}"))?;
            }
        }
        Ok(())
    }

    /// Prints `count` words from `addr`, eight per line.
    pub fn dump(&mut self, addr: u32, count: u32) -> Result<(), String> {
        for row in (0..count).step_by(8) {
            let mut line = format!("dump {:#07x}:", addr + row);
            for k in row..count.min(row + 8) {
                let w = self
                    .core
                    .dm_read(addr + k)
                    .map_err(|a| format!("dump: bad address 0x{a:x}"))?;
                line.push_str(&format!(" {w:08x}"));
            }
            eprintln!("{line}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dump() {
        let p = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse("dump", &p(&["0x92400", "8"])),
            Some(Ok(Cmd::Dump {
                addr: 0x92400,
                count: 8
            }))
        );
        assert!(parse("dump", &p(&["0x1"])).unwrap().is_err());
        assert!(parse("poke", &p(&["1", "2"])).is_none());
    }

    #[test]
    fn parses_refill() {
        let p = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse("refill", &p(&["0"])),
            Some(Ok(Cmd::Refill {
                voice: 0,
                fill: DEFAULT_FILL
            }))
        );
        assert!(parse("refill", &p(&["56"])).unwrap().is_err());
    }

    #[test]
    fn default_fill_selects_the_15_bit_handler() {
        // count - 1 at bits 22..19 -> index 2 * count = 30 -> 0x1C12E7 in the table at 0x95C7C
        assert_eq!(((DEFAULT_FILL >> 19) & 0xF) + 1, 15);
        assert_eq!(DEFAULT_FILL & 0x3FFF, 0x40);
    }

    #[test]
    fn refill_fills_the_buffer_once_the_voice_has_a_base() {
        let mut r = Runner::new(&[0u8; 16], false);
        r.refill.push((0, 0xABCD));
        r.run_refill().unwrap(); // base 0: nothing written
        assert_eq!(r.core.dm_read(0x93ff7).unwrap(), 0);
        r.core.dm_write(REC_BASE + BASE_FIELD, 0x93ff7).unwrap();
        r.run_refill().unwrap();
        assert_eq!(r.core.dm_read(0x93ff7).unwrap(), 0xABCD);
        assert_eq!(r.core.dm_read(0x93ff7 + 0x7f).unwrap(), 0xABCD);
        assert_eq!(r.core.dm_read(0x93ff7 + 0x80).unwrap(), 0);
    }

    #[test]
    fn dump_reads_dm() {
        let mut r = Runner::new(&[0u8; 16], false);
        r.core.dm_write(0x92400, 7).unwrap();
        assert!(r.dump(0x92400, 3).is_ok());
    }
}
