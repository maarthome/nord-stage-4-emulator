//! `sharc-aot IMAGE ENTRY MAX_INSNS [--fast-float] [--fn NAME] [--out FILE]`
//! transpiles a SHARC VISA routine from a flat SRAM image into standalone Rust source (Level-3 AOT).
use nord_sharc::core::{AotOptions, Core};
use std::process::ExitCode;

fn parse_number(s: &str) -> Option<u64> {
    match s.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

fn run_cli(args: &[String]) -> Result<Option<String>, String> {
    let mut positional: Vec<String> = Vec::new();
    let mut opts = AotOptions::default();
    let mut out_path: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--fast-float" => opts.fast_float = true,
            "--fn" => {
                opts.fn_name = it.next().ok_or("--fn needs NAME")?.clone();
            }
            "--out" => {
                out_path = Some(it.next().ok_or("--out needs FILE")?.clone());
            }
            _ => positional.push(a.clone()),
        }
    }
    let [image, entry, max_insns] = positional.as_slice() else {
        return Err(
            "usage: sharc-aot IMAGE ENTRY MAX_INSNS [--fast-float] [--fn NAME] [--out FILE]"
                .to_string(),
        );
    };
    let data = std::fs::read(image).map_err(|e| format!("{image}: {e}"))?;
    let entry = parse_number(entry).ok_or("bad ENTRY")? as u32;
    let max_insns = parse_number(max_insns).ok_or("bad MAX_INSNS")? as usize;
    let core = Core::new(&data);
    let src = core.emit_aot_rust(entry, max_insns, &opts)?;
    if let Some(path) = out_path {
        std::fs::write(&path, &src).map_err(|e| format!("{path}: {e}"))?;
        Ok(None)
    } else {
        Ok(Some(src))
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run_cli(&args) {
        Ok(Some(src)) => {
            print!("{src}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_rejects_bad_arguments_and_missing_image() {
        assert!(run_cli(&[]).is_err());
        assert!(run_cli(&["missing.bin".into(), "0x120000".into(), "4".into()]).is_err());
    }

    #[test]
    fn cli_transpiles_synthetic_image_to_rust() {
        let dir = std::env::temp_dir();
        let img = dir.join("sharc_aot_test.bin");
        let mut sram = vec![0u8; 0x25_0000];
        // Place R1 = 7 (0x0f81, 0x0007) + RTS (0x0abe) at short-word 0x12_0000.
        let parcels = [0x0f81u16, 0x0007, 0x0abe];
        for (k, p) in parcels.iter().enumerate() {
            let o = (0x12_0000 + k) * 2;
            sram[o..o + 2].copy_from_slice(&p.to_le_bytes());
        }
        std::fs::write(&img, &sram).unwrap();
        let out = run_cli(&[
            img.to_string_lossy().into(),
            "0x120000".into(),
            "4".into(),
            "--fast-float".into(),
            "--fn".into(),
            "test_fn".into(),
        ])
        .unwrap()
        .unwrap();
        let _ = std::fs::remove_file(&img);
        assert!(out.contains("pub fn test_fn("));
        assert!(out.contains("core.r[1] = 0x00000007;"));
        assert!(out.contains("return;"));
    }
}
