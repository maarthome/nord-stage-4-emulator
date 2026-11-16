//! Whole-block lowering for the hot-block AOT prototype: a straight-line SIMD range of
//! [`DecodedOp`]s becomes a list of [`LoweredOp`]s (one per instruction, registers and compute
//! fields resolved at compile time) that [`emit_block_rust`] prints as Rust source.
//!
//! Every lowered op acts on ONE processing element (PE) and never reads the other, so the
//! generated function runs the whole block on PEx, swaps the PEx/PEy views once, runs it again
//! and swaps back, instead of swapping around every instruction like [`Core::step`] does. That
//! reordering is exact because compute ops, immediate loads, `Rn = Rm [, compute]` moves and
//! shift immediates only touch registers, flags, multiplier results, float extensions, ASTAT
//! extras and STKY of their own element (see `docs/research/sharc-aot-hot-block.md`).
use super::aot_pe::{pe_supports, pe_supports_shift};
use super::decode_cache::{ComputeOp, DecodedOp, MoveOp};
use super::shifter::shift_imm_params;
use super::Core;
use crate::visa::bits;
use std::fmt::Write;

/// How the generated block executes its ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AotTier {
    /// The interpreter's own compute implementation per op, PE-batched (no decode, no per-step swap).
    Interp,
    /// [`AotTier::Interp`] with the native-width float kernels of `aot_fast.rs`.
    Fast,
    /// Register-resident per-element state and native kernels (`aot_pe.rs`).
    Pe,
    /// [`AotTier::Pe`] with host `f64` float kernels where the result is provably exact
    /// (`aot_native.rs`); still bit-exact.
    Guarded,
    /// [`AotTier::Pe`] with host `f32` kernels: NOT bit-exact (extension dropped, denormals
    /// flushed); only for the audio-metric comparison, never a replacement for the exact path.
    Approx,
}

/// One instruction of an AOT block, acting on a single processing element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoweredOp {
    /// Unconditional compute field (`T2a`/`T2b`/`T2c`).
    Compute(ComputeOp),
    /// `Rn = immediate` (the SIMD twin write is the other element's pass).
    LoadImm { reg: u8, value: u32 },
    /// `Rdst = Rsrc [, compute]`; the source and its float extension are read before the compute.
    RegMove {
        src: u8,
        dst: u8,
        compute: Option<ComputeOp>,
    },
    /// `Rn = <shifter op> Rx BY <immediate>` with the register-form opcode and the Ry value.
    Shift { op: u64, n: u8, x: u8, ry: u32 },
}

/// A lowered straight-line block and what the runtime guard needs to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Lowered {
    pub start: u32,
    /// Start pc of the last instruction.
    pub last: u32,
    /// pc after the last instruction.
    pub exit: u32,
    /// Instructions (= steps).
    pub steps: u32,
    /// Instruction parcels `start..exit`, to detect self-modified code at run time.
    pub code: Vec<u16>,
    pub ops: Vec<LoweredOp>,
}

fn lower_compute(op: ComputeOp) -> Result<ComputeOp, String> {
    match op {
        ComputeOp::Invalid => Err("invalid compute field".to_string()),
        other => Ok(other),
    }
}

impl Core {
    /// Lists `(pc, next_pc, Debug of the decoded op)` of each instruction starting in
    /// `start..=last` (diagnostic for choosing lowering coverage).
    pub fn aot_describe_range(&self, start: u32, last: u32) -> Vec<(u32, u32, String)> {
        let mut out = Vec::new();
        let mut pc = start;
        while pc <= last {
            let d = self.fetch(pc);
            let next = pc.wrapping_add(d.insn.len as u32);
            let op = DecodedOp::from_insn(pc, &d.insn, d.w);
            out.push((pc, next, format!("{op:?}")));
            pc = next;
        }
        out
    }

    /// Lowers the straight-line range whose instructions start in `start..=last`.
    pub(super) fn lower_block(&self, start: u32, last: u32) -> Result<Lowered, String> {
        if self.isa_at(start) {
            return Err("ISA-mode code is not lowered".to_string());
        }
        let (mut pc, mut ops, mut code) = (start, Vec::new(), Vec::new());
        let mut last_start = start;
        while pc <= last {
            let d = self.fetch(pc);
            let op = DecodedOp::from_insn(pc, &d.insn, d.w);
            let lowered = match op {
                DecodedOp::Nop => None,
                DecodedOp::ComputeCond { cond: 31, op } | DecodedOp::ComputeBoth { op } => {
                    Some(LoweredOp::Compute(lower_compute(op)?))
                }
                DecodedOp::Move(MoveOp::LoadImm { ureg, value }) if ureg < 16 => {
                    Some(LoweredOp::LoadImm { reg: ureg, value })
                }
                DecodedOp::Move(MoveOp::RegMove {
                    src,
                    dst,
                    cond: 31,
                    compute,
                }) if src < 16 && dst < 16 => Some(LoweredOp::RegMove {
                    src,
                    dst,
                    compute: compute.map(lower_compute).transpose()?,
                }),
                DecodedOp::ShiftImm if bits(d.w, 37, 33) == 31 => {
                    let (op, n, x, ry) = shift_imm_params(d.w)
                        .ok_or_else(|| format!("unmodelled shift immediate at 0x{pc:x}"))?;
                    Some(LoweredOp::Shift {
                        op,
                        n: n as u8,
                        x: x as u8,
                        ry,
                    })
                }
                other => return Err(format!("unsupported op at 0x{pc:x}: {other:?}")),
            };
            ops.extend(lowered);
            if lowered.is_none() {
                // a NOP still counts as a step; keep one slot so `steps` == instructions
                ops.push(LoweredOp::Compute(ComputeOp::Nop));
            }
            for k in 0..d.insn.len as u32 {
                code.push(self.parcel(pc.wrapping_add(k)));
            }
            last_start = pc;
            pc = pc.wrapping_add(d.insn.len as u32);
        }
        if last_start != last {
            return Err(format!(
                "range end 0x{last:x} is not an instruction start (last start 0x{last_start:x})"
            ));
        }
        Ok(Lowered {
            start,
            last,
            exit: pc,
            steps: ops.len() as u32,
            code,
            ops,
        })
    }
}

impl Core {
    /// Rust source of the AOT module for the straight-line range whose instructions start in
    /// `start..=last` (`sharc-aot --hot`); see [`Core::enable_aot_hot`] for how it is used.
    pub fn emit_hot_block_rust(
        &self,
        start: u32,
        last: u32,
        tier: AotTier,
    ) -> Result<String, String> {
        self.lower_block(start, last)
            .and_then(|b| emit_block_rust(&b, tier))
    }
}

fn compute_expr(op: ComputeOp) -> String {
    format!("{op:?}")
}

fn emit_op(out: &mut String, op: LoweredOp, native: bool) {
    let call = if native { "aot_op_native" } else { "aot_op" };
    let ind = "    ";
    match op {
        LoweredOp::Compute(ComputeOp::Nop) => {}
        LoweredOp::Compute(c) => {
            let _ = writeln!(out, "{ind}core.{call}(ComputeOp::{});", compute_expr(c));
        }
        LoweredOp::LoadImm { reg, value } => {
            let _ = writeln!(out, "{ind}core.write_ureg({reg}, 0x{value:08x});");
        }
        LoweredOp::RegMove { src, dst, compute } => {
            let _ = writeln!(out, "{ind}let m = core.aot_move_src({src});");
            if let Some(c) = compute {
                let _ = writeln!(out, "{ind}core.{call}(ComputeOp::{});", compute_expr(c));
            }
            let _ = writeln!(out, "{ind}core.aot_move_dst({dst}, m);");
        }
        LoweredOp::Shift { op, n, x, ry } => {
            let _ = writeln!(out, "{ind}core.aot_shift(0x{op:x}, {n}, {x}, 0x{ry:08x});");
        }
    }
}

/// Prints the Rust source of the generated block module: the constants and `run` function that
/// `aot_hot.rs` includes at build time (`NORD_SHARC_AOT_HOT=<file>`).
pub(super) fn emit_block_rust(b: &Lowered, tier: AotTier) -> Result<String, String> {
    if matches!(tier, AotTier::Pe | AotTier::Guarded | AotTier::Approx) {
        return emit_pe_block_rust(b, tier);
    }
    let native = tier == AotTier::Fast;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Generated by `sharc-aot --hot`; derived from firmware: never commit this file."
    );
    let _ = writeln!(out, "use crate::core::aot_hot::AotBlock;");
    let _ = writeln!(out, "use crate::core::decode_cache::ComputeOp;");
    let _ = writeln!(out, "use crate::core::Core;");
    let code: Vec<String> = b
        .code
        .iter()
        .flat_map(|p| p.to_le_bytes())
        .map(|byte| format!("0x{byte:02x}"))
        .collect();
    let _ = writeln!(
        out,
        "static CODE: [u8; {}] = [{}];",
        code.len(),
        code.join(", ")
    );
    let _ = writeln!(out, "#[inline(always)]");
    let _ = writeln!(out, "fn one_pe(core: &mut Core) {{");
    for &op in &b.ops {
        emit_op(&mut out, op, native);
    }
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "fn run(core: &mut Core) {{");
    let _ = writeln!(out, "    one_pe(core);");
    let _ = writeln!(out, "    core.aot_swap_pe();");
    let _ = writeln!(out, "    one_pe(core);");
    let _ = writeln!(out, "    core.aot_swap_pe();");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "pub(super) fn block() -> Option<AotBlock> {{");
    let _ = writeln!(
        out,
        "    Some(AotBlock {{ start: 0x{:x}, last: 0x{:x}, exit: 0x{:x}, steps: {}, code: &CODE, run }})",
        b.start, b.last, b.exit, b.steps
    );
    let _ = writeln!(out, "}}");
    Ok(out)
}

/// The common head of a generated module: header, imports and the code snapshot.
fn emit_head(out: &mut String, b: &Lowered, imports: &[&str]) {
    let _ = writeln!(
        out,
        "// Generated by `sharc-aot --hot`; derived from firmware: never commit this file."
    );
    for i in imports {
        let _ = writeln!(out, "use {i};");
    }
    let code: Vec<String> = b
        .code
        .iter()
        .flat_map(|p| p.to_le_bytes())
        .map(|byte| format!("0x{byte:02x}"))
        .collect();
    let _ = writeln!(
        out,
        "static CODE: [u8; {}] = [{}];",
        code.len(),
        code.join(", ")
    );
}

/// The `block()` constructor every generated module ends with.
fn emit_tail(out: &mut String, b: &Lowered) {
    let _ = writeln!(out, "pub(super) fn block() -> Option<AotBlock> {{");
    let _ = writeln!(
        out,
        "    Some(AotBlock {{ start: 0x{:x}, last: 0x{:x}, exit: 0x{:x}, steps: {}, code: &CODE, run }})",
        b.start, b.last, b.exit, b.steps
    );
    let _ = writeln!(out, "}}");
}

/// [`AotTier::Pe`]: each element is loaded into a [`Pe`](super::aot_pe::Pe), the block runs on
/// it and it is stored back; there is no swap of the whole register file.
fn emit_pe_block_rust(b: &Lowered, tier: AotTier) -> Result<String, String> {
    let mut out = String::new();
    emit_head(
        &mut out,
        b,
        &[
            "crate::core::aot_hot::AotBlock",
            "crate::core::aot_native::Kern",
            "crate::core::aot_pe::Pe",
            "crate::core::decode_cache::ComputeOp",
            "crate::core::Core",
        ],
    );
    let _ = writeln!(out, "#[inline(always)]");
    let _ = writeln!(out, "fn one_pe(pe: &mut Pe) {{");
    for &op in &b.ops {
        let ind = "    ";
        let check = |c: ComputeOp| {
            if pe_supports(c) {
                Ok(())
            } else {
                Err(format!("register-resident tier does not support {c:?}"))
            }
        };
        match op {
            LoweredOp::Compute(ComputeOp::Nop) => {}
            LoweredOp::Compute(c) => {
                check(c)?;
                let _ = writeln!(out, "{ind}pe.exec(ComputeOp::{});", compute_expr(c));
            }
            LoweredOp::LoadImm { reg, value } => {
                let _ = writeln!(out, "{ind}pe.load_imm({reg}, 0x{value:08x});");
            }
            LoweredOp::RegMove { src, dst, compute } => {
                let _ = writeln!(out, "{ind}let m = pe.move_src({src});");
                if let Some(c) = compute {
                    check(c)?;
                    let _ = writeln!(out, "{ind}pe.exec(ComputeOp::{});", compute_expr(c));
                }
                let _ = writeln!(out, "{ind}pe.move_dst({dst}, m);");
            }
            LoweredOp::Shift { op, n, x, ry } => {
                if !pe_supports_shift(op) {
                    return Err(format!(
                        "register-resident tier does not support shift op 0x{op:x}"
                    ));
                }
                let _ = writeln!(out, "{ind}pe.shift(0x{op:x}, {n}, {x}, 0x{ry:08x});");
            }
        }
    }
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "fn run(core: &mut Core) {{");
    for y in ["false", "true"] {
        let kern = match tier {
            AotTier::Guarded => "Guarded",
            AotTier::Approx => "Approx",
            _ => "Exact",
        };
        let _ = writeln!(
            out,
            "    let mut pe = Pe::load(core, {y}).with_kern(Kern::{kern});"
        );
        let _ = writeln!(out, "    one_pe(&mut pe);");
        let _ = writeln!(out, "    pe.store(core);");
    }
    let _ = writeln!(out, "}}");
    emit_tail(&mut out, b);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::aot_hot::AotBlock;
    use super::super::support::core_with;
    use super::*;

    /// R14 = 0x3f800000; F4 = F14 * F14 (FloatMul n=4); R1 = R14 (RegMove); NOP
    fn program() -> Vec<u16> {
        vec![
            0x0f0e, 0x3f80, 0x0000, // R14 = 0x3f800000 (48-bit load)
            0x013e, 0x0013, 0x04ee, // F4 = F14 * F14
            0x0001, // NOP
        ]
    }

    #[test]
    fn lowers_loadimm_float_mul_and_nop_with_code_snapshot() {
        let c = core_with(&program());
        let b = c.lower_block(c.pc, c.pc + 6).unwrap();
        assert_eq!(
            (b.start, b.last, b.exit, b.steps),
            (c.pc, c.pc + 6, c.pc + 7, 3)
        );
        assert_eq!(b.code.len(), 7);
        assert_eq!(
            b.ops[0],
            LoweredOp::LoadImm {
                reg: 14,
                value: 0x3f80_0000
            }
        );
        assert!(matches!(
            b.ops[1],
            LoweredOp::Compute(ComputeOp::FloatMul { n: 4, x: 14, y: 14 })
        ));
        assert_eq!(b.ops[2], LoweredOp::Compute(ComputeOp::Nop));
    }

    #[test]
    fn rejects_unsupported_ops_and_misaligned_range_end() {
        // JUMP is not straight-line code
        let c = core_with(&[0x073e, 0x00ff, 0xfffa]);
        assert!(c.lower_block(c.pc, c.pc).is_err());
        let c = core_with(&program());
        assert!(c.lower_block(c.pc, c.pc + 1).is_err(), "inside an insn");
    }

    #[test]
    fn emits_rust_with_one_pass_per_pe_and_a_single_swap_pair() {
        let c = core_with(&program());
        let b = c.lower_block(c.pc, c.pc + 6).unwrap();
        let src = emit_block_rust(&b, AotTier::Interp).unwrap();
        let native = emit_block_rust(&b, AotTier::Fast).unwrap();
        let pe = emit_block_rust(&b, AotTier::Pe).unwrap();
        assert!(pe.contains("pe.load_imm(14, 0x3f800000);"), "{pe}");
        assert!(
            pe.contains("pe.exec(ComputeOp::FloatMul { n: 4, x: 14, y: 14 });"),
            "{pe}"
        );
        assert_eq!(pe.matches("Pe::load(core,").count(), 2, "{pe}");
        assert!(pe.contains("with_kern(Kern::Exact)"), "{pe}");
        let guarded = emit_block_rust(&b, AotTier::Guarded).unwrap();
        assert!(guarded.contains("with_kern(Kern::Guarded)"), "{guarded}");
        let approx = emit_block_rust(&b, AotTier::Approx).unwrap();
        assert!(approx.contains("with_kern(Kern::Approx)"), "{approx}");
        assert!(native.contains("core.aot_op_native(ComputeOp::FloatMul { n: 4, x: 14, y: 14 });"));
        assert!(src.contains("core.write_ureg(14, 0x3f800000);"), "{src}");
        assert!(
            src.contains("core.aot_op(ComputeOp::FloatMul { n: 4, x: 14, y: 14 });"),
            "{src}"
        );
        assert_eq!(src.matches("one_pe(core);").count(), 2, "{src}");
        assert_eq!(src.matches("core.aot_swap_pe();").count(), 2, "{src}");
        assert!(src.contains("steps: 3"), "{src}");
    }

    #[test]
    fn describe_range_lists_each_instruction() {
        let c = core_with(&program());
        let rows = c.aot_describe_range(c.pc, c.pc + 6);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].2.contains("LoadImm"), "{rows:?}");
        assert_eq!(rows[2].1, c.pc + 7);
    }

    #[test]
    fn aot_block_is_plain_copy_data() {
        // the generated module builds `AotBlock` by value; keep it `Copy`
        fn is_copy<T: Copy>() {}
        is_copy::<AotBlock>();
    }
}
