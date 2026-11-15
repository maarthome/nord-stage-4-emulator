//! Writes (or checks) the per-program synth velocity census CSV of a Nord Stage 4 backup.
//!
//! Usage: `cargo run --release -p nord-tools --bin synth_velocity_census -- <backup.ns4b> --out <csv>`
//! or `... <backup.ns4b> --check <csv>` (exit 1 when the committed CSV differs). Derived numbers only.

use nord_core::cbin::{CbinFile, Ns4BackupArchive};
use nord_tools::velocity_census::{row, LayerFacts, HEADER};
use std::fs::File;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
}

fn census(path: &str) -> Result<String, String> {
    let archive = Ns4BackupArchive::open(path).map_err(|e| e.to_string())?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut out = format!("{HEADER}\n");
    let mut names: Vec<String> = archive
        .entries_with_ext(".ns4p")
        .iter()
        .map(|e| e.name.clone())
        .collect();
    names.sort();
    for name in names {
        let bytes = archive
            .read_entry(&mut file, &name)
            .map_err(|e| e.to_string())?;
        let prog = CbinFile::parse(&bytes)
            .and_then(|c| c.decode_program())
            .map_err(|e| format!("{name}: {e}"))?;
        let refs = archive.resolve_program_sounds(&prog);
        let mask = prog.global.scene_1_mask | prog.global.scene_2_mask;
        for (i, (letter, layer, sound)) in [
            ('A', &prog.synth.layer_a, &refs.synth_a),
            ('B', &prog.synth.layer_b, &refs.synth_b),
            ('C', &prog.synth.layer_c, &refs.synth_c),
        ]
        .into_iter()
        .enumerate()
        {
            out.push_str(&row(&LayerFacts {
                entry: &name,
                program: &prog.name,
                layer: letter,
                enabled: layer.enabled,
                in_scene: mask & (1 << (4 + i)) != 0,
                sound: sound.is_some(),
                amp_setting: layer.amp_velocity,
                filter_velocity: layer.filter_velocity,
                filter_on: layer.filter_on,
                osc_env_velocity: layer.osc_env_velocity,
            }));
            out.push('\n');
        }
    }
    Ok(out)
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .ok_or("usage: <backup.ns4b> --out|--check <csv>")?;
    let text = census(path)?;
    if let Some(out) = flag(&args, "--out") {
        std::fs::write(out, &text).map_err(|e| e.to_string())?;
        println!("wrote {} rows to {out}", text.lines().count() - 1);
        return Ok(0);
    }
    if let Some(check) = flag(&args, "--check") {
        let have = std::fs::read_to_string(check).map_err(|e| e.to_string())?;
        let same = have.replace("\r\n", "\n") == text;
        println!(
            "{}",
            if same {
                "census matches"
            } else {
                "census DIFFERS"
            }
        );
        return Ok(i32::from(!same));
    }
    print!("{text}");
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_returns_the_value_after_the_name() {
        let args: Vec<String> = ["bin", "x.ns4b", "--out", "a.csv"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(flag(&args, "--out").map(String::as_str), Some("a.csv"));
        assert!(flag(&args, "--check").is_none());
    }

    #[test]
    fn a_missing_backup_is_an_error_not_a_panic() {
        assert!(census("no/such/backup.ns4b").is_err());
    }
}
