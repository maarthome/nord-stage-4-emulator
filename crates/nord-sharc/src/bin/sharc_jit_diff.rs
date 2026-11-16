//! `sharc-jit-diff IMAGE [options]` runs a real SHARC firmware image in both [`Core::run`]
//! (interpreter oracle) and [`Core::run_jit`], comparing full architectural state and SRAM every
//! `--chunk N` steps and stopping at the first divergence.

use nord_sharc::core::Core;
use nord_sharc::runner::parse_number;
use std::process::ExitCode;

struct Opts {
    image: String,
    pc: Option<u32>,
    steps: u64,
    chunk: u64,
    lenient: bool,
    fast_float: bool,
}

fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        image: String::new(),
        pc: None,
        steps: 200_000,
        chunk: 1_000,
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
            "--lenient" => o.lenient = true,
            "--fast-float" => o.fast_float = true,
            _ if a.starts_with('-') => return Err(format!("unknown option {a}")),
            _ => o.image = a.clone(),
        }
    }
    if o.image.is_empty() {
        return Err("usage: sharc-jit-diff IMAGE [--pc HEX] [--steps N] [--chunk K] [--lenient] [--fast-float]".into());
    }
    Ok(o)
}

#[cfg(feature = "jit")]
fn run_diff(o: &Opts, image: &[u8]) -> ExitCode {
    let mut interp = Core::new(image);
    let mut jit = Core::new(image);
    for c in [&mut interp, &mut jit] {
        if let Some(pc) = o.pc {
            c.pc = pc;
        }
        c.lenient = o.lenient;
        c.fast_float = o.fast_float;
    }
    match nord_sharc::runner::jit_diff::diff_run(&mut interp, &mut jit, o.steps, o.chunk) {
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
    }
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
            "0x120000",
            "--steps",
            "5000",
            "--chunk",
            "250",
            "--lenient",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let o = parse_args(&args).unwrap();
        assert_eq!(o.image, "org.bin");
        assert_eq!(o.pc, Some(0x12_0000));
        assert_eq!(o.steps, 5000);
        assert_eq!(o.chunk, 250);
        assert!(o.lenient);
        assert!(parse_args(&[]).is_err());
    }
}
