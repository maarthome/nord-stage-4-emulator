//! The program folder: which `.ns4p` files the `program` port selects, in which order.
//!
//! Shared verbatim by the plugin and the GUI (`nord-lv2-ui` includes this file with `#[path]`), so
//! both number the programs the same way. Uses only `std`: no engine code.

use std::path::{Path, PathBuf};

/// Overrides the program folder.
pub const PROGRAM_DIR_ENV: &str = "NS4_PROGRAM_DIR";
/// Sub-folders deeper than this are not scanned.
const MAX_DEPTH: usize = 3;

/// Program folder for the given inputs (pure, for tests): `$NS4_PROGRAM_DIR`, else
/// `$XDG_CONFIG_HOME/ns4/programs`, else `~/.config/ns4/programs` (next to the `ns4/config.json`
/// that `ns4 firmware set` writes). Empty values count as unset.
pub fn program_dir_from(
    env: Option<&str>,
    xdg: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    let some = |v: Option<&str>| {
        v.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = some(env) {
        return Some(dir);
    }
    let base = some(xdg).or_else(|| some(home).map(|h| h.join(".config")))?;
    Some(base.join("ns4").join("programs"))
}

/// The program folder of this user.
pub fn program_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var(k).ok();
    program_dir_from(
        var(PROGRAM_DIR_ENV).as_deref(),
        var("XDG_CONFIG_HOME").as_deref(),
        var("HOME").as_deref(),
    )
}

fn is_program(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ns4p"))
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if depth < MAX_DEPTH {
                scan(&p, depth + 1, out);
            }
        } else if is_program(&p) {
            out.push(p);
        }
    }
}

/// All `.ns4p` files under `dir` (sub-folders included, e.g. `Bank A/`), sorted by their path
/// relative to `dir`, case-insensitively. Program number N is entry N-1.
pub fn list_programs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    scan(dir, 0, &mut out);
    let key = |p: &PathBuf| {
        p.strip_prefix(dir)
            .unwrap_or(p)
            .to_string_lossy()
            .to_lowercase()
    };
    out.sort_by_key(key);
    out
}

/// Name shown for a program: its file name without `.ns4p`.
pub fn display_name(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_precedence() {
        let p = |e, x, h| program_dir_from(e, x, h).map(|p| p.to_string_lossy().into_owned());
        assert_eq!(
            p(Some("/x/progs"), Some("/c"), Some("/h")).as_deref(),
            Some("/x/progs")
        );
        assert_eq!(
            p(Some(" "), Some("/c"), Some("/h")).as_deref(),
            Some("/c/ns4/programs")
        );
        assert_eq!(
            p(None, None, Some("/home/maart")).as_deref(),
            Some("/home/maart/.config/ns4/programs")
        );
        assert_eq!(p(None, None, None), None);
    }

    #[test]
    fn lists_ns4p_recursively_sorted_case_insensitive() {
        let dir = std::env::temp_dir().join(format!("nord-lv2-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Bank B")).unwrap();
        std::fs::create_dir_all(dir.join("a/b/c/d")).unwrap();
        for f in [
            "zeta.ns4p",
            "Alpha.NS4P",
            "Bank B/Organ.ns4p",
            "notes.txt",
            "a/b/c/d/deep.ns4p",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let names: Vec<String> = list_programs(&dir)
            .iter()
            .map(|p| display_name(p))
            .collect();
        assert_eq!(
            names,
            ["Alpha", "Organ", "zeta"],
            "deep.ns4p is below MAX_DEPTH"
        );
        assert!(list_programs(&dir.join("missing")).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
