//! macOS app discovery.
//!
//! An "installed app" on macOS is just a `.app` bundle sitting in a known
//! location — there is no package database to consult, which is exactly why
//! leftovers are such a problem on this platform.

use crate::discovery::ScanOptions;
use crate::fsutil;
use crate::model::*;
use std::path::{Path, PathBuf};

/// Places a user's own applications live. `/System/Applications` is
/// deliberately absent: those are OS apps and are not removable.
fn scan_roots() -> Vec<(PathBuf, AppSource)> {
    let mut roots = vec![
        (PathBuf::from("/Applications"), AppSource::Applications),
        (
            PathBuf::from("/Applications/Utilities"),
            AppSource::Applications,
        ),
        (PathBuf::from("/Applications/Setapp"), AppSource::Setapp),
    ];
    if let Some(home) = dirs::home_dir() {
        roots.push((home.join("Applications"), AppSource::UserApplications));
    }
    roots
}

pub fn installed_apps(opts: ScanOptions) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    for (root, source) in scan_roots() {
        for path in fsutil::children(&root) {
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            if seen.contains(&path) {
                continue;
            }
            seen.push(path.clone());
            if let Some(app) = read_bundle(&path, source, opts) {
                if app.is_system && !opts.include_system {
                    continue;
                }
                apps.push(app);
            }
        }
    }
    apps
}

/// Read an `.app` bundle's `Info.plist` into an `InstalledApp`.
fn read_bundle(path: &Path, source: AppSource, opts: ScanOptions) -> Option<InstalledApp> {
    let info = path.join("Contents/Info.plist");
    let dict = plist::Value::from_file(&info)
        .ok()
        .and_then(|v| v.into_dictionary());

    let get = |key: &str| -> Option<String> {
        dict.as_ref()
            .and_then(|d| d.get(key))
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
    };

    let bundle_id = get("CFBundleIdentifier");

    // Display name preference matches what Finder shows.
    let name = get("CFBundleDisplayName")
        .or_else(|| get("CFBundleName"))
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });

    let version = get("CFBundleShortVersionString").or_else(|| get("CFBundleVersion"));

    let (created, modified) = fsutil::timestamps(path);

    // Apple's own bundle ids, and anything under a system path, are not ours to
    // remove. `/Applications` does contain a few Apple apps (Safari, Xcode).
    let is_system = bundle_id
        .as_deref()
        .map(|b| b.starts_with("com.apple."))
        .unwrap_or(false);

    Some(InstalledApp {
        id: bundle_id
            .clone()
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        name,
        path: Some(path.to_path_buf()),
        executable: get("CFBundleExecutable"),
        bundle_id,
        version,
        publisher: None,
        size_bytes: if opts.compute_sizes {
            fsutil::size_on_disk(path)
        } else {
            0
        },
        source: detect_source(path, source),
        icon_png_base64: None,
        created_at: unix_secs(created),
        modified_at: unix_secs(modified),
        last_opened_at: None,
        notarized: None,
        is_running: false,
        is_system,
        scope: None,
    })
}

/// Mac App Store apps carry a receipt inside the bundle. Knowing this matters
/// because their containers are named differently and they can be reinstalled
/// for free, which is worth telling the user before they remove one.
fn detect_source(path: &Path, fallback: AppSource) -> AppSource {
    if path.join("Contents/_MASReceipt/receipt").exists() {
        return AppSource::MacAppStore;
    }
    fallback
}

pub fn enrich(app: &mut InstalledApp) {
    let Some(path) = app.path.clone() else { return };

    if app.size_bytes == 0 {
        app.size_bytes = fsutil::size_on_disk(&path);
    }
    app.publisher = signing_authority(&path);
    app.notarized = Some(is_notarized(&path));
    app.last_opened_at = last_used(&path);
    app.is_running = is_running(&path);
    app.icon_png_base64 = icon_png(&path);
}

pub fn icon(app: &InstalledApp) -> Option<String> {
    icon_png(app.path.as_ref()?)
}

/// One `sips` per app, spread across threads — doing several hundred in
/// sequence left the list showing placeholders for the better part of a minute.
pub fn icons(apps: &[InstalledApp]) -> std::collections::HashMap<String, String> {
    super::icons_in_parallel(apps, icon)
}

/// The Developer ID the app is signed with — the "Developer" row in the UI.
/// Also the most reliable vendor token we have for leftover matching, since it
/// is what the vendor actually calls themselves.
fn signing_authority(path: &Path) -> Option<String> {
    let out = crate::proc::command("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(path)
        .output()
        .ok()?;
    // codesign writes its report to stderr.
    let text = String::from_utf8_lossy(&out.stderr);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Authority=") {
            // "Developer ID Application: Tatyana Livinskaya (ABCD123456)"
            let name = rest.split_once(": ").map(|(_, n)| n).unwrap_or(rest).trim();
            // Strip the trailing team id in parentheses.
            let name = name.split(" (").next().unwrap_or(name).trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn is_notarized(path: &Path) -> bool {
    let Ok(out) = crate::proc::command("/usr/sbin/spctl")
        .args(["-a", "-vv"])
        .arg(path)
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&out.stderr).contains("source=Notarized")
}

/// Last-opened comes from Spotlight's metadata, which is where the OS actually
/// records it. Returns `None` when Spotlight has no entry (common for apps that
/// have never been launched, and on volumes with indexing disabled).
fn last_used(path: &Path) -> Option<i64> {
    let out = crate::proc::command("/usr/bin/mdls")
        .args(["-raw", "-name", "kMDItemLastUsedDate"])
        .arg(path)
        .output()
        .ok()?;
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() || raw == "(null)" {
        return None;
    }
    // "2026-08-18 09:34:12 +0000"
    parse_mdls_date(&raw)
}

fn parse_mdls_date(raw: &str) -> Option<i64> {
    let out = crate::proc::command("/bin/date")
        .args(["-j", "-f", "%Y-%m-%d %H:%M:%S %z", raw, "+%s"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<i64>()
        .ok()
}

/// True when a process is running from inside this bundle. Removing a running
/// app leaves the process alive with its files gone, so the UI must offer to
/// quit it first.
fn is_running(path: &Path) -> bool {
    let Ok(out) = crate::proc::command("/bin/ps")
        .args(["-A", "-o", "comm="])
        .output()
    else {
        return false;
    };
    let prefix = path.to_string_lossy().to_string();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.starts_with(&prefix))
}

/// The app's icon, converted to PNG and base64-encoded for the UI.
fn icon_png(path: &Path) -> Option<String> {
    let icns = find_icns(path)?;
    // Unique per call: icon extraction runs on several threads at once, and a
    // shared filename would have them overwriting each other's output.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    // A per-process directory, so nothing can pre-place a symlink at a path
    // `sips` is about to write to.
    let dir = std::env::temp_dir().join(format!("bhu-icons-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let tmp = dir.join(format!(
        "icon-{}.png",
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let ok = crate::proc::command("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z", "256"])
        .arg(&icns)
        .arg("--out")
        .arg(&tmp)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    let bytes = std::fs::read(&tmp).ok()?;
    let _ = std::fs::remove_file(&tmp);
    Some(crate::base64_encode(&bytes))
}

fn find_icns(bundle: &Path) -> Option<PathBuf> {
    let info = bundle.join("Contents/Info.plist");
    let named = plist::Value::from_file(&info)
        .ok()
        .and_then(|v| v.into_dictionary())
        .and_then(|d| {
            d.get("CFBundleIconFile")
                .and_then(|v| v.as_string())
                .map(str::to_string)
        });

    let resources = bundle.join("Contents/Resources");
    if let Some(mut n) = named {
        if !n.ends_with(".icns") {
            n.push_str(".icns");
        }
        let p = resources.join(&n);
        if p.exists() {
            return Some(p);
        }
    }
    // Fall back to the first .icns in Resources.
    fsutil::children(&resources)
        .into_iter()
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("icns"))
}
