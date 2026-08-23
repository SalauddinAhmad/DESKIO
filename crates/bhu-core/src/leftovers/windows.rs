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
use winreg::enums::*;
use winreg::RegKey;

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

/// Registry keys an application has left behind.
///
/// Uninstallers routinely remove their files and leave their settings. Those
/// keys are harmless in themselves but they are the reason a reinstalled
/// application remembers an expired trial, or a stale licence, or settings the
/// user thought they had cleared.
///
/// The same matcher decides ownership here as for files, and the same rule
/// about shared vendors applies: `Software\Adobe` holding several products is
/// never itself removable — only the product key inside it.
fn registry_leftovers(tokens: &AppTokens, others: &[AppTokens]) -> Vec<Leftover> {
    // (hive handle, hive name for display, path under it)
    let roots: [(RegKey, &str, &str); 3] = [
        (RegKey::predef(HKEY_CURRENT_USER), "HKCU", "Software"),
        (RegKey::predef(HKEY_LOCAL_MACHINE), "HKLM", "SOFTWARE"),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            "HKLM",
            "SOFTWARE\\WOW6432Node",
        ),
    ];

    let mut found: Vec<Leftover> = Vec::new();
    let mut names: Vec<String> = Vec::new();

    for (root, hive, root_path) in roots {
        let Ok(container) = root.open_subkey(root_path) else {
            continue;
        };
        for name in container.enum_keys().flatten() {
            let full = format!("{hive}\\{root_path}\\{name}");

            if let Some(m) = classify(&name, tokens) {
                if let Some(l) = registry_leftover(&full, &name, hive, m.confidence, m.reason) {
                    found.push(l);
                    names.push(name.clone());
                }
                continue;
            }

            // A vendor key holds one subkey per product. The vendor key itself
            // is never taken; the product key inside it is.
            if !super::is_vendor_dir(&name, tokens) {
                continue;
            }
            let Ok(vendor) = container.open_subkey(&name) else {
                continue;
            };
            for child in vendor.enum_keys().flatten() {
                let Some(cm) = super::classify_in_vendor_dir(&child, tokens) else {
                    continue;
                };
                let child_full = format!("{full}\\{child}");
                if let Some(l) =
                    registry_leftover(&child_full, &child, hive, cm.confidence, cm.reason)
                {
                    found.push(l);
                    names.push(child);
                }
            }
        }
    }

    resolve_sharing(&mut found, &names, others);
    found
}

/// Build a registry leftover, refusing anything the safety rules would not
/// allow us to remove — showing a key we could never touch is only noise.
fn registry_leftover(
    full: &str,
    name: &str,
    hive: &str,
    confidence: Confidence,
    reason: String,
) -> Option<Leftover> {
    if safety::check_registry_removable(full).is_err() {
        return None;
    }
    Some(Leftover {
        path: PathBuf::from(full),
        name: name.to_string(),
        // A registry key has no size worth reporting, and claiming "Zero KB"
        // would read as though it were empty.
        size_bytes: 0,
        size_unknown: false,
        is_directory: false,
        kind: LeftoverKind::RegistryKey,
        confidence,
        reason,
        // Machine-wide keys need an administrator, exactly as machine-wide
        // files do.
        requires_admin: hive.eq_ignore_ascii_case("HKLM"),
        shared_with: Vec::new(),
        registry_key: Some(full.to_string()),
    })
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
                            registry_key: None,
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
                registry_key: None,
                path,
            });
            names.push(name);
        }
    }

    let names: Vec<String> = found.iter().map(|l| l.name.clone()).collect();
    resolve_sharing(&mut found, &names, &others);

    // Registry keys resolve their own sharing, so they are added after.
    found.extend(registry_leftovers(&tokens, &others));

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
    }

    groups.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
    groups
}
