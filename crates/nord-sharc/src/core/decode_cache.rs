//! Level-1 pre-decoded instruction representation and direct-mapped PC cache.
//!
//! Decodes each fetched instruction once into a structured [`DecodedOp`] (foundation for M1 and
//! future Level-2 JIT / Level-3 AOT backends) and caches it per `(pc, code_gen, isa)` so hot DSP
//! loops skip `parcel()` reads, `classify()`, `word48()`, and compute-field bit slicing.
use super::alu::short_op;
use super::Core;
use crate::visa::{bits, Insn, Kind};

/// Number of slots in the direct-mapped decode cache (power of two).
pub(super) const DECODE_CACHE_SIZE: usize = 4096;
const DECODE_CACHE_MASK: usize = DECODE_CACHE_SIZE - 1;

/// Pre-decoded 23-bit compute operation (`T2a`, `T2b`, and pre-resolved `T2c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ComputeOp {
    Nop,
    FixedAlu { op: u8, n: u8, x: u8, y: u8 },
    FloatMul { n: u8, x: u8, y: u8 },
    FloatAlu { op: u8, n: u8, x: u8, y: u8 },
    DualAddSub { c: u64 },
    FixedMult { op: u8, n: u8, x: u8, y: u8 },
    Shifter { op: u8, n: u8, x: u8, y: u8 },
    Multifunction { c: u64 },
    Invalid,
}

impl ComputeOp {
    /// Decodes a 23-bit compute field once into a structured [`ComputeOp`].
    #[inline]
    pub(super) fn decode(c: u64) -> Self {
        if c == 0 {
            return Self::Nop;
        }
        if bits(c, 22, 22) == 1 {
            return Self::Multifunction { c };
        }
        let cu = bits(c, 22, 20);
        let op = bits(c, 19, 12) as u8;
        let n = bits(c, 11, 8) as u8;
        let x = bits(c, 7, 4) as u8;
        let y = bits(c, 3, 0) as u8;
        match cu {
            0 if op < 0x80 => {
                if matches!(op >> 4, 0x7) {
                    // 0x70..=0x7F is dual add/sub in fixed-point
                    Self::DualAddSub { c }
                } else {
                    Self::FixedAlu { op, n, x, y }
                }
            }
            0 => {
                if matches!(op >> 4, 0xF) {
                    Self::DualAddSub { c }
                } else {
                    Self::FloatAlu { op, n, x, y }
                }
            }
            1 if op == 0x30 => Self::FloatMul { n, x, y },
            1 => Self::FixedMult { op, n, x, y },
            2 => Self::Shifter { op, n, x, y },
            _ => Self::Invalid,
        }
    }
}

/// Structured decoded instruction for [`Core::step`] and future JIT/AOT lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DecodedOp {
    Nop,
    Idle,
    /// Conditional compute (`T2a`): `cond` is `0..=31` (`31` = unconditional `TRUE`).
    ComputeCond {
        cond: u8,
        op: ComputeOp,
    },
    /// Unconditional compute (`T2b` / `T2c`): runs on both PEs when SIMD is enabled.
    ComputeBoth {
        op: ComputeOp,
    },
    ModifyImm,
    StackOp,
    DoUntil,
    Type25,
    ShiftImm,
    ShiftMove,
    DualMove,
    Flow,
    Move,
    Unimplemented,
}

impl DecodedOp {
    /// Decodes a classified instruction and its 48-bit word into a [`DecodedOp`].
    #[inline]
    pub(super) fn from_insn(insn: &Insn, w: u64) -> Self {
        match insn.kind {
            Kind::T21c if insn.parcels[0] == 0x0001 => Self::Nop,
            Kind::T21c if insn.parcels[0] | 0x10 == 0x0091 => Self::Idle,
            Kind::T21 if w & !(1 << 36) == 0x0080_0000_0000 => Self::Idle,
            Kind::T21 if w == 0 => Self::Nop,
            Kind::T19a => Self::ModifyImm,
            Kind::T20a => Self::StackOp,
            Kind::T13a => Self::DoUntil,
            Kind::T25 => Self::Type25,
            Kind::T2a => Self::ComputeCond {
                cond: bits(w, 37, 33) as u8,
                op: ComputeOp::decode(bits(w, 22, 0)),
            },
            Kind::T2b => Self::ComputeBoth {
                op: ComputeOp::decode(bits(w, 38, 16)),
            },
            Kind::T2c => {
                let c = bits(w, 43, 32);
                let (n, x) = (bits(c, 7, 4), bits(c, 3, 0));
                let op = match short_op(bits(c, 11, 8)) {
                    Some(s) => {
                        let (xs, ys) = if s.binary { (n, x) } else { (x, 0) };
                        ComputeOp::decode(
                            (s.unit << 20) | (s.opcode << 12) | (n << 8) | (xs << 4) | ys,
                        )
                    }
                    None => ComputeOp::Invalid,
                };
                Self::ComputeBoth { op }
            }
            Kind::T6n => Self::ShiftImm,
            Kind::T6a => Self::ShiftMove,
            Kind::T1a | Kind::T1b => Self::DualMove,
            Kind::T8a | Kind::T9a | Kind::T9b | Kind::T11a | Kind::T11c | Kind::T12a => Self::Flow,
            Kind::T17a
            | Kind::T17b
            | Kind::T14a
            | Kind::T15a
            | Kind::T15b
            | Kind::T16a
            | Kind::T16b
            | Kind::T18a
            | Kind::T5a
            | Kind::T5b
            | Kind::T3a
            | Kind::T3b
            | Kind::T3c
            | Kind::T4a
            | Kind::T4b
            | Kind::T7a
            | Kind::T7b => Self::Move,
            _ => Self::Unimplemented,
        }
    }
}

/// One cached instruction entry in [`DecodeCache`].
#[derive(Clone, Copy)]
pub(super) struct CachedInsn {
    pub pc: u32,
    pub gen: u64,
    pub next_pc: u32,
    pub isa: bool,
    pub cond: u8,
    pub insn: Insn,
    pub w: u64,
    pub op: DecodedOp,
}

impl Default for CachedInsn {
    fn default() -> Self {
        Self {
            pc: 0,
            gen: 0,
            next_pc: 0,
            isa: false,
            cond: 31,
            insn: Insn {
                kind: Kind::Unknown,
                parcels: [0; 3],
                len: 0,
            },
            w: 0,
            op: DecodedOp::Unimplemented,
        }
    }
}

/// Direct-mapped instruction decode cache keyed by `pc & DECODE_CACHE_MASK`.
pub(super) struct DecodeCache {
    slots: Box<[CachedInsn; DECODE_CACHE_SIZE]>,
}

impl DecodeCache {
    pub(super) fn new() -> Box<Self> {
        Box::new(Self {
            slots: vec![CachedInsn::default(); DECODE_CACHE_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!()),
        })
    }

    #[inline]
    pub(super) fn get(&self, pc: u32, gen: u64, isa: bool) -> Option<&CachedInsn> {
        let slot = &self.slots[(pc as usize) & DECODE_CACHE_MASK];
        if slot.gen == gen && slot.pc == pc && slot.isa == isa {
            Some(slot)
        } else {
            None
        }
    }

    #[inline]
    pub(super) fn insert(&mut self, entry: CachedInsn) {
        self.slots[(entry.pc as usize) & DECODE_CACHE_MASK] = entry;
    }

    /// Invalidates slots whose instruction could overlap short-word range `first_short..first_short + count`.
    #[inline]
    pub(super) fn invalidate_sram_range(&mut self, first_short: u32, count: u32) {
        let start = first_short.saturating_sub(2);
        let end = first_short.saturating_add(count);
        for s in start..end {
            self.slots[(s as usize) & DECODE_CACHE_MASK].gen = 0;
        }
    }
}

impl Core {
    /// Invalidates cached instructions overlapping `first_short..first_short + count`.
    #[inline]
    pub(super) fn invalidate_sram_cache(&mut self, first_short: u32, count: u32) {
        if let Some(cache) = self.decode_cache.as_deref_mut() {
            cache.invalidate_sram_range(first_short, count);
        }
    }

    /// Invalidates the entire decode cache in O(1) by advancing the code generation.
    #[inline]
    pub(super) fn invalidate_code_cache(&mut self) {
        self.code_gen = self.code_gen.wrapping_add(1).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::super::support::core_with;
    use super::*;

    #[test]
    fn decode_op_extracts_compute_and_short_compute_fields() {
        // T2a: IF TRUE R3 = R1 + R2 -> 0x013e, 0x0000, 0x1312
        let t2a = Insn {
            kind: Kind::T2a,
            parcels: [0x013e, 0x0000, 0x1312],
            len: 3,
        };
        let w = crate::visa::word48(&t2a.parcels);
        assert_eq!(
            DecodedOp::from_insn(&t2a, w),
            DecodedOp::ComputeCond {
                cond: 31,
                op: ComputeOp::FixedAlu {
                    op: 0x01,
                    n: 3,
                    x: 1,
                    y: 2,
                },
            }
        );

        // T2c: R1 = R1 - R2 (short op 1, n=1, x=2 -> 0x0112)
        let t2c = Insn {
            kind: Kind::T2c,
            parcels: [0xc112, 0, 0],
            len: 1,
        };
        let w_short = crate::visa::word48(&t2c.parcels[..1]);
        assert_eq!(
            DecodedOp::from_insn(&t2c, w_short),
            DecodedOp::ComputeBoth {
                op: ComputeOp::FixedAlu {
                    op: 0x02,
                    n: 1,
                    x: 1,
                    y: 2,
                },
            }
        );
    }

    #[test]
    fn decode_cache_hits_and_invalidates_on_sram_write() {
        // Two NOPs at CODE, then overwrite the second NOP with a faultable/different word via dm_write.
        let mut c = core_with(&[0x0001, 0x0001, 0x0001]);
        assert_eq!(c.step(), Ok(()));
        let gen_before = c.code_gen;
        assert!(c
            .decode_cache
            .as_ref()
            .unwrap()
            .get(c.pc - 1, gen_before, false)
            .is_some());

        // Overwrite short word CODE + 1 via normal-word dm_write at CODE / 2.
        c.dm_write(c.pc / 2, 0x0001_0001).unwrap();
        assert!(c
            .decode_cache
            .as_ref()
            .unwrap()
            .get(c.pc - 1, gen_before, false)
            .is_none());
    }

    #[test]
    fn decode_cache_invalidates_on_isa_reload() {
        let mut c = Core::new(&[]);
        let g0 = c.code_gen;
        c.load_isa(&[0u8; 6]);
        assert_ne!(c.code_gen, g0);
    }
}
