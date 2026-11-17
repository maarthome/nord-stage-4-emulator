//! Default source of the WAVE-bank tables: the user's own OS image named by an environment variable.
//!
//! The 46 WAVE tables are firmware rodata and are never committed (D22, AGENTS.md rule 6). The
//! flag `--synth-wave-tables PATH` loads them from the user's OS image; this module lets the
//! environment variable `NS4_WAVE_TABLES` do the same without a flag, so the firmware bank is the
//! default wherever the user has pointed at their image and the 64-table placeholder stays the
//! default everywhere else (CI, tests, a fresh clone). Nothing is read unless the variable is set,
//! so rendering stays deterministic and the committed goldens do not change.

use std::path::PathBuf;

/// Name of the environment variable that points at the user's OS image (container or `bank0_CODE`).
pub const WAVE_TABLES_ENV: &str = "NS4_WAVE_TABLES";

/// The image path to load by default, or `None` when the placeholder stays.
///
/// `explicit_flag` is true when `--synth-wave-tables` was given (the flag wins); `disabled` is
/// true for `--no-synth-wave-bank`. An empty variable counts as unset.
pub fn default_tables_path(
    explicit_flag: bool,
    disabled: bool,
    env: Option<&str>,
) -> Option<PathBuf> {
    if explicit_flag || disabled {
        return None;
    }
    env.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_or_empty_variable_keeps_the_placeholder() {
        assert_eq!(default_tables_path(false, false, None), None);
        assert_eq!(default_tables_path(false, false, Some("")), None);
        assert_eq!(default_tables_path(false, false, Some("  ")), None);
    }

    #[test]
    fn a_set_variable_names_the_image() {
        assert_eq!(
            default_tables_path(false, false, Some("C:/fw/os.bin")),
            Some(PathBuf::from("C:/fw/os.bin"))
        );
    }

    #[test]
    fn the_flag_and_the_off_switch_win_over_the_variable() {
        assert_eq!(default_tables_path(true, false, Some("x")), None);
        assert_eq!(default_tables_path(false, true, Some("x")), None);
    }

    #[test]
    fn the_variable_name_is_stable() {
        assert_eq!(WAVE_TABLES_ENV, "NS4_WAVE_TABLES");
    }
}
