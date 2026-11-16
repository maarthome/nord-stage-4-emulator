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

fn cond_expr_rust(cond: u8) -> Result<&'static str, String> {
    match cond {
        31 => Ok("true"),
        0 => Ok("core.flags.az"),
        16 => Ok("!core.flags.az"),
        1 => Ok("core.flags.an"),
        17 => Ok("!core.flags.an"),
        other => Err(format!("unsupported AOT condition code {other}")),
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
        let mut decoded_insns = Vec::with_capacity(max_insns);
        let mut pc = start_pc;
        let mut has_jump = false;
        for _ in 0..max_insns {
            let d = self.fetch(pc);
            let next_pc = pc.wrapping_add(if self.isa_at(pc) {
                1
            } else {
                d.insn.len as u32
            });
            let op = DecodedOp::from_insn(pc, &d.insn, d.w);
            if matches!(op, DecodedOp::Flow(FlowOp::JumpDirect { .. })) {
                has_jump = true;
            }
            let is_ret = matches!(
                op,
                DecodedOp::Flow(FlowOp::Return {
                    cond: 31,
                    rti: false,
                    delayed: false,
                    compute: None,
                    ..
                })
            );
            decoded_insns.push((pc, next_pc, op));
            pc = next_pc;
            if is_ret {
                break;
            }
        }

        let mut out = String::new();
        let _ = writeln!(
            out,
            "pub fn {}(core: &mut nord_sharc::core::Core, _sram: &mut [u32]) {{",
            opts.fn_name
        );
        if has_jump {
            let _ = writeln!(out, "    let mut pc: u32 = 0x{start_pc:x};");
            let _ = writeln!(out, "    loop {{");
            let _ = writeln!(out, "        match pc {{");
            for &(ipc, inext, op) in &decoded_insns {
                let _ = writeln!(out, "            0x{ipc:x} => {{");
                Self::emit_single_op_rust(
                    &mut out,
                    "                ",
                    ipc,
                    inext,
                    op,
                    opts,
                    true,
                )?;
                if !matches!(
                    op,
                    DecodedOp::Flow(FlowOp::JumpDirect { cond: 31, .. } | FlowOp::Return { .. })
                ) {
                    let _ = writeln!(out, "                pc = 0x{inext:x};");
                }
                let _ = writeln!(out, "            }}");
            }
            let _ = writeln!(out, "            _ => return,");
            let _ = writeln!(out, "        }}");
            let _ = writeln!(out, "    }}");
            let _ = writeln!(out, "}}");
            return Ok(out);
        }

        let mut loop_end_stack: Vec<u32> = Vec::new();
        for &(ipc, inext, op) in &decoded_insns {
            let indent = if loop_end_stack.is_empty() {
                "    "
            } else {
                "        "
            };
            match op {
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
                _ => {
                    Self::emit_single_op_rust(&mut out, indent, ipc, inext, op, opts, false)?;
                }
            }

            if loop_end_stack.last().copied() == Some(ipc) {
                loop_end_stack.pop();
                let close_indent = if loop_end_stack.is_empty() {
                    "    "
                } else {
                    "        "
                };
                let _ = writeln!(out, "{close_indent}}}");
            }
        }
        while !loop_end_stack.is_empty() {
            loop_end_stack.pop();
            let _ = writeln!(out, "    }}");
        }
        let _ = writeln!(out, "}}");
        Ok(out)
    }

    fn emit_single_op_rust(
        out: &mut String,
        indent: &str,
        pc: u32,
        next_pc: u32,
        op: DecodedOp,
        opts: &AotOptions,
        in_state_machine: bool,
    ) -> Result<(), String> {
        match op {
            DecodedOp::Nop => {
                let _ = writeln!(out, "{indent}// 0x{pc:x}: nop");
            }
            DecodedOp::ComputeCond { cond: 31, op } | DecodedOp::ComputeBoth { op } => {
                emit_compute_rust(out, indent, op, opts.fast_float)?;
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
                        let _ = writeln!(out, "{indent}core.write_ureg({ureg}, 0x{value:08x});");
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
                    let _ = writeln!(out, "{indent}_sram[addr] = core.r[{ureg}];");
                } else {
                    let _ = writeln!(out, "{indent}core.r[{ureg}] = _sram[addr];");
                }
                if post_modify {
                    let _ = writeln!(
                        out,
                        "{indent}core.i[{ii}] = core.i[{ii}].wrapping_add(core.m[{mi}]);"
                    );
                }
            }
            DecodedOp::DualMove(dm) if dm.dm_reg < 16 && dm.pm_reg < 16 => {
                let _ = writeln!(
                    out,
                    "{indent}let (dm_a, pm_a) = (core.i[{}] as usize, core.i[{}] as usize);",
                    dm.dmi, dm.pmi
                );
                if dm.dm_write {
                    let _ = writeln!(out, "{indent}let dm_cap = core.r[{}];", dm.dm_reg);
                } else {
                    let _ = writeln!(out, "{indent}let dm_val = _sram[dm_a];");
                }
                if dm.pm_write {
                    let _ = writeln!(out, "{indent}let pm_cap = core.r[{}];", dm.pm_reg);
                } else {
                    let _ = writeln!(out, "{indent}let pm_val = _sram[pm_a];");
                }
                if let Some(cop) = dm.compute {
                    emit_compute_rust(out, indent, cop, opts.fast_float)?;
                }
                if dm.dm_write {
                    let _ = writeln!(out, "{indent}_sram[dm_a] = dm_cap;");
                } else {
                    let _ = writeln!(out, "{indent}core.r[{}] = dm_val;", dm.dm_reg);
                }
                if dm.pm_write {
                    let _ = writeln!(out, "{indent}_sram[pm_a] = pm_cap;");
                } else {
                    let _ = writeln!(out, "{indent}core.r[{}] = pm_val;", dm.pm_reg);
                }
                let _ = writeln!(
                    out,
                    "{indent}core.i[{0}] = core.i[{0}].wrapping_add(core.m[{1}]);",
                    dm.dmi, dm.dmm
                );
                let _ = writeln!(
                    out,
                    "{indent}core.i[{0}] = core.i[{0}].wrapping_add(core.m[{1}]);",
                    dm.pmi, dm.pmm
                );
            }
            DecodedOp::Flow(FlowOp::JumpDirect {
                cond,
                target,
                call: false,
                delayed: false,
            }) if in_state_machine => {
                let c_expr = cond_expr_rust(cond)?;
                if cond == 31 {
                    let _ = writeln!(out, "{indent}pc = 0x{target:x};");
                } else {
                    let _ = writeln!(
                        out,
                        "{indent}if {c_expr} {{ pc = 0x{target:x}; continue; }} else {{ pc = 0x{next_pc:x}; continue; }}"
                    );
                }
            }
            DecodedOp::Flow(FlowOp::Return {
                cond: 31,
                rti: false,
                delayed: false,
                compute: None,
                ..
            }) => {
                let _ = writeln!(out, "{indent}return;");
            }
            other => return Err(format!("unsupported AOT op at 0x{pc:x}: {other:?}")),
        }
        Ok(())
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
        assert!(src.contains("core.r[0] = _sram[addr];"), "{src}");
        assert!(src.contains("core.r[1] = res;"), "{src}");
        assert!(
            src.contains("f32::from_bits(core.r[1]) * f32::from_bits(core.r[2])"),
            "{src}"
        );
        assert!(src.contains("return;"), "{src}");
    }

    #[test]
    fn aot_emits_dual_move_and_jump_direct_state_machine() {
        // 0..3: R1 = R1 + R2 (2a)
        // 3..6: DualMove F2 = F0 + F4, r0=dm(i6,m3), r4=pm(i10,m10)
        // 6..9: JUMP (PC, -6) -> 0x120000 (T8a: 0x073e, 0xff, 0xfffa)
        let program = [
            0x013eu16, 0x0000, 0x1112, // R1 = R1 + R2
            0x2cc0, 0x9208, 0x1204, // DualMove
            0x073e, 0x00ff, 0xfffa, // JUMP (PC, -6)
        ];
        let c = core_with(&program);
        let src = c
            .emit_aot_rust(
                c.pc,
                3,
                &AotOptions {
                    fast_float: true,
                    fn_name: "jump_loop".to_string(),
                },
            )
            .expect("emit JumpDirect + DualMove");
        assert!(src.contains("match pc {"), "{src}");
        assert!(
            src.contains("let (dm_a, pm_a) = (core.i[6] as usize, core.i[10] as usize);"),
            "{src}"
        );
        assert!(src.contains("pc = 0x120000;"), "{src}");
    }

    #[test]
    fn aot_compiled_rustc_matches_core_run_oracle() {
        // Transpile a real DO loop + SRAM load + fixed ADD + float MUL + RTS,
        // compile it with `rustc -O`, execute it, and assert bit-exact equality with Core::run.
        let program = [
            0x0d05u16, 0x0000, 0x0007, // DO (+7) UNTIL LCE, LCNTR=R5
            0x9410, // R0 = DM(I2, M0)
            0x013e, 0x0000, 0x1110, // R1 = R1 + R0
            0x013e, 0x0013, 0x0312, // F3 = F1 * F2
            0x0abe, // RTS
        ];
        let mut oracle = core_with(&program);
        oracle.r[5] = 4;
        oracle.r[1] = 1.5f32.to_bits();
        oracle.r[2] = 2.0f32.to_bits();
        (oracle.i[2], oracle.m[0]) = (0x9_0100, 1);
        for k in 0..4u32 {
            oracle.dm_write(0x9_0100 + k, (k + 1) * 0x10_0000).unwrap();
        }

        let body = oracle
            .emit_aot_rust(
                oracle.pc,
                5,
                &AotOptions {
                    fast_float: true,
                    fn_name: "dsp_kernel".to_string(),
                },
            )
            .expect("emit AOT Rust");

        // Run oracle: 1 DO + 4 * 3 body insns = 13 steps (before RTS).
        assert_eq!(oracle.run(13), None);

        let dir = std::env::temp_dir().join(format!("sharc_aot_oracle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rs_path = dir.join("oracle_check.rs");
        let exe_path = dir.join(if cfg!(windows) {
            "oracle_check.exe"
        } else {
            "oracle_check"
        });
        let driver = format!(
            r#"mod nord_sharc {{
    pub mod core {{
        #[derive(Default)]
        pub struct Flags {{
            pub az: bool,
            pub an: bool,
            pub av: bool,
            pub ac: bool,
        }}
        #[derive(Default)]
        pub struct Core {{
            pub r: [u32; 16],
            pub i: [u32; 16],
            pub m: [u32; 16],
            pub mr: [i128; 2],
            pub flags: Flags,
        }}
    }}
}}
{body}
fn main() {{
    let mut core = nord_sharc::core::Core::default();
    core.r[5] = 4;
    core.r[1] = 1.5f32.to_bits();
    core.r[2] = 2.0f32.to_bits();
    core.i[2] = 0x9_0100;
    core.m[0] = 1;
    let mut sram = vec![0u32; 0x10_0000];
    for k in 0..4u32 {{
        sram[0x9_0100 + k as usize] = (k + 1) * 0x10_0000;
    }}
    dsp_kernel(&mut core, &mut sram);
    println!("{{}} {{}} {{}} {{}} {{}} {{}}", core.r[0], core.r[1], core.r[3], core.i[2], core.flags.az, core.flags.an);
}}"#
        );
        std::fs::write(&rs_path, driver).unwrap();
        let status = std::process::Command::new("rustc")
            .args(["-O", "-o"])
            .arg(&exe_path)
            .arg(&rs_path)
            .status()
            .expect("invoke rustc -O");
        assert!(status.success(), "rustc -O failed");

        let out = std::process::Command::new(&exe_path)
            .output()
            .expect("run compiled AOT binary");
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = stdout.split_whitespace().collect();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(parts.len(), 6, "unexpected output: {stdout}");
        assert_eq!(parts[0].parse::<u32>().unwrap(), oracle.r[0]);
        assert_eq!(parts[1].parse::<u32>().unwrap(), oracle.r[1]);
        assert_eq!(parts[2].parse::<u32>().unwrap(), oracle.r[3]);
        assert_eq!(parts[3].parse::<u32>().unwrap(), oracle.i[2]);
        assert_eq!(parts[4].parse::<bool>().unwrap(), oracle.flags.az);
        assert_eq!(parts[5].parse::<bool>().unwrap(), oracle.flags.an);
    }

    #[test]
    #[ignore = "AOT rustc -O benchmark: run with cargo test --release -p nord-sharc --lib bench_aot_rustc_o3_throughput -- --ignored --nocapture"]
    fn bench_aot_rustc_o3_throughput() {
        let mut program = vec![0x0d00u16, 0x0000, 3 + 3 + 3];
        program.extend_from_slice(&[0x013e, 0x0000, 0x1331]); // R3 = R3 + R1 (loop-carried!)
        program.extend_from_slice(&[0x013e, 0x0013, 0x0432]); // F4 = F3 * F2
        program.extend_from_slice(&[0x0001]); // NOP

        let c = core_with(&program);
        let body = c
            .emit_aot_rust(
                c.pc,
                4,
                &AotOptions {
                    fast_float: true,
                    fn_name: "aot_bench_kernel".to_string(),
                },
            )
            .expect("emit AOT kernel");

        // Adapt signature to a standalone zero-dependency Rust binary for rustc -O.
        let body_standalone = body.replace("nord_sharc::core::Core", "Core");
        let harness = format!(
            r#"
#[derive(Default)]
pub struct Flags {{ pub az: bool, pub an: bool }}
#[derive(Default)]
pub struct Core {{ pub r: [u32; 16], pub i: [u32; 16], pub m: [u32; 16], pub l: [u32; 16], pub flags: Flags }}
{body_standalone}
fn main() {{
    let mut core = Core::default();
    let mut sram = [0u32; 16];
    core.r[0] = 200_000_000;
    core.r[1] = 1.5f32.to_bits();
    core.r[2] = 2.0f32.to_bits();
    let t0 = std::time::Instant::now();
    aot_bench_kernel(&mut core, &mut sram);
    let dt = t0.elapsed();
    let steps = 200_000_000u64 * 3;
    let msteps = steps as f64 / 1e6 / dt.as_secs_f64();
    println!("bench_aot_rustc_o3_throughput: {{steps}} steps in {{dt:?}} ({{msteps:.2}} Msteps/s) r3={{:#x}}", core.r[3]);
}}
"#
        );
        let dir = std::env::temp_dir();
        let rs_path = dir.join("sharc_aot_bench.rs");
        let exe_path = dir.join("sharc_aot_bench.exe");
        std::fs::write(&rs_path, harness).unwrap();
        let status = std::process::Command::new("rustc")
            .args(["--edition", "2021", "-C", "opt-level=3", "-o"])
            .arg(&exe_path)
            .arg(&rs_path)
            .status()
            .expect("invoke rustc");
        assert!(status.success(), "rustc failed");
        let out = std::process::Command::new(&exe_path)
            .output()
            .expect("run compiled AOT binary");
        assert!(out.status.success());
        eprint!("{}", String::from_utf8_lossy(&out.stdout));
    }
}
