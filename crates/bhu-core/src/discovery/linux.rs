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

/// Run a command and keep whatever it printed, even if it reports failure.
///
/// ⚠️ `dpkg-query -S` exits non-zero when **any** path it was given is not
/// owned by a package — and the launcher directories are full of those: snap
/// puts its own there, so do flatpak and the user's own `~/.local/share`. So
/// the ordinary runner threw away a perfectly good answer for every other
/// path, the ownership map came back empty, and everything fell back to
/// guessing the package from the filename. That is the bug that kept Chrome
/// out of the list even after the filename matching was supposed to be gone.
fn output_partial(program: &str, args: &[&str]) -> Option<String> {
    let out = crate::proc::command(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    (!text.trim().is_empty()).then_some(text)
}

pub fn installed_apps(opts: ScanOptions) -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    apps.extend(dpkg());
    apps.extend(rpm());
    apps.extend(flatpak());
    apps.extend(snap());
    apps.extend(appimages(opts));

    // A package name is not what a person recognises, so where a launcher
    // exists its Name is preferred — and a package with no launcher at all is
    // not an application, it is part of the system.
    let launchers = launchers_by_package();
    for app in apps.iter_mut() {
        match launchers.get(&app.id) {
            Some(launcher) => app.name = launcher.name.clone(),
            // Everything that a package manager installed has to earn its
            // place by shipping a launcher. That covers snap's runtimes and
            // bases — `core24`, `bare`, `gnome-46-2404`, `gtk-common-themes` —
            // which are machinery other snaps run on top of and are no more an
            // application than `libc6` is. An AppImage is a file the user put
            // there deliberately, so it always counts.
            None => app.is_system = !matches!(app.source, AppSource::AppImage),
        }
    }
    if !opts.include_system {
        apps.retain(|a| !a.is_system);
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
            // dpkg has no install date, but it writes the package's file list
            // when it installs it, so that file's timestamp is one.
            let list = PathBuf::from("/var/lib/dpkg/info").join(format!("{package}.list"));
            let (created, modified) = fsutil::timestamps(&list);
            app.created_at = created
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            app.modified_at = modified
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
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
        &["list", "--app", "--columns=application,name,version,origin"],
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
            let origin = parts
                .next()
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty() && o != "-");
            let mut app = base(id.to_string(), name, version, AppSource::Flatpak);
            app.publisher = origin;
            // A flatpak id is reverse-DNS, which the leftover matcher can use
            // exactly as it uses a macOS bundle id.
            app.bundle_id = Some(id.to_string());
            Some(app)
        })
        .collect()
}

/// Installed snaps.
///
/// `snap list` prints `Name Version Rev Tracking Publisher Notes`. Only the
/// name and version were being read, which is why a snap showed a version and
/// nothing else at all — no publisher, no size, no dates, no location. The
/// revision is what makes the rest reachable: the downloaded package sits at
/// `/var/lib/snapd/snaps/<name>_<rev>.snap`, and its size and timestamp are
/// the snap's.
fn snap() -> Vec<InstalledApp> {
    let Some(text) = output("snap", &["list"]) else {
        return Vec::new();
    };
    parse_snap_list(&text)
        .into_iter()
        .map(|row| {
            let name = row.name;
            let mut app = base(name.clone(), name.clone(), row.version, AppSource::Snap);
            app.publisher = row.publisher;

            // The mounted revision is the location a person would recognise.
            let current = PathBuf::from("/snap").join(&name).join("current");
            if current.exists() {
                app.path = Some(current);
            }
            if let Some(rev) = row.rev {
                let pkg = PathBuf::from("/var/lib/snapd/snaps").join(format!("{name}_{rev}.snap"));
                if let Ok(meta) = std::fs::metadata(&pkg) {
                    app.size_bytes = meta.len();
                }
                let (created, modified) = fsutil::timestamps(&pkg);
                app.created_at = created
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                app.modified_at = modified
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
            }
            app
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
/// Parse `dpkg-query -S` output into path -> owning package.
///
/// Lines read `pkg: /path`, or `pkg1, pkg2: /path` when more than one package
/// ships the same file. Anything else — the "no path found" notices, blank
/// lines — is skipped, because this deliberately runs on partial output.
fn parse_dpkg_owners(text: &str) -> std::collections::HashMap<String, String> {
    let mut owner = std::collections::HashMap::new();
    for line in text.lines() {
        let Some((pkgs, path)) = line.rsplit_once(": ") else {
            continue;
        };
        let path = path.trim();
        if !path.starts_with('/') {
            continue;
        }
        if let Some(first) = pkgs.split(',').next() {
            let first = first.trim();
            if !first.is_empty() {
                owner.insert(path.to_string(), first.to_string());
            }
        }
    }
    owner
}

/// One row of `snap list`: `Name Version Rev Tracking Publisher Notes`.
struct SnapRow {
    name: String,
    version: Option<String>,
    rev: Option<String>,
    publisher: Option<String>,
}

/// Parse `snap list`.
///
/// The publisher carries a trailing `*` when the account is verified, and is
/// `-` for a sideloaded snap. ⚠️ A sideloaded revision is written `x1` and the
/// file on disk is named with that same `x1`, so the `x` must be kept.
fn parse_snap_list(text: &str) -> Vec<SnapRow> {
    text.lines()
        .skip(1) // header
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let name = parts.first()?.to_string();
            if name.is_empty() {
                return None;
            }
            Some(SnapRow {
                name,
                version: parts.get(1).map(|v| v.to_string()),
                rev: parts.get(2).map(|r| r.to_string()),
                publisher: parts
                    .get(4)
                    .map(|p| p.trim_end_matches('*').to_string())
                    .filter(|p| !p.is_empty() && p != "-"),
            })
        })
        .collect()
}

/// One `.desktop` launcher.
#[derive(Clone)]
struct Launcher {
    name: String,
    icon: Option<String>,
}

/// Every application launcher on the system, keyed by the **package that owns
/// it** rather than by its filename.
///
/// The filename is not the package name and assuming it is loses real
/// applications: Google Chrome installs as `google-chrome-stable` and ships
/// `google-chrome.desktop`, so a filename match finds nothing and Chrome never
/// appears at all. `dpkg -S` answers the question properly, in one call for
/// every launcher at once.
///
/// This map is also what separates an application from a package. A Linux
/// system has upwards of 1700 packages installed and almost none of them are
/// things a person would say they have "installed" — `acl`, `adduser`,
/// `libc6`. A package that ships no launcher is marked as a system package and
/// hidden, exactly as the macOS and Windows adapters already do.
fn launchers_by_package() -> std::collections::HashMap<String, Launcher> {
    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    // Read every launcher first, keyed by its path.
    let mut found: Vec<(PathBuf, String, Launcher)> = Vec::new();
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
            let field = |key: &str| {
                text.lines()
                    .find_map(|l| l.strip_prefix(key))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            };
            // `NoDisplay=true` is how a launcher says it is plumbing rather
            // than an application — file-type handlers, settings panels, and
            // the like. The desktop hides them and so do we.
            if field("NoDisplay=").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                continue;
            }
            let Some(name) = field("Name=") else { continue };
            found.push((
                path,
                stem,
                Launcher {
                    name,
                    icon: field("Icon="),
                },
            ));
        }
    }

    // Ask dpkg who owns them, all in one call.
    let paths: Vec<String> = found
        .iter()
        .map(|(p, _, _)| p.to_string_lossy().to_string())
        .collect();
    let mut owner: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !paths.is_empty() {
        let args: Vec<&str> = std::iter::once("-S")
            .chain(paths.iter().map(String::as_str))
            .collect();
        if let Some(text) = output_partial("dpkg-query", &args) {
            owner = parse_dpkg_owners(&text);
        }
    }

    let mut map = std::collections::HashMap::new();
    for (path, stem, launcher) in found {
        // The owning package where dpkg could tell us, and the filename
        // otherwise — which is right for flatpak, snap and rpm systems, where
        // the id already is the launcher's name.
        let owned = owner.get(&path.to_string_lossy().to_string()).cloned();
        // snap names its launchers `<snap>_<app>.desktop`, so the snap itself
        // is the part before the underscore. Recorded as well as the full
        // stem, since either may be what the caller knows it by.
        if owned.is_none() {
            if let Some((snap_name, _)) = stem.split_once('_') {
                map.entry(snap_name.to_string()).or_insert(Launcher {
                    name: launcher.name.clone(),
                    icon: launcher.icon.clone(),
                });
            }
        }
        map.entry(owned.unwrap_or(stem)).or_insert(launcher);
    }
    map
}

/// The same map, built once per run.
///
/// Icons are fetched one application at a time and in parallel, so calling the
/// builder per row would mean a `dpkg -S` for every row and would dominate the
/// scan. The list itself reads fresh in `installed_apps`, so a refresh still
/// reflects what is installed now; a stale icon for something already removed
/// costs nothing, because it is no longer in the list to draw.
fn cached_launchers() -> &'static std::collections::HashMap<String, Launcher> {
    static CACHE: std::sync::OnceLock<std::collections::HashMap<String, Launcher>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(launchers_by_package)
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
    // The owning package again, for the same reason as the name: Chrome's
    // launcher is not called after its package.
    let name = cached_launchers()
        .get(&app.id)
        .and_then(|l| l.icon.clone())
        .or_else(|| desktop_icon_name(&app.id))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `dpkg-query -S` output, including the notice it prints for paths
    /// nothing owns — which is why it exits non-zero and why this has to work
    /// on a partial answer.
    #[test]
    fn dpkg_ownership_survives_unowned_paths() {
        let text = "\
google-chrome-stable: /usr/share/applications/google-chrome.desktop
firefox, firefox-esr: /usr/share/applications/firefox.desktop
dpkg-query: no path found matching pattern /var/lib/snapd/desktop/applications/firefox_firefox.desktop
gimp: /usr/share/applications/gimp.desktop
";
        let owners = parse_dpkg_owners(text);
        // The one that mattered: the package is not what the launcher is called.
        assert_eq!(
            owners.get("/usr/share/applications/google-chrome.desktop"),
            Some(&"google-chrome-stable".to_string())
        );
        // Several packages ship it; the first is enough to identify the app.
        assert_eq!(
            owners.get("/usr/share/applications/firefox.desktop"),
            Some(&"firefox".to_string())
        );
        assert_eq!(owners.len(), 3, "the notice line must not become an entry");
    }

    #[test]
    fn snap_rows_are_read_across_all_their_columns() {
        let text = "\
Name               Version   Rev    Tracking       Publisher   Notes
bare               1.0       5      latest/stable  canonical*  base
core24             20250801  1140   latest/stable  canonical*  base
firefox            149.0.2-1 6836   latest/stable  mozilla*    -
my-local-build     0.1       x1     -              -           -
";
        let rows = parse_snap_list(text);
        assert_eq!(rows.len(), 4, "the header must not be read as a snap");

        let firefox = rows.iter().find(|r| r.name == "firefox").unwrap();
        assert_eq!(firefox.version.as_deref(), Some("149.0.2-1"));
        assert_eq!(firefox.rev.as_deref(), Some("6836"));
        // The verified-publisher asterisk is not part of the name.
        assert_eq!(firefox.publisher.as_deref(), Some("mozilla"));

        // A sideloaded snap has no publisher, and its revision keeps the `x`
        // because the file on disk is named with it.
        let local = rows.iter().find(|r| r.name == "my-local-build").unwrap();
        assert_eq!(local.publisher, None);
        assert_eq!(local.rev.as_deref(), Some("x1"));
    }
}
