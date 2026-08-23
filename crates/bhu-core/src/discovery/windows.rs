//! Windows app discovery.
//!
//! Written on macOS and compile-verified for the Windows target; it has not yet
//! been run on Windows. Anything it gets wrong is contained here — the model,
//! the safety rules, the matching, the UI and the removal engine are shared and
//! already exercised.

use crate::discovery::ScanOptions;
use crate::fsutil;
use crate::model::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
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

    // Two ways to get a size, and both fail often enough that neither can be
    // trusted alone: `InstallLocation` may be missing, wrong, or point at a
    // directory this process cannot read, and `EstimatedSize` may be absent or
    // stale. Measuring is preferred, the registry's own figure is the fallback,
    // and the larger wins when a walk was cut short by permissions — which is
    // how apps like Android Studio ended up reporting nothing at all.
    let size_bytes = if opts.compute_sizes {
        let walked = install_location
            .as_ref()
            .map(|path| fsutil::size_on_disk(path))
            .unwrap_or(0);
        // EstimatedSize is recorded in kibibytes.
        let estimated = dword("EstimatedSize").map(|kb| kb as u64 * 1024).unwrap_or(0);
        walked.max(estimated)
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

pub fn icon(app: &InstalledApp) -> Option<String> {
    icons(std::slice::from_ref(app)).into_values().next()
}

/// Icons for the whole list, extracted in one pass.
///
/// Windows keeps an app's icon inside its executable, and pulling one out means
/// Win32 interop — or one line of .NET. PowerShell has that .NET available, so
/// it does the work; but starting PowerShell costs a few hundred milliseconds,
/// which across a hundred apps would be half a minute of placeholders. So a
/// single script handles every app at once and Rust reads the results.
///
/// The icons come out at 32x32, which is what `ExtractAssociatedIcon` gives for
/// an executable. Sharper would need `SHGetFileInfo` and real interop.
pub fn icons(apps: &[InstalledApp]) -> HashMap<String, String> {
    let sources: Vec<(String, String)> = apps
        .iter()
        .filter_map(|a| icon_source(a).map(|src| (a.id.clone(), src)))
        .collect();
    if sources.is_empty() {
        return HashMap::new();
    }

    // A directory of our own, created fresh, so nothing can pre-place a file
    // for us to read back as an icon.
    let dir = std::env::temp_dir().join(format!("bhu-icons-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return HashMap::new();
    }

    let mut script = String::from(
        "$ErrorActionPreference='SilentlyContinue'\nAdd-Type -AssemblyName System.Drawing\n",
    );
    for (i, (_, src)) in sources.iter().enumerate() {
        script.push_str(&format!(
            "try {{\n  $p = {src}\n  if ($p.ToLower().EndsWith('.ico')) {{ $ic = New-Object              System.Drawing.Icon($p) }} else {{ $ic =              [System.Drawing.Icon]::ExtractAssociatedIcon($p) }}\n  if ($ic -ne $null) {{ $b =              $ic.ToBitmap(); $b.Save({out}, [System.Drawing.Imaging.ImageFormat]::Png);              $b.Dispose(); $ic.Dispose() }}\n}} catch {{}}\n",
            src = ps_quote(src),
            out = ps_quote(&dir.join(format!("{i}.png")).to_string_lossy()),
        ));
    }

    let script_path = dir.join("extract.ps1");
    if std::fs::write(&script_path, script).is_err() {
        return HashMap::new();
    }

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .output();

    let mut out = HashMap::new();
    for (i, (id, _)) in sources.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(dir.join(format!("{i}.png"))) {
            if !bytes.is_empty() {
                out.insert(id.clone(), crate::base64_encode(&bytes));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// A single-quoted PowerShell string. Only `'` is special inside one, and it is
/// escaped by doubling — so a path can never break out of the literal.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Where this app's icon lives: the registry's `DisplayIcon` if it has one,
/// otherwise the most likely executable in its install directory.
fn icon_source(app: &InstalledApp) -> Option<String> {
    if let Some(display_icon) = registry_value(&app.id, "DisplayIcon") {
        // `DisplayIcon` is often `C:\path\app.exe,0` — the index selects which
        // icon in the file, and is not part of the path.
        let path = display_icon
            .rsplit_once(',')
            .map(|(p, _)| p)
            .unwrap_or(&display_icon)
            .trim()
            .trim_matches('"')
            .to_string();
        if !path.is_empty() && Path::new(&path).is_file() {
            return Some(path);
        }
    }

    // No usable DisplayIcon: look for an executable named after the app, then
    // fall back to any executable that is not obviously an uninstaller.
    let dir = app.path.as_ref()?;
    let wanted = crate::leftovers::slug(&app.name);
    let mut fallback: Option<String> = None;
    for entry in fsutil::children(dir) {
        if entry.extension().and_then(|e| e.to_str()) != Some("exe") {
            continue;
        }
        let stem = entry.file_stem()?.to_string_lossy().to_string();
        let slug = crate::leftovers::slug(&stem);
        if slug.contains("uninst") || slug.contains("setup") || slug.contains("crashpad") {
            continue;
        }
        if !wanted.is_empty() && (wanted.contains(&slug) || slug.contains(&wanted)) {
            return Some(entry.to_string_lossy().to_string());
        }
        fallback.get_or_insert_with(|| entry.to_string_lossy().to_string());
    }
    fallback
}

/// Read one value from this app's uninstall key, wherever it lives.
fn registry_value(key_name: &str, value: &str) -> Option<String> {
    for (root, path) in [
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL),
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL_WOW),
        (RegKey::predef(HKEY_CURRENT_USER), UNINSTALL),
    ] {
        if let Ok(container) = root.open_subkey(path) {
            if let Ok(key) = container.open_subkey(key_name) {
                if let Ok(v) = key.get_value::<String, _>(value) {
                    if !v.trim().is_empty() {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}
