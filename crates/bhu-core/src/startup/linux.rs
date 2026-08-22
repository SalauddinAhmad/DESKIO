//! Linux startup items.
//!
//! Compile-verified for the Linux target, not yet run on Linux.
//!
//! Disabling a `.desktop` autostart entry writes `Hidden=true` into a copy
//! under `~/.config/autostart`. That is the XDG-defined way to mask an entry,
//! and it means a system-wide entry in `/etc/xdg/autostart` can be switched off
//! without touching `/etc` or needing root.

use super::{StartupItem, StartupKind};
use crate::fsutil;
use std::path::{Path, PathBuf};

fn user_autostart() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config/autostart"))
}

fn roots() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/etc/xdg/autostart")];
    if let Some(user) = user_autostart() {
        v.push(user);
    }
    v
}

pub fn list() -> Vec<StartupItem> {
    let mut items: Vec<StartupItem> = Vec::new();

    for root in roots() {
        for path in fsutil::children(&root) {
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let field = |key: &str| -> Option<String> {
                text.lines()
                    .find_map(|l| l.strip_prefix(&format!("{key}=")))
                    .map(|v| v.trim().to_string())
            };
            let Some(id) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };

            let hidden = field("Hidden")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let enabled_key = field("X-GNOME-Autostart-enabled")
                .map(|v| !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true);

            let item = StartupItem {
                name: field("Name").unwrap_or_else(|| id.trim_end_matches(".desktop").to_string()),
                id: id.clone(),
                kind: StartupKind::Autostart,
                program: field("Exec"),
                path: Some(path),
                enabled: !hidden && enabled_key,
                can_toggle: true,
                locked_reason: None,
                // Masking is always done in the user's own directory, so this
                // never needs root even for a system-wide entry.
                requires_admin: false,
                app_id: None,
            };

            // A user entry shadows the system one of the same name, which is
            // precisely how XDG expects overriding to work.
            match items.iter_mut().find(|i| i.id == item.id) {
                Some(existing) => *existing = item,
                None => items.push(item),
            }
        }
    }
    items
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<(), String> {
    let dir = user_autostart().ok_or("no home directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(&item.id);

    // Start from the user's copy if there is one, otherwise from the
    // system-wide entry being masked.
    let source = if target.exists() {
        target.clone()
    } else {
        item.path
            .clone()
            .ok_or("this entry has no file behind it")?
    };
    let text = std::fs::read_to_string(&source).map_err(|e| e.to_string())?;

    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.starts_with("Hidden=") && !l.starts_with("X-GNOME-Autostart-enabled="))
        .map(str::to_string)
        .collect();
    if !enabled {
        lines.push("Hidden=true".into());
        lines.push("X-GNOME-Autostart-enabled=false".into());
    }
    lines.push(String::new());

    std::fs::write(&target, lines.join("\n"))
        .map_err(|e| format!("could not write {}: {e}", target.display()))
}

#[allow(dead_code)]
fn is_desktop(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("desktop")
}
