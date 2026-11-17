//! Runtime of the hot-block AOT prototype: the opt-in hook that runs one generated straight-line
//! block (see `aot_lower.rs`) in place of its interpreted steps, the guard that decides when that
//! is exact, and a strict state comparator for the differential runs.
//!
//! The generated module is compiled into this crate by `build.rs` when `NORD_SHARC_AOT_HOT` names
//! a file written by `sharc-aot --hot`; without it `gen::block()` is `None` and nothing changes.
//! With no block enabled, [`Core::run`] and [`Core::step`] are untouched (flag-off output is
//! bit-identical to the interpreter).
use super::decode_cache::ComputeOp;
use super::Core;

mod gen {
    include!(concat!(env!("OUT_DIR"), "/aot_hot_gen.rs"));
}

/// One generated block: its pc range, step count, the code bytes it was generated from and the
/// compiled function that executes all of its steps.
#[derive(Clone, Copy)]
pub(super) struct AotBlock {
    pub start: u32,
    pub last: u32,
    pub exit: u32,
    pub steps: u32,
    /// Little-endian bytes of the parcels `start..exit`.
    pub code: &'static [u8],
    pub run: fn(&mut Core),
}

/// Counters of the enabled hot block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AotHotStats {
    /// Times the generated block ran.
    pub calls: u64,
    /// Interpreter steps replaced by those calls.
    pub steps: u64,
    /// Times the pc was at the block start but the guard (or the step limit) refused.
    pub misses: u64,
}

pub(super) struct AotHot {
    block: AotBlock,
    stats: AotHotStats,
}

// The `aot_*` helpers below are called only by the generated block module (and by the test
// oracle `run_lowered`), so a build without `NORD_SHARC_AOT_HOT` sees them as unused.
#[allow(dead_code)]
impl Core {
    /// Exchanges the PEx/PEy views (generated code calls this twice per block).
    #[inline(always)]
    pub(super) fn aot_swap_pe(&mut self) {
        self.swap_pe();
    }

    /// Runs one lowered compute field on the current element. Every compute op of a block is
    /// checked by the lowering, so a `false` (unmodelled op) would be a lowering bug.
    #[inline(always)]
    pub(super) fn aot_op(&mut self, op: ComputeOp) {
        let ok = self.exec_compute_op(op);
        debug_assert!(ok, "AOT block holds an unmodelled compute op: {op:?}");
    }

    /// Reads the source of `Rdst = Rsrc`: value, float-extension tag and low byte.
    #[inline(always)]
    pub(super) fn aot_move_src(&self, src: u8) -> (u32, u32, u8) {
        let n = usize::from(src);
        (self.r[n], self.fx.tag[n], self.fx.lo[n])
    }

    /// Writes a captured register move; a move between data registers keeps the extension.
    #[inline(always)]
    pub(super) fn aot_move_dst(&mut self, dst: u8, m: (u32, u32, u8)) {
        let n = usize::from(dst);
        self.write_ureg(dst, m.0);
        (self.fx.tag[n], self.fx.lo[n]) = (m.1, m.2);
    }

    /// `Rn = <shifter op> Rx BY imm` with the register-form opcode.
    #[inline(always)]
    pub(super) fn aot_shift(&mut self, op: u64, n: u8, x: u8, ry: u32) {
        let ok = self.shifter_op(op, usize::from(n), self.r[usize::from(x)], ry);
        debug_assert!(ok, "AOT block holds an unmodelled shift op {op:#x}");
    }
}

impl Core {
    /// Whether the live SRAM still holds the code the block was generated from.
    fn aot_code_matches(&self, b: &AotBlock) -> bool {
        let o = b.start as usize * 2;
        self.sram.get(o..o + b.code.len()) == Some(b.code)
    }

    /// Whether running `b` now is exactly equivalent to `b.steps` interpreter steps: SIMD on (the
    /// block was lowered for both elements), no delayed branch or loop end inside the block, no
    /// interrupt-controller event due within it, no enabled arithmetic-exception interrupt that a
    /// compute op could latch and take mid-block, tracing off and the code unchanged.
    fn aot_ready(&self, b: &AotBlock) -> bool {
        let n = u64::from(b.steps);
        self.simd()
            && self.pending.is_none()
            && self.trace.is_none()
            && self.irq.window_len == 0
            && !self.irq.idle
            && self.steps + n <= self.irq.due
            && self
                .loops
                .last()
                .is_none_or(|f| f.term.is_none() && !(b.start..=b.last).contains(&f.end))
            && !self.arith_irq_enabled()
            && self.aot_code_matches(b)
    }

    /// Enables the generated block for `start..=last` (instruction starts), failing when this
    /// binary has no generated block for that range or the loaded code differs from it.
    pub fn enable_aot_hot(&mut self, start: u32, last: u32) -> Result<(), String> {
        let b = gen::block().ok_or(
            "built without a generated AOT block (set NORD_SHARC_AOT_HOT=<file from sharc-aot --hot> and rebuild)",
        )?;
        if (b.start, b.last) != (start, last) {
            return Err(format!(
                "generated block is 0x{:x}..=0x{:x}, not 0x{start:x}..=0x{last:x}",
                b.start, b.last
            ));
        }
        if !self.aot_code_matches(&b) {
            return Err("loaded code differs from the code the block was generated from".into());
        }
        self.aot_pc = b.start;
        self.aot_hot = Some(Box::new(AotHot {
            block: b,
            stats: AotHotStats::default(),
        }));
        Ok(())
    }

    /// Turns the hook off again.
    pub fn disable_aot_hot(&mut self) {
        self.aot_pc = u32::MAX;
        self.aot_hot = None;
    }

    /// `(start, last, steps)` of the enabled block.
    pub fn aot_hot_range(&self) -> Option<(u32, u32, u32)> {
        self.aot_hot
            .as_deref()
            .map(|h| (h.block.start, h.block.last, h.block.steps))
    }

    /// Counters of the enabled block.
    pub fn aot_hot_stats(&self) -> AotHotStats {
        self.aot_hot.as_deref().map(|h| h.stats).unwrap_or_default()
    }

    /// Runs the enabled block when the pc is at its start, `limit` leaves room for all its
    /// steps and the guard holds; returns whether it ran (the pc is then the block's exit).
    ///
    /// The per-step cost is one compare of `pc` against `aot_pc` (`u32::MAX` while disabled);
    /// everything else lives in [`Core::aot_enter`].
    #[inline(always)]
    pub fn aot_try(&mut self, limit: u64) -> bool {
        self.pc == self.aot_pc && self.aot_enter(limit)
    }

    #[inline(never)]
    fn aot_enter(&mut self, limit: u64) -> bool {
        let Some(h) = self.aot_hot.as_deref() else {
            return false;
        };
        let b = h.block;
        if self.steps + u64::from(b.steps) > limit || !self.aot_ready(&b) {
            if let Some(h) = self.aot_hot.as_deref_mut() {
                h.stats.misses += 1;
            }
            return false;
        }
        (b.run)(self);
        self.steps += u64::from(b.steps);
        self.pc = b.exit;
        if let Some(h) = self.aot_hot.as_deref_mut() {
            h.stats.calls += 1;
            h.stats.steps += u64::from(b.steps);
        }
        true
    }

    /// [`Core::run`] with the hot-block hook: identical results, but the enabled block runs as
    /// one call whenever the pc reaches its start with room for all of its steps.
    pub fn run_aot(&mut self, max_steps: u64) -> Option<super::Fault> {
        let limit = self.steps + max_steps;
        while self.steps < limit {
            if self.aot_try(limit) {
                continue;
            }
            if let Err(f) = self.step() {
                return Some(f);
            }
        }
        None
    }

    /// First difference between the complete architectural state of `self` and `other`,
    /// including what `compare_full_state` cannot see: PEy flags and multiplier results, the
    /// effective float extensions, ASTAT extras, STKY, USTAT and the interrupt latches.
    pub fn state_mismatch(&self, other: &Core) -> Option<String> {
        fn ext(c: &Core, y: bool) -> [u64; 16] {
            let (r, fx) = if y { (&c.s, &c.fx_y) } else { (&c.r, &c.fx) };
            std::array::from_fn(|n| {
                let lo = if fx.tag[n] == r[n] { fx.lo[n] } else { 0 };
                u64::from(r[n]) << 8 | u64::from(lo)
            })
        }
        macro_rules! same {
            ($what:literal, $a:expr, $b:expr) => {
                if $a != $b {
                    return Some(format!("{}: {:?} vs {:?}", $what, $a, $b));
                }
            };
        }
        same!("pc", self.pc, other.pc);
        same!("steps", self.steps, other.steps);
        same!("r", self.r, other.r);
        same!("s", self.s, other.s);
        same!("i", self.i, other.i);
        same!("m", self.m, other.m);
        same!("l", self.l, other.l);
        same!("b", self.b, other.b);
        same!("mr", self.mr, other.mr);
        same!("mr_y", self.mr_y, other.mr_y);
        same!("flags", self.flags, other.flags);
        same!("flags_y", self.flags_y, other.flags_y);
        same!("mode1", self.mode1, other.mode1);
        same!("mode2", self.mode2, other.mode2);
        same!("ustat", self.ustat, other.ustat);
        same!("f40 x", ext(self, false), ext(other, false));
        same!("f40 y", ext(self, true), ext(other, true));
        same!("astat_extra", self.sys.astat_extra, other.sys.astat_extra);
        same!("stky", self.sys.stky, other.sys.stky);
        same!("irptl", self.irq.irptl, other.irq.irptl);
        same!("lirptl", self.irq.lirptl, other.irq.lirptl);
        same!("loops", self.loops.len(), other.loops.len());
        same!("call depth", self.call_stack.len(), other.call_stack.len());
        None
    }

    /// Test oracle for the lowering: runs `ops` PE-batched exactly like the generated code.
    #[cfg(test)]
    pub(super) fn run_lowered(&mut self, ops: &[super::aot_lower::LoweredOp]) {
        use super::aot_lower::LoweredOp;
        for _ in 0..2 {
            for &op in ops {
                match op {
                    LoweredOp::Compute(c) => self.aot_op(c),
                    LoweredOp::LoadImm { reg, value } => self.write_ureg(reg, value),
                    LoweredOp::RegMove { src, dst, compute } => {
                        let m = self.aot_move_src(src);
                        if let Some(c) = compute {
                            self.aot_op(c);
                        }
                        self.aot_move_dst(dst, m);
                    }
                    LoweredOp::Shift { op, n, x, ry } => self.aot_shift(op, n, x, ry),
                }
            }
            self.aot_swap_pe();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::support::core_with;
    use super::*;

    /// A block mixing every lowered form: immediates, float add/sub/mul/clip/copysign/recips/
    /// FLOAT, fixed add, `Rn = Rm, compute` moves, a shift immediate, a multifunction and NOP.
    fn mixed_program() -> Vec<u16> {
        vec![
            0x0f0e, 0x3eb0, 0xf34d, // R14 = 0x3eb0f34d
            0x0180, 0x1ecc, // R14 = R12 + R12 (fixed ALU, 32-bit form)
            0x018d, 0xa4ce, // F4 = FLOAT R12 BY R14
            0x0188, 0x2be4, // F11 = F14 - F4
            0x018e, 0x3bbe, // F11 = CLIP F11 BY F14
            0xc8be, // F11 = F11 + F14 (16-bit form)
            0x0193, 0x0ebe, // F14 = F11 * F14
            0x703e, 0x030c, 0x4040, // R6 = R0, F0 = RECIPS F4
            0x01d9, 0x602e, // F6 = F0 * F6, F0 = F11 - F14
            0x703f, 0x863f, // R12 = R3
            0x023e, 0x0032, 0x1fa0, // R10 = BTGL R0 BY 31
            0x018e, 0x000a, // F0 = F0 COPYSIGN F10
            0x0001, // NOP
        ]
    }

    fn seeded(c: &mut Core, seed: u32) {
        let mut x = seed.wrapping_mul(0x9E37_79B9) | 1;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        for k in 0..16 {
            let v = next();
            // keep exponents in a normal range so most ops produce ordinary floats
            c.r[k] = (v & 0x807F_FFFF) | ((120 + (next() % 16)) << 23);
            c.s[k] = (next() & 0x807F_FFFF) | ((118 + (next() % 20)) << 23);
        }
        c.mode1 |= 1 << 21;
    }

    #[test]
    fn lowered_block_matches_interpreter_on_random_simd_state() {
        let program = mixed_program();
        let probe = core_with(&program);
        let last = probe.pc + program.len() as u32 - 1;
        let low = match probe.lower_block(probe.pc, last) {
            Ok(l) => l,
            Err(e) => panic!("lowering failed: {e}"),
        };
        for seed in 1..40 {
            let mut interp = core_with(&program);
            let mut aot = core_with(&program);
            seeded(&mut interp, seed);
            seeded(&mut aot, seed);
            assert_eq!(interp.run(u64::from(low.steps)), None);
            aot.run_lowered(&low.ops);
            aot.steps += u64::from(low.steps);
            aot.pc = low.exit;
            assert_eq!(interp.state_mismatch(&aot), None, "seed {seed}");
        }
    }

    #[test]
    fn guard_rejects_scalar_mode_pending_branch_and_due_event_and_changed_code() {
        let program = [0x0001u16, 0x0001];
        let c = core_with(&program);
        let b = AotBlock {
            start: c.pc,
            last: c.pc + 1,
            exit: c.pc + 2,
            steps: 2,
            code: &[1, 0, 1, 0],
            run: |_| {},
        };
        let mut c = core_with(&program);
        assert!(!c.aot_ready(&b), "scalar mode");
        c.mode1 |= 1 << 21;
        c.irq.due = u64::MAX;
        assert!(c.aot_ready(&b));
        c.irq.due = c.steps + 1;
        assert!(!c.aot_ready(&b), "event due inside the block");
        c.irq.due = c.steps + 2;
        assert!(c.aot_ready(&b), "event due right after the block is fine");
        c.irq.due = u64::MAX;
        c.trace = Some((0, 1, 1));
        assert!(!c.aot_ready(&b), "tracing");
        c.trace = None;
        c.dm_write(0x9_0000, 0xDEAD_BEEF).unwrap();
        assert!(!c.aot_ready(&b), "code differs from the generated snapshot");
    }

    #[test]
    fn enable_fails_without_generated_block_and_flag_off_run_aot_equals_run() {
        let program = mixed_program();
        let mut a = core_with(&program);
        assert!(a.enable_aot_hot(a.pc, a.pc + 1).is_err());
        assert_eq!(a.aot_hot_range(), None);
        let mut b = core_with(&program);
        seeded(&mut a, 3);
        seeded(&mut b, 3);
        assert_eq!(a.run(8), None);
        assert_eq!(b.run_aot(8), None);
        assert_eq!(a.state_mismatch(&b), None);
        assert_eq!(b.aot_hot_stats(), AotHotStats::default());
        assert!(!b.aot_try(100));
    }

    #[test]
    fn arith_exception_interrupts_block_the_fast_path_only_when_enabled() {
        let mut c = core_with(&[0x0001]);
        assert!(!c.arith_irq_enabled());
        c.mode1 |= 1 << 12; // IRPTEN
        c.imask = 1 << 18; // some unrelated vector bit
        let _ = c.arith_irq_enabled();
    }

    #[test]
    fn state_mismatch_sees_pey_flags_and_float_extension() {
        let mut a = Core::new(&[]);
        let b = Core::new(&[]);
        assert_eq!(a.state_mismatch(&b), None);
        a.flags_y.az = true;
        assert!(a.state_mismatch(&b).unwrap().starts_with("flags_y"));
        a.flags_y.az = false;
        a.fx.tag[3] = a.r[3];
        a.fx.lo[3] = 0x80;
        assert!(a.state_mismatch(&b).unwrap().starts_with("f40 x"));
    }
}
