//! `sharc-jit-diff IMAGE [options]` runs a real SHARC firmware image in both [`Core::run`]
//! (interpreter oracle) and [`Core::run_jit`], comparing full architectural state and SRAM every
//! `--chunk N` steps and stopping at the first divergence. With `--passes N`, it boots the
//! image, delivers ARM messages (`--msg` / `--msg-script`, base `0xE0000`), runs `N` audio passes,
//! optionally reports `--coverage` PC ranges, and `--bench` measures mean/worst pass latency and
//! Msteps/s for both interpreter and JIT.

use nord_sharc::core::Core;
use nord_sharc::runner::msg::{self, Msg, STRIP_BASE};
#[cfg(feature = "jit")]
use nord_sharc::runner::pass::PassConfig;
use nord_sharc::runner::{parse_number, parse_value};
use std::process::ExitCode;
#[cfg(feature = "jit")]
use std::time::Instant;

struct Opts {
    image: String,
    pc: Option<u32>,
    steps: u64,
    chunk: u64,
    boot: u64,
    passes: u32,
    max_pass_steps: u64,
    trigger: u32,
    idle_pc: u32,
    devs: Vec<(u32, u32)>,
    msgs: Vec<Msg>,
    msg_base: u32,
    coverage: bool,
    bench: bool,
    lenient: bool,
    fast_float: bool,
}

fn parse_kv(s: &str) -> Result<(u32, u32), String> {
    let (a, v) = s.split_once('=').ok_or_else(|| format!("bad pair {s:?}"))?;
    Ok((
        parse_number(a.trim()).ok_or("bad address")?,
        parse_value(v.trim()).ok_or("bad value")?,
    ))
}

fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        image: String::new(),
        pc: None,
        steps: 200_000,
        chunk: 1_000,
        boot: 0,
        passes: 0,
        max_pass_steps: 200_000,
        trigger: 0xB_00E2,
        idle_pc: 0x12_5DDF,
        devs: Vec::new(),
        msgs: Vec::new(),
        msg_base: STRIP_BASE,
        coverage: false,
        bench: false,
        lenient: false,
        fast_float: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |what: &str| it.next().cloned().ok_or(format!("{a} needs {what}"));
        match a.as_str() {
            "--pc" => o.pc = Some(parse_number(&val("HEX")?).ok_or("bad --pc")?),
            "--steps" => o.steps = u64::from(parse_number(&val("N")?).ok_or("bad --steps")?),
            "--chunk" => o.chunk = u64::from(parse_number(&val("N")?).ok_or("bad --chunk")?),
            "--boot" => o.boot = u64::from(parse_number(&val("N")?).ok_or("bad --boot")?),
            "--passes" => o.passes = parse_number(&val("N")?).ok_or("bad --passes")?,
            "--max-pass-steps" => {
                o.max_pass_steps =
                    u64::from(parse_number(&val("N")?).ok_or("bad --max-pass-steps")?);
            }
            "--trigger" => o.trigger = parse_number(&val("ADDR")?).ok_or("bad --trigger")?,
            "--idle-pc" => o.idle_pc = parse_number(&val("PC")?).ok_or("bad --idle-pc")?,
            "--dev" => o.devs.push(parse_kv(&val("ADDR=V")?)?),
            "--msg" => o.msgs.push(msg::parse_msg(&val("[PASS:]W1=W2")?)?),
            "--msg-script" => {
                let path = val("FILE")?;
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                o.msgs.extend(msg::parse_script(&text)?);
            }
            "--msg-base" => o.msg_base = parse_number(&val("ADDR")?).ok_or("bad --msg-base")?,
            "--coverage" => o.coverage = true,
            "--bench" => o.bench = true,
            "--lenient" => o.lenient = true,
            "--fast-float" => o.fast_float = true,
            _ if a.starts_with('-') => return Err(format!("unknown option {a}")),
            _ => o.image = a.clone(),
        }
    }
    if o.image.is_empty() {
        return Err("usage: sharc-jit-diff IMAGE [--pc HEX] [--steps N] [--chunk K] [--boot N] [--passes N] [--dev ADDR=V] [--msg [PASS:]W1=W2] [--coverage] [--bench] [--lenient]".into());
    }
    Ok(o)
}

#[cfg(feature = "jit")]
fn init_core(o: &Opts, image: &[u8]) -> Core {
    let mut c = Core::new(image);
    if let Some(pc) = o.pc {
        c.pc = pc;
    }
    c.lenient = o.lenient;
    c.fast_float = o.fast_float;
    for &(a, v) in &o.devs {
        c.preset_device(a, v);
    }
    c
}

#[cfg(feature = "jit")]
fn run_single_pass_timed(
    c: &mut Core,
    cfg: &PassConfig,
    use_jit: bool,
) -> Result<(u64, f64), String> {
    c.dm_write(cfg.trigger, 1)
        .map_err(|a| format!("bad trigger 0x{a:x}"))?;
    let start = c.steps;
    let mut left_idle = false;
    let t0 = Instant::now();
    while c.steps - start < cfg.max_steps {
        let f = if use_jit { c.run_jit(1) } else { c.run(1) };
        if let Some(fault) = f {
            return Err(format!("fault: {fault:?}"));
        }
        if c.pc != cfg.idle_pc {
            left_idle = true;
        }
        if left_idle && c.pc == cfg.idle_pc && c.dm_read(cfg.trigger).unwrap_or(1) == 0 {
            break;
        }
    }
    Ok((c.steps - start, t0.elapsed().as_secs_f64()))
}

#[cfg(feature = "jit")]
fn run_pass_burst_timed(
    c: &mut Core,
    cfg: &PassConfig,
    use_jit: bool,
) -> Result<(u64, f64), String> {
    if !use_jit {
        return run_single_pass_timed(c, cfg, false);
    }
    c.dm_write(cfg.trigger, 1)
        .map_err(|a| format!("bad trigger 0x{a:x}"))?;
    let start = c.steps;
    let t0 = Instant::now();
    while c.steps - start < cfg.max_steps {
        let chunk = (cfg.max_steps - (c.steps - start)).min(64);
        if let Some(fault) = c.run_jit(chunk) {
            return Err(format!("jit fault: {fault:?}"));
        }
        if c.dm_read(cfg.trigger).unwrap_or(1) == 0
            && (cfg.idle_pc.saturating_sub(4)..=cfg.idle_pc.saturating_add(4)).contains(&c.pc)
        {
            break;
        }
    }
    Ok((c.steps - start, t0.elapsed().as_secs_f64()))
}

#[cfg(feature = "jit")]
fn run_diff(o: &Opts, image: &[u8]) -> ExitCode {
    use nord_sharc::runner::jit_diff::{diff_pass, diff_run};

    let mut interp = init_core(o, image);
    let mut jit = init_core(o, image);

    if o.passes == 0 {
        return match diff_run(&mut interp, &mut jit, o.steps, o.chunk) {
            Ok(()) => {
                println!(
                    "OK: {} steps matched 1:1 (chunk={}, final pc=0x{:x})",
                    interp.steps, o.chunk, interp.pc
                );
                ExitCode::SUCCESS
            }
            Err(m) => {
                eprintln!("MISMATCH: {m}");
                ExitCode::FAILURE
            }
        };
    }

    if o.boot > 0 {
        if let Err(m) = diff_run(&mut interp, &mut jit, o.boot, o.chunk) {
            eprintln!("BOOT MISMATCH: {m}");
            return ExitCode::FAILURE;
        }
        println!("booted {} steps 1:1 (pc=0x{:x})", interp.steps, interp.pc);
    }

    let cfg = PassConfig {
        trigger: o.trigger,
        idle_pc: o.idle_pc,
        max_steps: o.max_pass_steps,
        irq: None,
    };

    for p in 0..o.passes {
        for m in o.msgs.iter().filter(|m| m.pass == p) {
            for c in [&mut interp, &mut jit] {
                if let Err(e) = msg::deliver(c, o.msg_base, m) {
                    eprintln!("msg error pass {p}: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        match diff_pass(&mut interp, &mut jit, &cfg, o.chunk, o.coverage) {
            Ok(rep) => {
                print!(
                    "pass {p}: OK ({} steps, final pc=0x{:x})",
                    rep.steps, rep.final_pc
                );
                if o.coverage {
                    let ranges: Vec<String> = rep
                        .coverage
                        .iter()
                        .map(|(lo, hi)| format!("0x{lo:x}..0x{hi:x}"))
                        .collect();
                    print!(" coverage=[{}]", ranges.join(", "));
                }
                println!();
            }
            Err(m) => {
                eprintln!("pass {p} MISMATCH: {m}");
                return ExitCode::FAILURE;
            }
        }
    }

    if o.bench {
        let mut b_interp = init_core(o, image);
        let mut b_jit = init_core(o, image);
        if o.boot > 0 {
            let _ = b_interp.run(o.boot);
            let _ = b_jit.run_jit(o.boot);
            if b_jit.pc != b_interp.pc || b_jit.steps != b_interp.steps {
                eprintln!(
                    "WARN bench boot mismatch: interp(pc=0x{:x}, steps={}) vs jit(pc=0x{:x}, steps={})",
                    b_interp.pc, b_interp.steps, b_jit.pc, b_jit.steps
                );
            }
        }
        let (mut i_steps, mut i_total_s, mut i_worst_s) = (0u64, 0.0f64, 0.0f64);
        let (mut j_steps, mut j_total_s, mut j_worst_s) = (0u64, 0.0f64, 0.0f64);
        for p in 0..o.passes {
            for m in o.msgs.iter().filter(|m| m.pass == p) {
                let _ = msg::deliver(&mut b_interp, o.msg_base, m);
                let _ = msg::deliver(&mut b_jit, o.msg_base, m);
            }
            match run_pass_burst_timed(&mut b_interp, &cfg, false) {
                Ok((s, dt)) => {
                    i_steps += s;
                    i_total_s += dt;
                    i_worst_s = i_worst_s.max(dt);
                }
                Err(e) => {
                    eprintln!("bench interp pass {p} {e}");
                    return ExitCode::FAILURE;
                }
            }
            match run_pass_burst_timed(&mut b_jit, &cfg, true) {
                Ok((s, dt)) => {
                    j_steps += s;
                    j_total_s += dt;
                    j_worst_s = j_worst_s.max(dt);
                }
                Err(e) => {
                    eprintln!("bench jit pass {p} {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        let n = f64::from(o.passes);
        println!(
            "bench ({} passes): interp {} steps mean={:.3} ms worst={:.3} ms ({:.2} Msteps/s) | jit {} steps mean={:.3} ms worst={:.3} ms ({:.2} Msteps/s)",
            o.passes,
            i_steps,
            (i_total_s / n) * 1e3,
            i_worst_s * 1e3,
            i_steps as f64 / 1e6 / i_total_s,
            j_steps,
            (j_total_s / n) * 1e3,
            j_worst_s * 1e3,
            j_steps as f64 / 1e6 / j_total_s,
        );
    }

    ExitCode::SUCCESS
}

#[cfg(not(feature = "jit"))]
fn run_diff(_o: &Opts, _image: &[u8]) -> ExitCode {
    let _ = Core::new(&[]);
    eprintln!("sharc-jit-diff requires `--features jit`");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let image = match std::fs::read(&opts.image) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: {e}", opts.image);
            return ExitCode::FAILURE;
        }
    };
    run_diff(&opts, &image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_parse_and_missing_image_errors() {
        let args: Vec<String> = [
            "org.bin",
            "--pc",
            "0x125c88",
            "--boot",
            "100000",
            "--passes",
            "4",
            "--chunk",
            "250",
            "--dev",
            "0x282c0380=1",
            "--msg",
            "0:0=0x8012082f",
            "--coverage",
            "--bench",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let o = parse_args(&args).unwrap();
        assert_eq!(o.image, "org.bin");
        assert_eq!(o.pc, Some(0x12_5C88));
        assert_eq!(o.boot, 100_000);
        assert_eq!(o.passes, 4);
        assert_eq!(o.chunk, 250);
        assert_eq!(o.devs, vec![(0x282c_0380, 1)]);
        assert_eq!(o.msgs.len(), 1);
        assert!(o.coverage);
        assert!(o.bench);
        assert!(parse_args(&[]).is_err());
    }
}
