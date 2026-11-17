//! `sharc-live IMAGE [--pno] [--epdma] [--ring-dma] [--probe PC]... [--lane SET:PAIR]`: one DSP
//! chip as a line-protocol server (D138). The ARM harness (`tools/arm_dsp_link.py`) writes the
//! messages the firmware sends (`M W1 W2`), asks for passes (`P N`) and reads the summary lines;
//! the protocol is in `nord_sharc::runner::live`. Prints `READY` once the chip has booted.
//! Evidence: `docs/research/arm-dsp-live-link.md`.
use nord_sharc::runner::live::{boot_core, pno_preset, Live};
use nord_sharc::runner::parse_number;
use nord_sharc::runner::pass::PassConfig;
use nord_sharc::runner::periph_run::PeriphOpts;
use std::io::{BufRead, Write};
use std::process::ExitCode;

/// Parsed command line: image path, peripheral models, probe pcs, lane `(set, pair)`.
type Parsed = (String, PeriphOpts, Vec<u32>, (u32, u32));

fn parse(args: &[String]) -> Result<Parsed, String> {
    let (mut image, mut periph, mut probes, mut lane) =
        (String::new(), PeriphOpts::default(), vec![], (2, 0));
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--pno" => {} // the only preset; the flag keeps the command lines of sharc-sys
            "--probe" => probes.push(
                it.next()
                    .and_then(|s| parse_number(s))
                    .ok_or("bad --probe")?,
            ),
            "--lane" => {
                let s = it.next().ok_or("--lane needs SET:PAIR")?;
                let bad = || format!("bad --lane {s:?} (want SET:PAIR)");
                let (a, b) = s.split_once(':').ok_or_else(bad)?;
                lane = (
                    parse_number(a).ok_or_else(bad)?,
                    parse_number(b).ok_or_else(bad)?,
                );
                if lane.0 > 5 || lane.1 > 2 {
                    return Err(bad());
                }
            }
            _ if periph.take_flag(a) => {}
            _ if image.is_empty() && !a.starts_with("--") => image = a.clone(),
            _ => return Err(format!("unknown argument {a:?}")),
        }
    }
    if image.is_empty() {
        return Err("usage: sharc-live IMAGE [--pno] [--epdma] [--ring-dma] [--probe PC]... [--lane SET:PAIR]".into());
    }
    Ok((image, periph, probes, lane))
}

fn serve(args: &[String]) -> Result<(), String> {
    let (image, periph, probes, lane) = parse(args)?;
    let data = std::fs::read(&image).map_err(|e| format!("{image}: {e}"))?;
    let p = pno_preset();
    let core = boot_core(&data, &p, &periph)?;
    let cfg = PassConfig {
        trigger: p.trigger,
        idle_pc: p.idle_pc,
        max_steps: 200_000,
        irq: periph.start_status(p.irq),
    };
    let mut live = Live::new(core, cfg, p.ping_pong, lane, probes);
    let (stdin, stdout) = (std::io::stdin(), std::io::stdout());
    let mut out = stdout.lock();
    writeln!(out, "READY").map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim() == "Q" {
            break;
        }
        if let Some(reply) = live.handle(&line) {
            writeln!(out, "{reply}").map_err(|e| e.to_string())?;
        }
        if line.trim_start().starts_with(['P', 'D']) {
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match serve(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sharc-live: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn arguments_select_image_probes_lane_and_models() {
        let (img, p, probes, lane) = parse(&args(&[
            "core1.bin",
            "--pno",
            "--epdma",
            "--probe",
            "0x1256AD",
            "--probe",
            "7",
            "--lane",
            "1:2",
        ]))
        .unwrap();
        assert_eq!(
            (img.as_str(), probes, lane),
            ("core1.bin", vec![0x1256AD, 7], (1, 2))
        );
        assert!(p.epdma && !p.ring_dma);
        assert_eq!(
            parse(&args(&["a.bin"])).unwrap().3,
            (2, 0),
            "default lane 2:0"
        );
    }

    #[test]
    fn bad_arguments_are_refused() {
        for bad in [
            &["a.bin", "--lane", "9:0"][..],
            &["a.bin", "--bogus"],
            &[],
            &["a", "--probe", "zz"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
