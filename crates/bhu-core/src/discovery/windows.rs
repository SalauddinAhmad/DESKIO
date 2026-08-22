//! Windows app discovery.
//!
//! Written on macOS and compile-verified for the Windows target; it has not yet
//! been run on Windows. Anything it gets wrong is contained here — the model,
//! the safety rules, the matching, the UI and the removal engine are shared and
//! already exercised.

use crate::discovery::ScanOptions;
use crate::fsutil;
use crate::model::*;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

pub fn installed_apps(opts: ScanOptions) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();

    // Machine-wide installs, the 32-bit mirror on 64-bit Windows, and per-user
    // installs. The same product can appear in more than one; the first wins.
    let sources = [
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL),
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL_WOW),
        (RegKey::predef(HKEY_CURRENT_USER), UNINSTALL),
    ];

    for (root, path) in sources {
        let Ok(container) = root.open_subkey(path) else {
            continue;
        };
        for name in container.enum_keys().flatten() {
            let Ok(key) = container.open_subkey(&name) else {
                continue;
            };
            let Some(app) = read_entry(&key, &name, opts) else {
                continue;
            };
            if app.is_system && !opts.include_system {
                continue;
            }
            if apps.iter().any(|a| a.id == app.id) {
                continue;
            }
            apps.push(app);
        }
    }
    apps
}

fn read_entry(key: &RegKey, key_name: &str, opts: ScanOptions) -> Option<InstalledApp> {
    let get = |value: &str| key.get_value::<String, _>(value).ok();
    let dword = |value: &str| key.get_value::<u32, _>(value).ok();

    // No DisplayName means this is a component, not something a person
    // installed and would recognise.
    let name = get("DisplayName").filter(|n| !n.trim().is_empty())?;

    // Updates and patches are nested under the product they belong to; listing
    // them separately would fill the list with things that cannot be removed
    // on their own.
    if get("ParentKeyName").is_some() || dword("SystemComponent") == Some(1) {
        return None;
    }

    let install_location = get("InstallLocation")
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .filter(|p| p.exists());

    let size_bytes = if opts.compute_sizes {
        match &install_location {
            Some(path) => fsutil::size_on_disk(path),
            // EstimatedSize is in KB and frequently stale, but it is better
            // than reporting nothing at all.
            None => dword("EstimatedSize")
                .map(|kb| kb as u64 * 1024)
                .unwrap_or(0),
        }
    } else {
        0
    };

    let publisher = get("Publisher");
    let is_system = publisher
        .as_deref()
        .map(|p| p.eq_ignore_ascii_case("Microsoft Corporation") && name.starts_with("Windows "))
        .unwrap_or(false);

    Some(InstalledApp {
        id: key_name.to_string(),
        name,
        path: install_location,
        // Windows has no bundle identifier. Matching therefore leans on the
        // display name and publisher, which is why leftover confidence is
        // generally lower here than on macOS.
        bundle_id: None,
        executable: get("DisplayIcon")
            .and_then(|icon| {
                let path = icon.split(',').next().unwrap_or(&icon).trim().to_string();
                PathBuf::from(path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
            })
            .filter(|s| !s.is_empty()),
        version: get("DisplayVersion"),
        publisher,
        size_bytes,
        source: if get("WindowsInstaller").is_some() || dword("WindowsInstaller") == Some(1) {
            AppSource::WindowsMsi
        } else {
            AppSource::WindowsRegistry
        },
        icon_png_base64: None,
        created_at: None,
        modified_at: None,
        last_opened_at: None,
        notarized: None,
        is_running: false,
        is_system,
    })
}

/// The vendor's own uninstaller, which must run before any leftover sweep.
///
/// Windows apps are not a folder we can move to the Recycle Bin — they have an
/// uninstaller that knows how to undo what their installer did. Removing the
/// files ourselves would leave the registry describing software that is no
/// longer there.
pub fn uninstall_command(app: &InstalledApp) -> Option<String> {
    let sources = [
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL),
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL_WOW),
        (RegKey::predef(HKEY_CURRENT_USER), UNINSTALL),
    ];
    for (root, path) in sources {
        let Ok(container) = root.open_subkey(path) else {
            continue;
        };
        let Ok(key) = container.open_subkey(&app.id) else {
            continue;
        };
        // The quiet form runs without the vendor's own wizard where offered.
        if let Ok(quiet) = key.get_value::<String, _>("QuietUninstallString") {
            return Some(quiet);
        }
        if let Ok(normal) = key.get_value::<String, _>("UninstallString") {
            return Some(normal);
        }
    }
    None
}

pub fn enrich(app: &mut InstalledApp) {
    if app.size_bytes == 0 {
        if let Some(path) = app.path.clone() {
            app.size_bytes = fsutil::size_on_disk(&path);
        }
    }
}

pub fn icon(_app: &InstalledApp) -> Option<String> {
    // Extracting an icon resource from a PE binary needs more than the standard
    // library offers; the list falls back to initials until it is done.
    None
}
