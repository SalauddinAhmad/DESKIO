//! What runs when you log in.
//!
//! Launch agents, launch daemons and login items. Disabling one is always
//! reversible — the engine never deletes a startup entry to stop it running,
//! it turns it off, so it can be turned back on.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupKind {
    /// Runs when this user logs in.
    LaunchAgent,
    /// Runs at boot, as root. Turning one off needs an administrator password.
    LaunchDaemon,
    /// An app the user added to their login items.
    LoginItem,
    /// Windows: a `Run` registry value or a Startup-folder shortcut.
    RegistryRun,
    /// Linux: an XDG autostart entry.
    Autostart,
}

impl StartupKind {
    pub fn label(self) -> &'static str {
        match self {
            StartupKind::LaunchAgent => "Launch Agent",
            StartupKind::LaunchDaemon => "System Daemon",
            StartupKind::LoginItem => "User Login Item",
            StartupKind::RegistryRun => "Registry Run Entry",
            StartupKind::Autostart => "Autostart Entry",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    /// launchd label, or the login item's name. Unique within its kind.
    pub id: String,
    pub name: String,
    pub kind: StartupKind,
    /// The `.plist` (or shortcut) that defines it.
    pub path: Option<PathBuf>,
    /// The executable it launches, for the user to judge what it actually is.
    pub program: Option<String>,
    pub enabled: bool,
    /// False when the platform gives us no supported way to change it — the UI
    /// must then show the state without offering a switch that cannot work.
    pub can_toggle: bool,
    /// Why it cannot be toggled, when it cannot.
    pub locked_reason: Option<String>,
    pub requires_admin: bool,
    /// Bundle id of the app it belongs to, when that can be inferred.
    pub app_id: Option<String>,
}

/// Everything configured to run at startup.
pub fn list() -> Vec<StartupItem> {
    let mut items = imp::list();
    items.sort_by_key(|a| a.name.to_lowercase());
    items
}

/// Turn a startup item on or off. Reversible in both directions.
pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<(), String> {
    if !item.can_toggle {
        return Err(item
            .locked_reason
            .clone()
            .unwrap_or_else(|| "this item cannot be changed from here".into()));
    }
    imp::set_enabled(item, enabled)
}
