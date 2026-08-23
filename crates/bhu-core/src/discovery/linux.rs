//! Linux app discovery.
//!
//! Compile-verified for the Linux target, not yet run on Linux.
//!
//! Everything here is delegated: a package manager owns the install, so it owns
//! the uninstall too. This module only reports what is there.

use crate::discovery::ScanOptions;
use crate::fsutil;
use crate::model::*;
use std::path::{Path, PathBuf};

/// Run a command, returning its stdout when it exists and succeeds.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = crate::proc::command(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn installed_apps(opts: ScanOptions) -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    apps.extend(dpkg());
    apps.extend(rpm());
    apps.extend(flatpak());
    apps.extend(snap());
    apps.extend(appimages(opts));

    // A package name is not what a person recognises, so where a .desktop entry
    // exists its Name is preferred.
    let names = desktop_names();
    for app in apps.iter_mut() {
        if let Some(pretty) = names.get(&app.id) {
            app.name = pretty.clone();
        }
    }
    apps
}

fn base(id: String, name: String, version: Option<String>, source: AppSource) -> InstalledApp {
    InstalledApp {
        id,
        name,
        path: None,
        bundle_id: None,
        executable: None,
        version,
        publisher: None,
        size_bytes: 0,
        source,
        icon_png_base64: None,
        created_at: None,
        modified_at: None,
        last_opened_at: None,
        notarized: None,
        is_running: false,
        is_system: false,
        scope: None,
    }
}

fn dpkg() -> Vec<InstalledApp> {
    let Some(text) = output(
        "dpkg-query",
        &[
            "-W",
            "-f=${Package}\t${Version}\t${Maintainer}\t${Installed-Size}\n",
        ],
    ) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let package = parts.next()?.trim();
            if package.is_empty() {
                return None;
            }
            let version = parts.next().map(str::to_string).filter(|v| !v.is_empty());
            let maintainer = parts.next().map(str::to_string);
            // dpkg reports installed size in kibibytes.
            let size = parts
                .next()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0)
                * 1024;

            let mut app = base(
                package.to_string(),
                package.to_string(),
                version,
                AppSource::Dpkg,
            );
            app.publisher = maintainer;
            app.size_bytes = size;
            Some(app)
        })
        .collect()
}

fn rpm() -> Vec<InstalledApp> {
    let Some(text) = output(
        "rpm",
        &["-qa", "--qf", "%{NAME}\t%{VERSION}\t%{VENDOR}\t%{SIZE}\n"],
    ) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let package = parts.next()?.trim();
            if package.is_empty() {
                return None;
            }
            let version = parts.next().map(str::to_string);
            let vendor = parts.next().map(str::to_string);
            let size = parts
                .next()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let mut app = base(
                package.to_string(),
                package.to_string(),
                version,
                AppSource::Rpm,
            );
            app.publisher = vendor;
            app.size_bytes = size;
            Some(app)
        })
        .collect()
}

fn flatpak() -> Vec<InstalledApp> {
    let Some(text) = output(
        "flatpak",
        &["list", "--app", "--columns=application,name,version"],
    ) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let id = parts.next()?.trim();
            if id.is_empty() {
                return None;
            }
            let name = parts.next().unwrap_or(id).trim().to_string();
            let version = parts
                .next()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let mut app = base(id.to_string(), name, version, AppSource::Flatpak);
            // A flatpak id is reverse-DNS, which the leftover matcher can use
            // exactly as it uses a macOS bundle id.
            app.bundle_id = Some(id.to_string());
            Some(app)
        })
        .collect()
}

fn snap() -> Vec<InstalledApp> {
    let Some(text) = output("snap", &["list"]) else {
        return Vec::new();
    };
    text.lines()
        .skip(1) // header
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let version = parts.next().map(str::to_string);
            Some(base(
                name.to_string(),
                name.to_string(),
                version,
                AppSource::Snap,
            ))
        })
        .collect()
}

/// AppImages are single files with no manager behind them — the one Linux case
/// this app removes itself.
fn appimages(opts: ScanOptions) -> Vec<InstalledApp> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in ["Applications", ".local/bin", "Downloads"] {
        for path in fsutil::children(&home.join(dir)) {
            let is_appimage = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("appimage"))
                .unwrap_or(false);
            if !is_appimage {
                continue;
            }
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut app = base(
                path.to_string_lossy().to_string(),
                name,
                None,
                AppSource::AppImage,
            );
            app.size_bytes = if opts.compute_sizes {
                fsutil::size_on_disk(&path)
            } else {
                0
            };
            app.path = Some(path);
            out.push(app);
        }
    }
    out
}

/// Package name -> the name a person would recognise, from `.desktop` entries.
fn desktop_names() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    for dir in dirs {
        for path in fsutil::children(&dir) {
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(name) = text
                .lines()
                .find_map(|l| l.strip_prefix("Name="))
                .map(str::to_string)
            {
                map.insert(stem, name);
            }
        }
    }
    map
}

/// The command that removes this package. Shown to the user; never run without
/// them seeing it, since it needs root and is the package manager's business.
pub fn uninstall_command(app: &InstalledApp) -> Option<String> {
    Some(match app.source {
        AppSource::Dpkg => format!("sudo apt-get remove --purge {}", app.id),
        AppSource::Rpm => format!("sudo dnf remove {}", app.id),
        AppSource::Pacman => format!("sudo pacman -Rns {}", app.id),
        AppSource::Flatpak => format!("flatpak uninstall {}", app.id),
        AppSource::Snap => format!("sudo snap remove {}", app.id),
        _ => return None,
    })
}

pub fn enrich(_app: &mut InstalledApp) {}

/// The app's icon, found through its `.desktop` entry.
///
/// No subprocess and no image decoding: the icon named there is looked up in
/// the standard theme directories and, if a PNG is found, its bytes are used
/// as they are. SVG-only icons are skipped — rasterising them would mean
/// pulling in a renderer for a 38-pixel row.
pub fn icon(app: &InstalledApp) -> Option<String> {
    let name = desktop_icon_name(&app.id)?;

    // An absolute path in Icon= is used directly.
    let direct = PathBuf::from(&name);
    if direct.is_absolute() {
        return read_png(&direct);
    }

    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/icons/hicolor"),
        PathBuf::from("/usr/local/share/icons/hicolor"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons/hicolor"),
    ];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/icons/hicolor"));
        roots.push(home.join(".icons/hicolor"));
    }

    // Biggest first: the list shows them at 38 points, and downscaling a large
    // icon looks far better than upscaling a 16-pixel one.
    for size in [
        "256x256", "192x192", "128x128", "96x96", "64x64", "48x48", "scalable",
    ] {
        for root in &roots {
            let candidate = root.join(size).join("apps").join(format!("{name}.png"));
            if let Some(png) = read_png(&candidate) {
                return Some(png);
            }
        }
    }

    // The old flat location, still used by plenty of packages.
    for dir in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
        if let Some(png) = read_png(&PathBuf::from(dir).join(format!("{name}.png"))) {
            return Some(png);
        }
    }
    None
}

pub fn icons(apps: &[InstalledApp]) -> std::collections::HashMap<String, String> {
    super::icons_in_parallel(apps, icon)
}

/// Read a PNG, rejecting anything that is not one — the extension alone is not
/// enough to trust bytes we are about to hand to a webview as an image.
fn read_png(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    // Icons are small; anything enormous is not one.
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    Some(crate::base64_encode(&bytes))
}

/// The `Icon=` value from this app's `.desktop` entry.
fn desktop_icon_name(id: &str) -> Option<String> {
    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    for dir in dirs {
        for candidate in [format!("{id}.desktop"), format!("{id}_{id}.desktop")] {
            let path = dir.join(&candidate);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(icon) = text
                .lines()
                .find_map(|l| l.strip_prefix("Icon="))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                return Some(icon);
            }
        }
    }
    None
}
