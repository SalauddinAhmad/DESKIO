//! Windows junk.
//!
//! Compile-verified for the Windows target, not yet run on Windows.
//!
//! The Recycle Bin is reported only, exactly as the Trash is on macOS: emptying
//! it is the one irreversible act, and it stays with the user.

use super::{JunkCategory, JunkItem};
use crate::fsutil;
use crate::safety;
use std::path::{Path, PathBuf};

pub fn scan() -> Vec<JunkItem> {
    let mut out = Vec::new();
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return out;
    };

    // The user's temp directory: the single largest source of reclaimable junk
    // on most Windows machines.
    for path in fsutil::children(&local.join("Temp")) {
        if safety::check_removable(&path).is_ok() {
            out.push(item(path, JunkCategory::AppCaches, "Temporary file"));
        }
    }

    // Per-vendor caches, one level in: %LOCALAPPDATA%\<vendor>\<product>\Cache.
    for vendor in fsutil::children(&local) {
        if !vendor.is_dir() {
            continue;
        }
        for candidate in ["Cache", "cache", "Code Cache", "GPUCache"] {
            let path = vendor.join(candidate);
            if path.is_dir() && safety::check_removable(&path).is_ok() {
                out.push(item(path, JunkCategory::AppCaches, "Cache"));
            }
        }
    }

    for path in fsutil::children(&local.join("CrashDumps")) {
        if safety::check_removable(&path).is_ok() {
            out.push(item(path, JunkCategory::CrashReports, "Crash dump"));
        }
    }

    if let Some(home) = dirs::home_dir() {
        for (sub, detail) in [
            (r".nuget\packages", "NuGet package cache"),
            (r".gradle\caches", "Gradle cache"),
            (r".m2\repository", "Maven repository"),
            (r".cargo\registry\cache", "Cargo registry cache"),
        ] {
            let path = home.join(sub);
            if path.is_dir() && safety::check_removable(&path).is_ok() {
                out.push(item(path, JunkCategory::DeveloperJunk, detail));
            }
        }
    }

    out.retain(|i| i.size_bytes > 0);
    out
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

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}
