//! Where macOS accumulates reclaimable junk.

use super::{JunkCategory, JunkItem};
use crate::fsutil;
use crate::safety;
use std::path::{Path, PathBuf};

pub fn scan() -> Vec<JunkItem> {
    let mut out = Vec::new();
    out.extend(app_caches());
    out.extend(logs());
    out.extend(crash_reports());
    out.extend(developer_junk());
    out.extend(trash());
    out.retain(|i| i.size_bytes > 0 || i.category == JunkCategory::Trash);
    out
}

/// Caches belonging to installed apps.
///
/// Apple's own caches are deliberately left out. Clearing them is usually
/// harmless and occasionally is not, and the difference is not something this
/// app can tell the user in advance — so it does not offer them at all.
fn app_caches() -> Vec<JunkItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let root = home.join("Library/Caches");

    fsutil::children(&root)
        .into_iter()
        .filter(|p| {
            let name = file_name(p);
            !name.starts_with("com.apple.") && !name.starts_with('.')
        })
        .filter(|p| safety::check_removable(p).is_ok())
        .map(|path| item(path, JunkCategory::AppCaches, "Cache"))
        .collect()
}

fn logs() -> Vec<JunkItem> {
    let mut roots: Vec<PathBuf> = vec![PathBuf::from("/Library/Logs")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Library/Logs"));
    }
    roots
        .iter()
        .flat_map(|root| fsutil::children(root))
        .filter(|p| {
            let name = file_name(p);
            // DiagnosticReports is reported as crash reports, not as logs.
            name != "DiagnosticReports" && !name.starts_with('.')
        })
        .filter(|p| safety::check_removable(p).is_ok())
        .map(|path| item(path, JunkCategory::Logs, "Log"))
        .collect()
}

fn crash_reports() -> Vec<JunkItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let roots = [
        home.join("Library/Logs/DiagnosticReports"),
        home.join("Library/Application Support/CrashReporter"),
        PathBuf::from("/Library/Logs/DiagnosticReports"),
    ];
    roots
        .iter()
        .flat_map(|root| fsutil::children(root))
        .filter(|p| !file_name(p).starts_with('.'))
        .filter(|p| safety::check_removable(p).is_ok())
        .map(|path| item(path, JunkCategory::CrashReports, "Crash report"))
        .collect()
}

/// Xcode's build output and device support. Usually the largest thing here by
/// an order of magnitude, and entirely regenerable.
fn developer_junk() -> Vec<JunkItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let dev = home.join("Library/Developer");
    if !dev.exists() {
        return Vec::new();
    }

    let mut out = Vec::new();

    // DerivedData holds one folder per project; listing them individually lets
    // someone keep the project they are working on.
    for path in fsutil::children(&dev.join("Xcode/DerivedData")) {
        if safety::check_removable(&path).is_ok() {
            out.push(item(
                path,
                JunkCategory::DeveloperJunk,
                "Xcode build output",
            ));
        }
    }

    for (sub, detail) in [
        ("Xcode/Archives", "Xcode archive"),
        ("Xcode/iOS DeviceSupport", "iOS device support"),
        ("Xcode/watchOS DeviceSupport", "watchOS device support"),
        ("Xcode/tvOS DeviceSupport", "tvOS device support"),
        ("CoreSimulator/Caches", "Simulator cache"),
        ("Xcode/UserData/Previews", "SwiftUI preview cache"),
    ] {
        for path in fsutil::children(&dev.join(sub)) {
            if safety::check_removable(&path).is_ok() {
                out.push(item(path, JunkCategory::DeveloperJunk, detail));
            }
        }
    }

    // Xcode's own cache is Apple-prefixed, so the general rule above skips it —
    // but it is unambiguously build junk, so it is included here by name.
    let xcode_cache = home.join("Library/Caches/com.apple.dt.Xcode");
    if xcode_cache.exists() && safety::check_removable(&xcode_cache).is_ok() {
        out.push(item(
            xcode_cache,
            JunkCategory::DeveloperJunk,
            "Xcode cache",
        ));
    }

    out
}

/// What is already in the Trash. Reported so the user can see the space that is
/// waiting to be reclaimed — never acted on.
fn trash() -> Vec<JunkItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    fsutil::children(&home.join(".Trash"))
        .into_iter()
        .filter(|p| !file_name(p).starts_with('.'))
        .map(|path| item(path, JunkCategory::Trash, "Waiting in the Trash"))
        .collect()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn item(path: PathBuf, category: JunkCategory, detail: &str) -> JunkItem {
    let (size, size_unknown) = fsutil::size_with_confidence(&path);
    JunkItem {
        id: path.to_string_lossy().to_string(),
        name: file_name(&path),
        category,
        size_bytes: size,
        size_unknown,
        is_directory: path.is_dir(),
        requires_admin: safety::requires_admin(&path),
        detail: Some(detail.to_string()),
        path,
    }
}
