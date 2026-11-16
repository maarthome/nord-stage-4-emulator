//! Level-3 static AOT transpiler (`DecodedOp` -> Rust source).
//!
//! Translates straight-line blocks and `DO ... UNTIL LCE` counter loops from [`DecodedOp`] into
//! standalone Rust code so `rustc` / LLVM can inline SRAM accesses, promote `r`/`i`/`m` into
//! physical CPU registers, and eliminate dead `ASTAT` flag updates.
use super::decode_cache::{ComputeOp, DecodedOp, FlowOp, MoveOp};
use super::Core;
use std::fmt::Write;

/// Options controlling Level-3 Rust code generation.
#[derive(Debug, Clone)]
pub struct AotOptions {
    /// Emit native IEEE-754 `f32` arithmetic for `FloatMul` and `FADD`/`FSUB` instead of calling
    /// 40-bit software helpers.
    pub fast_float: bool,
    /// Name of the generated Rust function.
    pub fn_name: String,
}

impl Default for AotOptions {
    fn default() -> Self {
        Self {
            fast_float: false,
            fn_name: "sharc_aot_routine".to_string(),
        }
    }
}

fn emit_compute_rust(
    out: &mut String,
    indent: &str,
    op: ComputeOp,
    fast_float: bool,
) -> Result<(), String> {
    match op {
        ComputeOp::Nop => Ok(()),
        ComputeOp::FixedAlu { op: fop, n, x, y } => match fop {
            0x01 => {
                let _ = writeln!(out, "{indent}let (rx, ry) = (core.r[{x}], core.r[{y}]);");
                let _ = writeln!(out, "{indent}let res = rx.wrapping_add(ry);");
                let _ = writeln!(out, "{indent}core.r[{n}] = res;");
                let _ = writeln!(
                    out,
                    "{indent}core.flags.az = res == 0; core.flags.an = (res as i32) < 0;"
                );
                Ok(())
            }
            0x02 => {
                let _ = writeln!(out, "{indent}let (rx, ry) = (core.r[{x}], core.r[{y}]);");
                let _ = writeln!(out, "{indent}let res = rx.wrapping_sub(ry);");
                let _ = writeln!(out, "{indent}core.r[{n}] = res;");
                let _ = writeln!(
                    out,
                    "{indent}core.flags.az = res == 0; core.flags.an = (res as i32) < 0;"
                );
                Ok(())
            }
            0x21 => {
                let _ = writeln!(out, "{indent}let res = core.r[{x}];");
                let _ = writeln!(out, "{indent}core.r[{n}] = res;");
                let _ = writeln!(
                    out,
                    "{indent}core.flags.az = res == 0; core.flags.an = (res as i32) < 0;"
                );
                Ok(())
            }
            0x40..=0x42 => {
                let sym = match fop {
                    0x40 => "&",
                    0x41 => "|",
                    _ => "^",
                };
                let _ = writeln!(out, "{indent}let res = core.r[{x}] {sym} core.r[{y}];");
                let _ = writeln!(out, "{indent}core.r[{n}] = res;");
                let _ = writeln!(
                    out,
                    "{indent}core.flags.az = res == 0; core.flags.an = (res as i32) < 0;"
                );
                Ok(())
            }
            other => Err(format!("unsupported FixedAlu opcode 0x{other:02x} in AOT")),
        },
        ComputeOp::FloatMul { n, x, y } if fast_float => {
            let _ = writeln!(
                out,
                "{indent}let res = (f32::from_bits(core.r[{x}]) * f32::from_bits(core.r[{y}])).to_bits();"
            );
            let _ = writeln!(out, "{indent}core.r[{n}] = res;");
            let _ = writeln!(out, "{indent}core.flags.az = f32::from_bits(res) == 0.0; core.flags.an = (res as i32) < 0;");
            Ok(())
        }
        ComputeOp::FloatAlu {
            op: fop @ (0x81 | 0x82),
            n,
            x,
            y,
        } if fast_float => {
            let sym = if fop == 0x81 { "+" } else { "-" };
            let _ = writeln!(
                out,
                "{indent}let res = (f32::from_bits(core.r[{x}]) {sym} f32::from_bits(core.r[{y}])).to_bits();"
            );
            let _ = writeln!(out, "{indent}core.r[{n}] = res;");
            let _ = writeln!(out, "{indent}core.flags.az = f32::from_bits(res) == 0.0; core.flags.an = (res as i32) < 0;");
            Ok(())
        }
        ComputeOp::FloatMul { n, x, y } => {
            let _ = writeln!(out, "{indent}core.float_mul_public({n}, {x}, {y});");
            Ok(())
        }
        other => Err(format!("unsupported ComputeOp {other:?} in AOT")),
    }
}

impl Core {
    /// Exposes 40-bit `float_mul` for AOT-generated routines when `fast_float` is false.
    pub fn float_mul_public(&mut self, n: usize, x: usize, y: usize) {
        self.float_mul(n, x, y);
    }

    /// Transpiles up to `max_insns` instructions starting at `start_pc` into a standalone Rust
    /// function operating on `Core` and its flat `sram` slice.
    pub fn emit_aot_rust(
        &self,
        start_pc: u32,
        max_insns: usize,
        opts: &AotOptions,
    ) -> Result<String, String> {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "pub fn {}(core: &mut nord_sharc::core::Core, sram: &mut [u32]) {{",
            opts.fn_name
        );
        let mut pc = start_pc;
        let mut count = 0usize;
        let mut loop_end_stack: Vec<u32> = Vec::new();

        while count < max_insns {
            let d = self.fetch(pc);
            let next_pc = pc.wrapping_add(if self.isa_at(pc) {
                1
            } else {
                d.insn.len as u32
            });
            let op = DecodedOp::from_insn(pc, &d.insn, d.w);
            let indent = if loop_end_stack.is_empty() {
                "    "
            } else {
                "        "
            };

            match op {
                DecodedOp::Nop => {
                    let _ = writeln!(out, "{indent}// 0x{pc:x}: nop");
                }
                DecodedOp::ComputeCond { cond: 31, op } | DecodedOp::ComputeBoth { op } => {
                    emit_compute_rust(&mut out, indent, op, opts.fast_float)?;
                }
                DecodedOp::Move(MoveOp::LoadImm { ureg, value }) => {
                    let idx = ureg & 15;
                    match ureg >> 4 {
                        0 => {
                            let _ = writeln!(out, "{indent}core.r[{idx}] = 0x{value:08x};");
                        }
                        1 => {
                            let _ = writeln!(out, "{indent}core.i[{idx}] = 0x{value:08x};");
                        }
                        2 => {
                            let _ = writeln!(out, "{indent}core.m[{idx}] = 0x{value:08x};");
                        }
                        3 => {
                            let _ = writeln!(out, "{indent}core.l[{idx}] = 0x{value:08x};");
                        }
                        _ => {
                            let _ =
                                writeln!(out, "{indent}core.write_ureg({ureg}, 0x{value:08x});");
                        }
                    }
                }
                DecodedOp::Move(MoveOp::RegMove {
                    src,
                    dst,
                    cond: 31,
                    compute: None,
                }) if src < 16 && dst < 16 => {
                    let _ = writeln!(out, "{indent}core.r[{dst}] = core.r[{src}];");
                }
                DecodedOp::Move(MoveOp::DagMem {
                    ii,
                    mi,
                    ureg,
                    write,
                    post_modify,
                    cond: 31,
                    compute: None,
                }) if ureg < 16 => {
                    let addr_expr = if post_modify {
                        format!("core.i[{ii}]")
                    } else {
                        format!("core.i[{ii}].wrapping_add(core.m[{mi}])")
                    };
                    let _ = writeln!(out, "{indent}let addr = {addr_expr} as usize;");
                    if write {
                        let _ = writeln!(out, "{indent}sram[addr] = core.r[{ureg}];");
                    } else {
                        let _ = writeln!(out, "{indent}core.r[{ureg}] = sram[addr];");
                    }
                    if post_modify {
                        let _ = writeln!(
                            out,
                            "{indent}core.i[{ii}] = core.i[{ii}].wrapping_add(core.m[{mi}]);"
                        );
                    }
                }
                DecodedOp::Flow(FlowOp::DoUntilCounter {
                    end,
                    count_ureg,
                    imm_count,
                }) => {
                    let cnt_expr = match count_ureg {
                        Some(u) if u < 16 => format!("core.r[{u}]"),
                        Some(u) => format!("core.read_ureg({u})"),
                        None => format!("{imm_count}"),
                    };
                    let _ = writeln!(out, "{indent}for _ in 0..{cnt_expr} {{");
                    loop_end_stack.push(end);
                }
                DecodedOp::Flow(FlowOp::Return {
                    cond: 31,
                    rti: false,
                    delayed: false,
                    compute: None,
                    ..
                }) => {
                    let _ = writeln!(out, "{indent}return;");
                    break;
                }
                other => return Err(format!("unsupported AOT op at 0x{pc:x}: {other:?}")),
            }

            if loop_end_stack.last().copied() == Some(pc) {
                loop_end_stack.pop();
                let close_indent = if loop_end_stack.is_empty() {
                    "    "
                } else {
                    "        "
                };
                let _ = writeln!(out, "{close_indent}}}");
            }
            pc = next_pc;
            count += 1;
        }
        while !loop_end_stack.is_empty() {
            loop_end_stack.pop();
            let _ = writeln!(out, "    }}");
        }
        let _ = writeln!(out, "}}");
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::support::core_with;
    use super::*;

    #[test]
    fn aot_emits_rust_for_do_loop_with_sram_load_add_and_fast_float() {
        // DO (+8) UNTIL LCE, LCNTR=R5; R0=DM(I2,M0); R1=R1+R0; F3=F1*F2; RTS
        let program = [
            0x0d05u16, 0x0000, 0x0007, // 0..3: DO (+7 -> 0x120007)
            0x9410, // 3..4: R0 = DM(I2, M0)
            0x013e, 0x0000, 0x1110, // 4..7: R1 = R1 + R0
            0x013e, 0x0013, 0x0312, // 7..10: F3 = F1 * F2
            0x0abe, // 10..11: RTS
        ];
        let c = core_with(&program);
        let src = c
            .emit_aot_rust(
                c.pc,
                5,
                &AotOptions {
                    fast_float: true,
                    fn_name: "dsp_kernel".to_string(),
                },
            )
            .expect("emit AOT Rust");
        assert!(src.contains("pub fn dsp_kernel("), "{src}");
        assert!(src.contains("for _ in 0..core.r[5] {"), "{src}");
        assert!(src.contains("core.r[0] = sram[addr];"), "{src}");
        assert!(src.contains("core.r[1] = res;"), "{src}");
        assert!(
            src.contains("f32::from_bits(core.r[1]) * f32::from_bits(core.r[2])"),
            "{src}"
        );
        assert!(src.contains("return;"), "{src}");
    }
}
