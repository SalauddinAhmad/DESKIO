//! Linux junk.
//!
//! Compile-verified for the Linux target, not yet run on Linux.
//!
//! `/var/log` is deliberately absent: it is system-owned and journald-managed,
//! and the safety layer refuses it anyway.

use super::{JunkCategory, JunkItem};
use crate::fsutil;
use crate::safety;
use std::path::{Path, PathBuf};

pub fn scan() -> Vec<JunkItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();

    for path in fsutil::children(&home.join(".cache")) {
        if safety::check_removable(&path).is_ok() {
            out.push(item(path, JunkCategory::AppCaches, "Cache"));
        }
    }

    for path in fsutil::children(&home.join(".local/state")) {
        if safety::check_removable(&path).is_ok() {
            out.push(item(path, JunkCategory::Logs, "Application state and logs"));
        }
    }

    // Package caches for language toolchains: all re-downloadable.
    for (sub, detail) in [
        (".cargo/registry/cache", "Cargo registry cache"),
        (".npm/_cacache", "npm cache"),
        (".gradle/caches", "Gradle cache"),
        (".m2/repository", "Maven repository"),
        (".cache/go-build", "Go build cache"),
        (".cache/pip", "pip cache"),
    ] {
        let path = home.join(sub);
        if path.is_dir() && safety::check_removable(&path).is_ok() {
            out.push(item(path, JunkCategory::DeveloperJunk, detail));
        }
    }

    // The freedesktop trash, reported only — never emptied.
    for path in fsutil::children(&home.join(".local/share/Trash/files")) {
        out.push(item(path, JunkCategory::Trash, "Waiting in the Trash"));
    }

    out.retain(|i| i.size_bytes > 0 || i.category == JunkCategory::Trash);
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
