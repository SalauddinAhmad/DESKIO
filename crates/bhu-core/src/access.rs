//! What this app is currently allowed to see.
//!
//! macOS keeps parts of a user's Library behind Full Disk Access, and an app
//! cannot ask for that permission programmatically — it can only point at
//! System Settings. What it *can* do is be specific about the cost of not
//! having it, which is what this module is for: it probes the places a scan
//! needs and reports exactly which ones came back empty because permission was
//! refused rather than because there was nothing there.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedLocation {
    pub path: PathBuf,
    /// What the user loses by this being unreadable, in their terms.
    pub consequence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessReport {
    /// True when the operating system is not withholding anything.
    pub granted: bool,
    /// How many locations were probed.
    pub checked: usize,
    pub blocked: Vec<BlockedLocation>,
    /// False on platforms where none of this applies.
    pub applicable: bool,
}

impl AccessReport {
    pub fn fully_granted(&self) -> bool {
        self.granted && self.blocked.is_empty()
    }
}

/// Probe the places a scan depends on.
#[cfg(target_os = "macos")]
pub fn report() -> AccessReport {
    let Some(home) = dirs::home_dir() else {
        return AccessReport {
            granted: false,
            checked: 0,
            blocked: Vec::new(),
            applicable: true,
        };
    };
    let lib = home.join("Library");

    // Each probe is a real directory a scan reads, paired with what goes wrong
    // when it cannot be read. Vague warnings do not help anyone decide whether
    // a permission this broad is worth granting.
    let probes: Vec<(PathBuf, &str)> = vec![
        (
            lib.join("Containers"),
            "Sandboxed apps keep all of their data here. Uninstalling one of them would \
             leave its container behind.",
        ),
        (
            lib.join("Application Support/Google/Chrome"),
            "Chrome's profile — usually several gigabytes. Its size reads as unknown and \
             it is never offered for removal.",
        ),
        (
            lib.join("Application Support/Firefox"),
            "Firefox's profile, including its extensions.",
        ),
        (
            lib.join("Safari"),
            "Safari's data, including which extensions are installed.",
        ),
        (
            lib.join("Application Support/MobileSync"),
            "iPhone and iPad backups, which are often the largest single thing on a Mac.",
        ),
        (
            lib.join("Mail"),
            "Mail's storage, so mail data left by a removed app is invisible.",
        ),
    ];

    let checked = probes.len();
    let blocked: Vec<BlockedLocation> = probes
        .into_iter()
        // A location that does not exist is not blocked — there is simply
        // nothing there, and reporting it would be scaremongering.
        .filter(|(path, _)| path.exists() && std::fs::read_dir(path).is_err())
        .map(|(path, consequence)| BlockedLocation {
            path,
            consequence: consequence.to_string(),
        })
        .collect();

    AccessReport {
        granted: is_granted(),
        checked,
        blocked,
        applicable: true,
    }
}

/// The canonical test: can this process read the system permissions database?
///
/// Only Full Disk Access allows it. The file is present on every Mac, which
/// matters — an earlier version probed a directory under the user's own Library
/// that does not exist on all machines, so a *missing* directory and a
/// *forbidden* one were indistinguishable.
#[cfg(target_os = "macos")]
fn is_granted() -> bool {
    std::fs::File::open("/Library/Application Support/com.apple.TCC/TCC.db").is_ok()
}

#[cfg(not(target_os = "macos"))]
pub fn report() -> AccessReport {
    // Windows and Linux have no equivalent gate on a user's own files.
    AccessReport {
        granted: true,
        checked: 0,
        blocked: Vec::new(),
        applicable: false,
    }
}

/// The System Settings pane where the permission is granted.
///
/// There is no API to request Full Disk Access; opening the pane and explaining
/// the steps is the whole of what an app is permitted to do.
pub const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";
