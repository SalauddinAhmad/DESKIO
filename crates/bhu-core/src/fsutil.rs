//! Filesystem helpers shared by every scanner.

use std::fs;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Total size of a file or directory tree, in bytes.
///
/// Symlinks are never followed — an app's Application Support folder linking to
/// a huge directory elsewhere must not be reported as owning that size, and
/// must certainly not be walked into.
pub fn size_on_disk(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Size of a path, plus whether we were actually able to measure it.
///
/// A directory we cannot open reports `(0, true)` rather than a confident zero.
pub fn size_with_confidence(path: &Path) -> (u64, bool) {
    if is_unreadable_dir(path) {
        return (0, true);
    }
    (size_on_disk(path), false)
}

/// Timestamps for a path: (created, modified).
pub fn timestamps(path: &Path) -> (Option<SystemTime>, Option<SystemTime>) {
    match fs::metadata(path) {
        Ok(m) => (m.created().ok(), m.modified().ok()),
        Err(_) => (None, None),
    }
}

/// List the immediate children of a directory. Returns an empty vec rather than
/// an error when the directory is missing or unreadable — a scanner should skip
/// a root it cannot see, not abort the whole scan. (Unreadable roots are normal
/// before Full Disk Access is granted.)
pub fn children(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.filter_map(Result::ok).map(|e| e.path()).collect()
}

/// True when the directory exists and cannot be read — the signature of a
/// missing Full Disk Access grant on macOS.
pub fn is_unreadable_dir(dir: &Path) -> bool {
    dir.is_dir() && fs::read_dir(dir).is_err()
}

/// True when this path is a symlink.
///
/// Used to stop a directory walk following a link out of the tree it is
/// supposed to be scanning — `/Applications` can contain links to anywhere.
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}
