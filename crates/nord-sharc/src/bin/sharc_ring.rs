//! `sharc-ring IMAGE --trace LO:LEN [--from P --to Q] [--script-flat F]... [--script-at OFF:F]...`:
//! the PNO core 1 live lane (same preset as `sharc-sys --pno --epdma`) with a load and store trace of
//! the window `LO..LO + LEN` for passes `P..Q` (D126). Prints pc, count and address runs only.
use nord_sharc::core::Core;
use nord_sharc::runner::msg::{deliver, parse_script, Msg};
use nord_sharc::runner::pass::{fault_text, run_steps, PassConfig};
use nord_sharc::runner::ring_trace::trace_pass;
use nord_sharc::runner::system::{lean_pass, set_ping_pong, PNO_PING_PONG};
use nord_sharc::runner::{parse_number, parse_value};
use std::process::ExitCode;

fn read_script(file: &str, offset: u32, flat: bool) -> Result<Vec<Msg>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut v = parse_script(&text)?;
    for m in &mut v {
        m.pass = if flat { 0 } else { m.pass + offset };
    }
    Ok(v)
}

fn run(args: &[String]) -> Result<String, String> {
    let (mut image, mut msgs, mut win) = (String::new(), Vec::new(), (0xB_0000, 0x100));
    let (mut from, mut to, mut pokes) = (20, 24, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--trace" => {
                let s = val()?;
                let (x, n) = s.split_once(':').ok_or("--trace needs LO:LEN")?;
                win = (
                    parse_number(x).ok_or("bad LO")?,
                    parse_number(n).ok_or("bad LEN")?,
                );
            }
            "--from" => from = parse_number(&val()?).ok_or("bad --from")?,
            "--to" => to = parse_number(&val()?).ok_or("bad --to")?,
            "--script-flat" => msgs.extend(read_script(&val()?, 0, true)?),
            "--poke" => {
                let s = val()?;
                let (x, v) = s.split_once('=').ok_or("--poke needs ADDR=V")?;
                pokes.push((
                    parse_number(x).ok_or("bad address")?,
                    parse_value(v).ok_or("bad value")?,
                ));
            }
            "--script-at" => {
                let s = val()?;
                let (off, f) = s.split_once(':').ok_or("--script-at needs OFFSET:FILE")?;
                msgs.extend(read_script(
                    f,
                    parse_number(off).ok_or("bad offset")?,
                    false,
                )?);
            }
            _ if image.is_empty() && !a.starts_with("--") => image = a.clone(),
            _ => return Err(format!("unknown argument {a:?}")),
        }
    }
    let data = std::fs::read(&image).map_err(|e| format!("{image}: {e}"))?;
    let mut core = Core::new(&data);
    core.enable_epdma();
    core.pc = 0x12_1007;
    core.set_flag_input(1, true);
    for (a, v) in [(0x28ac_0380, 1), (0x3108_d008, 5), (0x3108_c06c, 0x400)] {
        core.preset_device(a, v);
    }
    core.preset_device(0x3108_c00c, 4);
    if let Some(f) = run_steps(&mut core, 3_000_000) {
        return Err(format!("boot faulted: {}", fault_text(&f)));
    }
    let cfg = PassConfig {
        trigger: 0xB_00E1,
        idle_pc: 0x12_1091,
        max_steps: 200_000,
        irq: Some((15, 1)),
    };
    msgs.sort_by_key(|m| m.pass);
    let (mut next, mut out) = (0, String::new());
    for p in 0..to {
        set_ping_pong(&mut core, PNO_PING_PONG, p);
        while next < msgs.len() && msgs[next].pass <= p {
            deliver(&mut core, 0, &msgs[next])?;
            next += 1;
        }
        if p == from {
            for &(a, v) in &pokes {
                core.dm_write(a, v)
                    .map_err(|a| format!("bad poke 0x{a:x}"))?;
            }
        }
        if p >= from {
            out += &format!(
                "pass {p}: {}",
                trace_pass(&mut core, &cfg, win.0, win.1).text()
            );
        } else if !lean_pass(&mut core, &cfg).1 {
            return Err(format!("pass {p} did not return to idle"));
        }
    }
    Ok(out)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(t) => {
            print!("{t}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sharc-ring: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_arguments_are_errors() {
        assert!(run(&["--bogus".into()]).is_err());
        assert!(run(&["--trace".into(), "zz".into()]).is_err());
        assert!(run(&["/no/such/image.bin".into()]).is_err());
    }

    #[test]
    fn script_offset_and_flat_mode() {
        assert!(read_script("/no/such/file", 0, true).is_err());
    }
}
