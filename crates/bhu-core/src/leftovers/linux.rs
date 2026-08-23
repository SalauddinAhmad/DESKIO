//! Where Linux apps leave things.
//!
//! Compile-verified for the Linux target, not yet run on Linux.
//!
//! A package manager removes its own files, so what matters here is the
//! per-user configuration and cache that `apt remove` deliberately leaves
//! behind — which is exactly what people are surprised to still find.

use super::{classify, resolve_sharing, AppTokens, OrphanGroup};
use crate::fsutil;
use crate::model::*;
use crate::safety;
use std::path::PathBuf;

fn roots() -> Vec<(PathBuf, LeftoverKind)> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    vec![
        (home.join(".config"), LeftoverKind::Preferences),
        (home.join(".local/share"), LeftoverKind::ApplicationSupport),
        (home.join(".local/state"), LeftoverKind::Logs),
        (home.join(".cache"), LeftoverKind::Caches),
        (home.join(".var/app"), LeftoverKind::Container),
    ]
}

/// Directories in these roots that belong to the desktop environment rather
/// than to any one app.
fn is_system_owned(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with('.')
        || matches!(
            lower.as_str(),
            "systemd"
                | "dconf"
                | "pulse"
                | "fontconfig"
                | "gtk-3.0"
                | "gtk-4.0"
                | "mimeapps.list"
                | "user-dirs.dirs"
                | "user-dirs.locale"
                | "trash"
                | "keyrings"
                | "applications"
                | "icons"
                | "themes"
                | "fonts"
                | "mime"
        )
}

pub fn for_app(app: &InstalledApp, all_apps: &[InstalledApp]) -> Vec<Leftover> {
    let tokens = AppTokens::from_app(app);
    let others: Vec<AppTokens> = all_apps
        .iter()
        .filter(|a| a.id != app.id)
        .map(AppTokens::from_app)
        .collect();

    let mut found: Vec<Leftover> = Vec::new();

    for (root, kind) in roots() {
        for path in fsutil::children(&root) {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if is_system_owned(&name) || safety::check_removable(&path).is_err() {
                continue;
            }
            let Some(m) = classify(&name, &tokens) else {
                continue;
            };
            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            found.push(Leftover {
                name,
                size_bytes: size,
                size_unknown,
                is_directory: path.is_dir(),
                kind,
                confidence: m.confidence,
                reason: m.reason,
                requires_admin: safety::requires_admin(&path),
                shared_with: Vec::new(),
                registry_key: None,
                path,
            });
        }
    }

    let names: Vec<String> = found.iter().map(|l| l.name.clone()).collect();
    resolve_sharing(&mut found, &names, &others);
    for l in found.iter_mut() {
        if l.shared_with.is_empty() && l.confidence == Confidence::Medium {
            l.confidence = Confidence::High;
        }
    }
    found.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(b.size_bytes.cmp(&a.size_bytes))
    });
    found
}

pub fn orphans(all_apps: &[InstalledApp]) -> Vec<OrphanGroup> {
    let installed: Vec<AppTokens> = all_apps.iter().map(AppTokens::from_app).collect();
    let mut groups: Vec<OrphanGroup> = Vec::new();

    for (root, kind) in roots() {
        for path in fsutil::children(&root) {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if is_system_owned(&name) || !path.is_dir() {
                continue;
            }
            if safety::check_removable(&path).is_err() {
                continue;
            }
            if installed.iter().any(|t| classify(&name, t).is_some()) {
                continue;
            }
            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            if size == 0 {
                continue;
            }
            let leftover = Leftover {
                name: name.clone(),
                size_bytes: size,
                size_unknown,
                is_directory: true,
                kind,
                confidence: Confidence::Medium,
                reason: format!("\"{name}\" is not claimed by any installed package"),
                requires_admin: safety::requires_admin(&path),
                shared_with: Vec::new(),
                registry_key: None,
                path,
            };
            match groups.iter_mut().find(|g| g.name == name) {
                Some(g) => {
                    g.size_bytes += leftover.size_bytes;
                    g.items.push(leftover);
                }
                None => groups.push(OrphanGroup {
                    name,
                    size_bytes: leftover.size_bytes,
                    items: vec![leftover],
                }),
            }
        }
    }

    groups.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
    groups
}
