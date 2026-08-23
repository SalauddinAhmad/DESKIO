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
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL, "All users"),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            UNINSTALL_WOW,
            "All users",
        ),
        (RegKey::predef(HKEY_CURRENT_USER), UNINSTALL, "You"),
    ];

    for (root, path, scope) in sources {
        let Ok(container) = root.open_subkey(path) else {
            continue;
        };
        for name in container.enum_keys().flatten() {
            let Ok(key) = container.open_subkey(&name) else {
                continue;
            };
            let Some(mut app) = read_entry(&key, &name, opts) else {
                continue;
            };
            // The same product can be installed both per-machine and per-user.
            // Both entries are real, so both are listed — saying who each was
            // installed for is what makes them tellable apart.
            app.scope = Some(scope.to_string());
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
    // `InstallLocation` is missing for plenty of installers — Android Studio
    // among them — so where it is absent the directory is inferred from the
    // paths the entry does record. Used only for measuring and for finding an
    // icon: `path` stays whatever the registry actually claims, because that is
    // what a removal would act on.
    let probable_dir = install_location
        .clone()
        .or_else(|| infer_install_dir(&get("DisplayIcon"), &get("UninstallString")));

    let size_bytes = if opts.compute_sizes {
        let walked = probable_dir
            .as_ref()
            .map(|path| fsutil::size_on_disk(path))
            .unwrap_or(0);
        // EstimatedSize is recorded in kibibytes.
        let estimated = dword("EstimatedSize")
            .map(|kb| kb as u64 * 1024)
            .unwrap_or(0);
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
        path: install_location.clone(),
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
        // Filled in by the caller, which knows which hive this came from.
        scope: None,
    })
}

/// Work out where an app lives when the registry does not say.
///
/// `InstallLocation` is optional and plenty of installers leave it out, but an
/// entry almost always records a path *somewhere* — the icon it displays, or
/// the uninstaller it runs. The directory containing either is usually the
/// install directory, or one level below it.
fn infer_install_dir(display_icon: &Option<String>, uninstall: &Option<String>) -> Option<PathBuf> {
    for candidate in [display_icon, uninstall] {
        let Some(raw) = candidate else { continue };
        let Some(exe) = executable_path(raw) else {
            continue;
        };
        let Some(dir) = exe.parent() else { continue };
        if !dir.is_dir() {
            continue;
        }
        // Binaries commonly sit in a subdirectory of the install root.
        let dir = match dir.file_name().and_then(|n| n.to_str()) {
            Some(n)
                if n.eq_ignore_ascii_case("bin")
                    || n.eq_ignore_ascii_case("app")
                    || n.eq_ignore_ascii_case("current") =>
            {
                dir.parent().unwrap_or(dir)
            }
            _ => dir,
        };
        // Never a shared root: measuring `C:\Program Files` would report every
        // installed application as the size of all of them.
        if is_shared_root(dir) {
            continue;
        }
        return Some(dir.to_path_buf());
    }
    None
}

/// Pull an executable path out of a registry string, which may be quoted and
/// may carry arguments or an icon index after it.
fn executable_path(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    let path = if let Some(rest) = raw.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        // `C:\dir\app.exe,0` or `C:\dir\unins.exe /SILENT`
        raw.split(',')
            .next()?
            .split(" /")
            .next()?
            .trim()
            .to_string()
    };
    // MSI entries are `msiexec /x {GUID}` and name no directory at all.
    if path.is_empty() || !path.contains('\\') {
        return None;
    }
    let path = PathBuf::from(path);
    path.is_file().then_some(path)
}

/// Directories that hold many applications rather than being one.
fn is_shared_root(dir: &Path) -> bool {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "PROGRAMDATA",
        "SystemRoot",
    ] {
        if let Some(v) = std::env::var_os(var) {
            roots.push(PathBuf::from(v));
        }
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.clone());
        roots.push(home.join("AppData\\Local"));
        roots.push(home.join("AppData\\Roaming"));
        roots.push(home.join("AppData\\Local\\Programs"));
    }
    roots.iter().any(|r| dir == r)
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
    let sources: Vec<(String, Vec<String>)> = apps
        .iter()
        .map(|a| (a.id.clone(), icon_sources(a)))
        .filter(|(_, srcs)| !srcs.is_empty())
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
    for (i, (_, srcs)) in sources.iter().enumerate() {
        let out = ps_quote(&dir.join(format!("{i}.png")).to_string_lossy());
        // Several candidates per app, tried in order and stopping at the first
        // that yields an image. Extraction fails often enough — a DisplayIcon
        // pointing at a file that has gone, a DLL with no icon resource — that
        // one source per app leaves obvious gaps in the list.
        let list = srcs
            .iter()
            .map(|s| ps_quote(s))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            "foreach ($p in @({list})) {{\n  if (Test-Path {out}) {{ break }}\n  try {{\n    if ($p.ToLower().EndsWith('.ico')) {{ $ic = New-Object System.Drawing.Icon($p) }} else {{ $ic = [System.Drawing.Icon]::ExtractAssociatedIcon($p) }}\n    if ($ic -ne $null) {{ $b = $ic.ToBitmap(); $b.Save({out}, [System.Drawing.Imaging.ImageFormat]::Png); $b.Dispose(); $ic.Dispose() }}\n  }} catch {{}}\n}}\n"
        );
        script.push_str(&body);
    }

    let script_path = dir.join("extract.ps1");
    if std::fs::write(&script_path, script).is_err() {
        return HashMap::new();
    }

    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
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

/// Everywhere this app's icon might be, best first.
///
/// The registry's `DisplayIcon` is the intended answer, but it is often absent
/// or points at a file that has since gone. The executables in the app's own
/// directory are offered after it, which is what fills in the entries that
/// would otherwise show a letter.
fn icon_sources(app: &InstalledApp) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let display_icon = registry_value(&app.id, "DisplayIcon");

    if let Some(raw) = &display_icon {
        // Often `C:\path\app.exe,0` — the index selects which icon inside the
        // file, and is not part of the path.
        let path = raw
            .rsplit_once(',')
            .map(|(p, _)| p)
            .unwrap_or(raw)
            .trim()
            .trim_matches('"')
            .to_string();
        if !path.is_empty() && Path::new(&path).is_file() {
            out.push(path);
        }
    }

    let dir = app
        .path
        .clone()
        .or_else(|| infer_install_dir(&display_icon, &registry_value(&app.id, "UninstallString")));
    let Some(dir) = dir else {
        return out;
    };

    // An executable named after the app first, then any other that is not
    // obviously an uninstaller or a background helper.
    let wanted = crate::leftovers::slug(&app.name);
    let mut others: Vec<String> = Vec::new();
    for entry in fsutil::children(&dir) {
        if entry.extension().and_then(|e| e.to_str()) != Some("exe") {
            continue;
        }
        let Some(stem) = entry.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let slug = crate::leftovers::slug(&stem);
        if slug.contains("uninst") || slug.contains("setup") || slug.contains("crashpad") {
            continue;
        }
        let path = entry.to_string_lossy().to_string();
        if !wanted.is_empty() && (wanted.contains(&slug) || slug.contains(&wanted)) {
            out.push(path);
        } else {
            others.push(path);
        }
    }
    out.extend(others.into_iter().take(2));
    out.dedup();
    out.truncate(4);
    out
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
