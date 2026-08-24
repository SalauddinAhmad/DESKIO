//! Moving things to the trash.
//!
//! The engine never deletes. Every removal is a move to the OS trash / recycle
//! bin, so that a wrong call by us, or a change of mind by the user, is always
//! recoverable.
//!
//! ## The two routes on macOS, and why the choice is not only cosmetic
//!
//! macOS offers two ways to trash a file:
//!
//! - **Through Finder.** Finder plays its trash sound for every item, and needs
//!   Automation permission. It records the information behind Finder's
//!   "Put Back".
//! - **Through `NSFileManager`.** Silent, faster, no extra permission — but it
//!   records nothing, so "Put Back" is greyed out. Verified on macOS 27: a file
//!   trashed this way has no `kMDItemWhereFroms` and no put-back attribute.
//!
//! So the sound setting is really a choice between Finder's restore affordance
//! and a quiet, permission-free removal. The engine covers the gap by recording
//! where each item came from *and* where it landed in the trash, so it can put
//! things back itself either way — see [`crate::undo`].

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TrashError {
    #[error("{0}")]
    Failed(String),
}

/// Move a path to the trash, returning where it ended up when that can be
/// determined. Never follows symlinks: a symlink is trashed as the link itself.
pub fn move_to_trash(path: &Path, sound: bool) -> Result<Option<PathBuf>, TrashError> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::TrashContextExtMacos;
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(if sound {
            trash::macos::DeleteMethod::Finder
        } else {
            trash::macos::DeleteMethod::NsFileManager
        });
        ctx.delete(path)
            .map_err(|e| TrashError::Failed(e.to_string()))?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = sound;
        trash::delete(path).map_err(|e| TrashError::Failed(e.to_string()))?;
    }
    Ok(locate_in_trash(path))
}

/// Work out where an item landed, so it can be put back later.
///
/// The trash APIs do not report the destination, and the name may have been
/// changed to avoid a collision with something already in there. We look for
/// the exact name first, then for the newest entry whose name is that one with
/// a suffix, which is the pattern macOS uses.
#[cfg(target_os = "macos")]
fn locate_in_trash(original: &Path) -> Option<PathBuf> {
    let trash = dirs::home_dir()?.join(".Trash");
    let name = original.file_name()?.to_string_lossy().to_string();

    let exact = trash.join(&name);
    if exact.exists() {
        return Some(exact);
    }

    let stem = Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.clone());

    std::fs::read_dir(&trash)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&stem))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

#[cfg(not(target_os = "macos"))]
fn locate_in_trash(_original: &Path) -> Option<PathBuf> {
    // The freedesktop and Windows implementations record their own restore
    // information, so the destination is not needed to put an item back.
    None
}

/// Whether this platform can restore from the trash through the file manager
/// the user already knows.
///
/// On macOS this depends on how the item was trashed — see the module docs —
/// so the UI must not promise "Put Back" unconditionally.
pub const fn can_restore_programmatically() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}
