//! Where Windows apps leave things.
//!
//! Compile-verified for the Windows target, not yet run on Windows.
//!
//! Windows apps have no bundle identifier, so [`super::classify`] has only the
//! display name and the publisher to work with. Matches are correspondingly
//! weaker than on macOS — expect `Medium` where a Mac would give `High` — and
//! the shared-vendor guard matters more, not less.

use super::{classify, resolve_sharing, AppTokens, OrphanGroup};
use crate::fsutil;
use crate::model::*;
use crate::safety;
use std::path::PathBuf;

fn roots() -> Vec<(PathBuf, LeftoverKind)> {
    let mut v: Vec<(PathBuf, LeftoverKind)> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        v.push((
            home.join(r"AppData\Roaming"),
            LeftoverKind::ApplicationSupport,
        ));
        v.push((home.join(r"AppData\Local"), LeftoverKind::Caches));
        v.push((home.join(r"AppData\LocalLow"), LeftoverKind::Caches));
    }
    if let Some(data) = std::env::var_os("PROGRAMDATA") {
        v.push((PathBuf::from(data), LeftoverKind::ApplicationSupport));
    }
    // Remnant install directories: the vendor's uninstaller has run, but left
    // a folder behind.
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(dir) = std::env::var_os(var) {
            v.push((PathBuf::from(dir), LeftoverKind::Other));
        }
    }
    v
}

/// Folders in these roots that belong to Windows rather than to an app.
fn is_system_owned(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "microsoft"
            | "windows"
            | "windowsapps"
            | "packages"
            | "temp"
            | "common files"
            | "internet explorer"
            | "windows defender"
            | "windows nt"
            | "windowspowershell"
            | "ssh"
            | "desktop.ini"
    ) || lower.starts_with('.')
}

pub fn for_app(app: &InstalledApp, all_apps: &[InstalledApp]) -> Vec<Leftover> {
    let tokens = AppTokens::from_app(app);
    let others: Vec<AppTokens> = all_apps
        .iter()
        .filter(|a| a.id != app.id)
        .map(AppTokens::from_app)
        .collect();

    let mut found: Vec<Leftover> = Vec::new();
    let mut names: Vec<String> = Vec::new();

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

            // A publisher folder holds one directory per product — the same
            // shape as a macOS vendor folder, and handled the same way: the
            // folder itself is never removable, only this app's own directory
            // inside it.
            let Some(m) = classify(&name, &tokens) else {
                if path.is_dir() && super::is_vendor_dir(&name, &tokens) {
                    for child in fsutil::children(&path) {
                        let Some(cname) = child
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(str::to_string)
                        else {
                            continue;
                        };
                        if safety::check_removable(&child).is_err() {
                            continue;
                        }
                        let Some(cm) = super::classify_in_vendor_dir(&cname, &tokens) else {
                            continue;
                        };
                        let (size, size_unknown) = fsutil::size_with_confidence(&child);
                        found.push(Leftover {
                            name: cname.clone(),
                            size_bytes: size,
                            size_unknown,
                            is_directory: child.is_dir(),
                            kind,
                            confidence: cm.confidence,
                            reason: cm.reason,
                            requires_admin: safety::requires_admin(&child),
                            shared_with: Vec::new(),
                            path: child,
                        });
                        names.push(cname);
                    }
                }
                continue;
            };

            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            found.push(Leftover {
                name: name.clone(),
                size_bytes: size,
                size_unknown,
                is_directory: path.is_dir(),
                kind,
                confidence: m.confidence,
                reason: m.reason,
                requires_admin: safety::requires_admin(&path),
                shared_with: Vec::new(),
                path,
            });
            names.push(name);
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

/// Orphans on Windows are folders in the app-data roots that no installed
/// program claims.
///
/// This is a weaker signal than the reverse-DNS names macOS gives us — a folder
/// called `Acme` might belong to something installed outside the registry — so
/// everything here stays low confidence and is never pre-selected.
pub fn orphans(all_apps: &[InstalledApp]) -> Vec<OrphanGroup> {
    let installed: Vec<AppTokens> = all_apps.iter().map(AppTokens::from_app).collect();
    let mut groups: Vec<OrphanGroup> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        for (root, kind) in [
            (
                home.join(r"AppData\Roaming"),
                LeftoverKind::ApplicationSupport,
            ),
            (home.join(r"AppData\Local"), LeftoverKind::Caches),
        ] {
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
                if !path.is_dir() {
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
                    reason: format!("\"{name}\" is not claimed by anything installed"),
                    requires_admin: safety::requires_admin(&path),
                    shared_with: Vec::new(),
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
    }

    groups.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
    groups
}
