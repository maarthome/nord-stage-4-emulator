//! Per-user remembered choices for the front ends (D129): only the **path** of the user's Nord OS
//! update file, in `<config dir>/ns4/config.json`. The firmware and the tables read from it are
//! never stored (D22). Libraries and tests never read this file: only the `ns4` and `nord-stage4`
//! binaries call [`apply_saved_default`] once at start-up.
//!
//! Precedence for the WAVE-bank source, first match wins: `--synth-wave-tables PATH`, then
//! `NS4_WAVE_TABLES`, then this file, then the placeholder table. The flag already wins inside
//! `render_opts`; [`apply_saved_default`] only fills the environment variable when it is unset, so
//! the other rules fall out unchanged.

use crate::render_opts_wave::WAVE_TABLES_ENV;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The saved settings.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfig {
    /// Path of the user's Nord OS update file (`.exe`, `.dmg`, `.zip` or extracted container).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_update_path: Option<String>,
}

/// Config directory for the given platform inputs (pure, for tests): Windows `%APPDATA%`, macOS
/// `~/Library/Application Support`, elsewhere `$XDG_CONFIG_HOME` or `~/.config`.
pub fn config_path_from(
    windows: bool,
    macos: bool,
    appdata: Option<&str>,
    xdg: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    let some = |v: Option<&str>| {
        v.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    let base = if windows {
        some(appdata)?
    } else if macos {
        some(home)?.join("Library").join("Application Support")
    } else {
        some(xdg).or_else(|| some(home).map(|h| h.join(".config")))?
    };
    Some(base.join("ns4").join("config.json"))
}

/// The config file of the current user, or `None` when the platform gives no home.
pub fn default_config_path() -> Option<PathBuf> {
    let var = |k: &str| std::env::var(k).ok();
    config_path_from(
        cfg!(windows),
        cfg!(target_os = "macos"),
        var("APPDATA").as_deref(),
        var("XDG_CONFIG_HOME").as_deref(),
        var("HOME").or_else(|| var("USERPROFILE")).as_deref(),
    )
}

/// Loads `path`; a missing file is the empty config, an unreadable or malformed one an error.
pub fn load(path: &Path) -> Result<UserConfig, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(UserConfig::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Writes `cfg` to `path`, creating the directory.
pub fn save(path: &Path, cfg: &UserConfig) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// The saved update-file path that applies now: `None` when `env` (the value of `NS4_WAVE_TABLES`)
/// is set and non-empty (the variable wins), else the path in the config file at `config`.
pub fn saved_default(env: Option<&str>, config: &Path) -> Option<PathBuf> {
    if env.is_some_and(|v| !v.trim().is_empty()) {
        return None;
    }
    load(config)
        .ok()?
        .os_update_path
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
}

/// Binaries only, once, before any thread starts and before options are parsed: fills
/// `NS4_WAVE_TABLES` from the saved config when the variable is unset. Returns the path applied.
pub fn apply_saved_default() -> Option<PathBuf> {
    let cfg = default_config_path()?;
    let path = saved_default(std::env::var(WAVE_TABLES_ENV).ok().as_deref(), &cfg)?;
    std::env::set_var(WAVE_TABLES_ENV, &path);
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ns4-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("ns4").join("config.json")
    }

    #[test]
    fn the_config_dir_follows_each_platform() {
        let p = |w, m, a, x, h| config_path_from(w, m, a, x, h);
        assert_eq!(
            p(true, false, Some("C:/Users/u/AppData/Roaming"), None, None),
            Some(PathBuf::from("C:/Users/u/AppData/Roaming/ns4/config.json"))
        );
        assert_eq!(
            p(false, true, None, None, Some("/Users/u")),
            Some(PathBuf::from(
                "/Users/u/Library/Application Support/ns4/config.json"
            ))
        );
        assert_eq!(
            p(false, false, None, Some("/x"), Some("/home/u")),
            Some(PathBuf::from("/x/ns4/config.json"))
        );
        assert_eq!(
            p(false, false, None, Some(" "), Some("/home/u")),
            Some(PathBuf::from("/home/u/.config/ns4/config.json"))
        );
        assert_eq!(p(false, false, None, None, None), None);
    }

    #[test]
    fn save_then_load_round_trips_and_a_missing_file_is_empty() {
        let path = tmp("roundtrip");
        assert_eq!(load(&path).unwrap(), UserConfig::default());
        let cfg = UserConfig {
            os_update_path: Some("C:/dl/Update.exe".into()),
        };
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path).unwrap(), cfg);
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).unwrap_err().contains("config.json"));
    }

    #[test]
    fn the_variable_wins_over_the_saved_path() {
        let path = tmp("precedence");
        save(
            &path,
            &UserConfig {
                os_update_path: Some("/saved/os.exe".into()),
            },
        )
        .unwrap();
        assert_eq!(
            saved_default(None, &path),
            Some(PathBuf::from("/saved/os.exe"))
        );
        assert_eq!(
            saved_default(Some(""), &path),
            Some(PathBuf::from("/saved/os.exe"))
        );
        assert_eq!(saved_default(Some("/env/os.exe"), &path), None);
        assert_eq!(saved_default(None, Path::new("no/such/config.json")), None);
    }

    #[test]
    fn an_unset_path_is_not_written() {
        let path = tmp("empty");
        save(&path, &UserConfig::default()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "{}");
        assert_eq!(saved_default(None, &path), None);
    }
}
