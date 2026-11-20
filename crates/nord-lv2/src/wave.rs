//! WAVE wavetables from the user's own Nord OS update file (never shipped, D22).
//!
//! Same source and precedence as `ns4` / `nord-stage4`: `$NS4_WAVE_TABLES`, else the path saved by
//! `ns4 firmware set` in `ns4/config.json`. Loaded once per process at instantiate (not real-time);
//! without a file the engine keeps the placeholder wavetable.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The OS file to read: a non-empty `env` wins, else `saved()` (pure, for tests).
pub fn tables_path(env: Option<&str>, saved: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    match env.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Some(PathBuf::from(v)),
        None => saved(),
    }
}

fn saved_path() -> Option<PathBuf> {
    let cfg = nord_core::user_config::default_config_path()?;
    nord_core::user_config::saved_default(None, &cfg)
        .ok()
        .flatten()
}

/// Same variable as `nord_core::render_opts_wave::WAVE_TABLES_ENV` (that module is private).
pub const WAVE_TABLES_ENV: &str = "NS4_WAVE_TABLES";

static INSTALLED: OnceLock<bool> = OnceLock::new();

/// Loads the tables once; true when firmware tables are available to the synth layers.
pub fn ensure_loaded() -> bool {
    *INSTALLED.get_or_init(|| {
        let env = std::env::var(WAVE_TABLES_ENV).ok();
        let Some(path) = tables_path(env.as_deref(), saved_path) else {
            return false;
        };
        nord_core::synth::prepare_wave_bank();
        match nord_core::synth::tables_from_file(&path, nord_core::synth::boot_tables()) {
            Ok(t) => {
                // false only when another plugin instance in this process installed them first
                let _ = nord_core::synth::install_wave_tables(t);
                true
            }
            Err(e) => {
                eprintln!(
                    "nord-lv2: Nord OS update file {} not used: {e}",
                    path.display()
                );
                false
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_wins_then_saved_then_none() {
        let saved = || Some(PathBuf::from("/saved.zip"));
        assert_eq!(
            tables_path(Some("/env.dmg"), saved),
            Some(PathBuf::from("/env.dmg"))
        );
        assert_eq!(
            tables_path(Some("  "), saved),
            Some(PathBuf::from("/saved.zip"))
        );
        assert_eq!(tables_path(None, || None), None);
    }
}
