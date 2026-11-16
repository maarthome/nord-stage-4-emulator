//! Prints the Piano Touch census of every `.ns4p` program of a Nord Stage 4 backup.
//!
//! Usage: `cargo run --release -p nord-tools --bin piano_touch_census -- <backup.ns4b>`.
//! Derived numbers and program names only.

use nord_core::cbin::{CbinFile, Ns4BackupArchive};
use nord_tools::touch_census::{facts, report};
use std::fs::File;

/// Program name of an archive entry path (`Program/Bank A/Name.ns4p` gives `Name`).
fn program_stem(entry: &str) -> String {
    let file = entry.rsplit('/').next().unwrap_or(entry);
    file.trim_end_matches(".ns4p").to_string()
}

fn run() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("usage: <backup.ns4b>")?;
    let archive = Ns4BackupArchive::open(&path).map_err(|e| e.to_string())?;
    let mut file = File::open(&path).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for e in archive.entries_with_ext(".ns4p") {
        let bytes = archive
            .read_entry(&mut file, &e.name)
            .map_err(|err| err.to_string())?;
        let prog = CbinFile::parse(&bytes)
            .and_then(|c| c.decode_program())
            .map_err(|err| format!("{}: {err}", e.name))?;
        let mut row = facts(&prog);
        row.sound_a = archive.resolve_program_sounds(&prog).piano_a;
        row.name = program_stem(&e.name);
        rows.push(row);
    }
    rows.sort_by(|a, b| a.slot.cmp(&b.slot));
    print!("{}", report(&rows));
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
    fn program_stem_drops_the_folders_and_the_extension() {
        assert_eq!(
            program_stem("Program/Bank A/Pearl Upright.ns4p"),
            "Pearl Upright"
        );
        assert_eq!(program_stem("Plain"), "Plain");
    }
}
