//! Reclaimable junk: caches, logs, crash reports, build artefacts.
//!
//! ## What this is not
//!
//! It is not a "speed up your Mac" cleaner. It reports what is genuinely
//! regenerable and lets the user decide, with sizes and full paths, and it goes
//! through the same review sheet and the same safety rules as everything else.
//!
//! ## Space is freed when the Trash is emptied, not before
//!
//! Everything here is moved to the Trash like every other removal, which means
//! the disk does not get any emptier until the user empties it. A cleaner that
//! quietly deleted instead would free space immediately — and would be the one
//! part of the app with no undo. The UI says this plainly rather than reporting
//! a number that has not happened yet.

use crate::model::{Confidence, Leftover, LeftoverKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JunkCategory {
    AppCaches,
    Logs,
    CrashReports,
    DeveloperJunk,
    Trash,
}

impl JunkCategory {
    pub fn label(self) -> &'static str {
        match self {
            JunkCategory::AppCaches => "Application Caches",
            JunkCategory::Logs => "Logs",
            JunkCategory::CrashReports => "Crash Reports",
            JunkCategory::DeveloperJunk => "Developer Junk",
            JunkCategory::Trash => "Trash",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            JunkCategory::AppCaches => {
                "Working files apps rebuild on their own. Clearing them can make an \
                 app's next launch slower, and nothing else."
            }
            JunkCategory::Logs => "Diagnostic logs written by installed apps.",
            JunkCategory::CrashReports => "Reports left behind when an app crashed.",
            JunkCategory::DeveloperJunk => {
                "Build output, archives and device-support files from Xcode. Large, and \
                 regenerated the next time you build."
            }
            JunkCategory::Trash => {
                "Already in the Trash. DESKIO does not empty it — that is the one \
                 action with no undo, and it belongs to you and Finder."
            }
        }
    }

    /// False for anything this app reports but will not act on.
    pub fn removable(self) -> bool {
        self != JunkCategory::Trash
    }

    pub fn all() -> [JunkCategory; 5] {
        [
            JunkCategory::AppCaches,
            JunkCategory::DeveloperJunk,
            JunkCategory::Logs,
            JunkCategory::CrashReports,
            JunkCategory::Trash,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunkItem {
    pub id: String,
    pub name: String,
    pub category: JunkCategory,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub size_unknown: bool,
    pub is_directory: bool,
    pub requires_admin: bool,
    pub detail: Option<String>,
}

impl JunkItem {
    pub fn to_leftover(&self) -> Leftover {
        Leftover {
            path: self.path.clone(),
            name: self.name.clone(),
            size_bytes: self.size_bytes,
            size_unknown: self.size_unknown,
            is_directory: self.is_directory,
            kind: match self.category {
                JunkCategory::Logs => LeftoverKind::Logs,
                JunkCategory::CrashReports => LeftoverKind::CrashReport,
                _ => LeftoverKind::Caches,
            },
            // The user picks these by category; nothing here is ever inferred
            // to belong to anything, so nothing is pre-ticked for them.
            confidence: Confidence::Medium,
            reason: self
                .detail
                .clone()
                .unwrap_or_else(|| self.category.label().to_string()),
            requires_admin: self.requires_admin,
            shared_with: Vec::new(),
            registry_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunkGroup {
    pub category: JunkCategory,
    pub label: String,
    pub description: String,
    pub removable: bool,
    pub items: Vec<JunkItem>,
    pub size_bytes: u64,
}

/// Everything reclaimable, grouped. Empty groups are kept so the UI can show
/// "nothing here" rather than a category that silently vanishes.
pub fn scan() -> Vec<JunkGroup> {
    let items = imp::scan();
    JunkCategory::all()
        .into_iter()
        .map(|category| {
            let mut items: Vec<JunkItem> = items
                .iter()
                .filter(|i| i.category == category)
                .cloned()
                .collect();
            items.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
            JunkGroup {
                category,
                label: category.label().to_string(),
                description: category.description().to_string(),
                removable: category.removable(),
                size_bytes: items.iter().map(|i| i.size_bytes).sum(),
                items,
            }
        })
        .collect()
}
