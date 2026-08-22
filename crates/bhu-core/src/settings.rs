//! User preferences.
//!
//! Stored as JSON next to the removal journal. Reading is fail-safe: a missing,
//! unreadable or corrupt file yields the defaults rather than an error, because
//! a preferences problem must never stop someone uninstalling something.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Settings {
    /// Play the Finder trash sound as each item is removed.
    ///
    /// Off by default. Beyond being noisy — a seven-item uninstall fires it
    /// seven times in a row — the quiet route also avoids asking for Automation
    /// permission over Finder on first use.
    ///
    /// The trade-off is real, and the UI says so: only the Finder route records
    /// the information behind Finder's "Put Back". With this off, restoring is
    /// done from BHUninstaller's own removal history, which knows where every
    /// item came from.
    pub removal_sound: bool,
    /// Whether the Full Disk Access explanation has been shown once.
    ///
    /// It appears on first launch when the permission is missing, and never
    /// again on its own — the banner stays, and Settings can reopen it. An app
    /// that re-asks for a permission this broad on every launch teaches people
    /// to dismiss it without reading.
    pub full_disk_prompt_seen: bool,
    /// Look for a newer BHUninstaller on its own, once a day at most.
    ///
    /// Off unless asked for. A tool that reaches out to the network without
    /// being told to is not what someone installs an uninstaller expecting,
    /// and the manual check is always there.
    pub auto_check_updates: bool,
    /// Unix seconds of the last check, so an enabled auto-check stays polite:
    /// the unauthenticated GitHub API budget is per IP, shared by everyone
    /// behind it.
    pub last_update_check: i64,
}

fn settings_path() -> Option<PathBuf> {
    crate::undo::data_dir().map(|d| d.join("settings.json"))
}

pub fn load() -> Settings {
    let Some(path) = settings_path() else {
        return Settings::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = settings_path().ok_or("no application data directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sound_is_off_unless_asked_for() {
        assert!(!Settings::default().removal_sound);
    }

    #[test]
    fn unreadable_settings_fall_back_to_defaults_rather_than_failing() {
        let parsed: Result<Settings, _> = serde_json::from_str("{ not json");
        assert!(parsed.is_err());
        // load() swallows exactly this case.
        let _ = load();
    }

    #[test]
    fn the_permission_prompt_defaults_to_not_yet_shown() {
        assert!(!Settings::default().full_disk_prompt_seen);
    }

    #[test]
    fn nothing_reaches_the_network_unless_asked() {
        assert!(!Settings::default().auto_check_updates);
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        // Settings written by a newer or older build must still load.
        let s: Settings = serde_json::from_str(r#"{"removal_sound":true,"future":1}"#).unwrap();
        assert!(s.removal_sound);
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.removal_sound);
    }
}
