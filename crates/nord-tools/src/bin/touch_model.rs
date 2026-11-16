//! Prints what the Rust Piano Touch model gives for a note and velocities, to compare with the
//! firmware harness `tools/arm_program_touch.py`.
//!
//! Usage: `touch_model <backup.ns4b> <npno member> <note> <touch 0|1|2> <velocity>...`.
//! Output rows: `velocity T X voice amplitude_q23 ...` (derived numbers only).

use nord_core::cbin::Ns4BackupArchive;
use nord_core::piano::npno_level::NpnoLevel;
use nord_core::piano::touch_curve::TouchCurve;
use nord_tools::touch_census::model_row;
use std::fs::File;

/// The Touch curve of the edit value `0`, `1` or `2`.
fn parse_touch(text: &str) -> Result<TouchCurve, String> {
    match text {
        "0" => Ok(TouchCurve::Heavy),
        "1" => Ok(TouchCurve::Medium),
        "2" => Ok(TouchCurve::Light),
        _ => Err("touch must be 0, 1 or 2".into()),
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 {
        return Err("usage: <backup.ns4b> <npno member> <note> <touch 0|1|2> <velocity>...".into());
    }
    let archive = Ns4BackupArchive::open(&args[1]).map_err(|e| e.to_string())?;
    let mut file = File::open(&args[1]).map_err(|e| e.to_string())?;
    let bytes = archive
        .read_entry(&mut file, &args[2])
        .map_err(|e| e.to_string())?;
    let level = NpnoLevel::parse(&bytes).ok_or("npno level model did not parse")?;
    let note: u8 = args[3].parse().map_err(|_| "note")?;
    let touch = parse_touch(&args[4])?;
    for v in &args[5..] {
        let v: u8 = v.parse().map_err(|_| "velocity")?;
        let row = model_row(&level, note, v, touch);
        println!("{row}");
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_edit_values_map_to_the_curves_and_others_are_rejected() {
        assert_eq!(parse_touch("0"), Ok(TouchCurve::Heavy));
        assert_eq!(parse_touch("1"), Ok(TouchCurve::Medium));
        assert_eq!(parse_touch("2"), Ok(TouchCurve::Light));
        assert!(parse_touch("3").is_err());
    }
}
