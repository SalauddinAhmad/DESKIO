//! The removal journal, and putting things back.
//!
//! Every executed removal is recorded before the UI forgets about it: what was
//! removed, and where in the trash it landed. That second part is what lets
//! BHUninstaller restore a removal itself, rather than depending on Finder's
//! "Put Back" — which is unavailable when items are trashed silently, and never
//! worked for the items that needed an administrator password.

use crate::model::RemovalReport;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// One removed item: where it was, and where it went.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoItem {
    pub original: PathBuf,
    pub trashed: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    pub id: String,
    pub timestamp: i64,
    /// Name of the app the removal was for, when there was one.
    pub app_name: Option<String>,
    #[serde(default)]
    pub items: Vec<UndoItem>,
    /// Entries written before the journal recorded trash destinations.
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    pub bytes_freed: u64,
    /// How many items are still sitting in the trash and could be put back.
    /// Recomputed on every read — the trash is not ours to keep track of.
    #[serde(default)]
    pub restorable: usize,
}

impl UndoEntry {
    /// Every item in this entry, whatever format it was written in.
    pub fn all_items(&self) -> Vec<UndoItem> {
        if !self.items.is_empty() {
            return self.items.clone();
        }
        self.paths
            .iter()
            .map(|p| UndoItem {
                original: p.clone(),
                trashed: None,
            })
            .collect()
    }

    /// How many of these can actually be put back.
    pub fn restorable_count(&self) -> usize {
        self.all_items()
            .iter()
            .filter(|i| i.trashed.as_ref().is_some_and(|t| t.exists()))
            .count()
    }
}

/// `~/Library/Application Support/BHUninstaller` (or the platform equivalent).
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("BHUninstaller"))
}

fn journal_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("removals.jsonl"))
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Append a report to the journal. Returns the entry id.
///
/// A journal write failure must never fail the removal itself — by the time we
/// get here the files are already in the trash, and erroring out would only
/// confuse the user about whether it worked.
pub fn record(report: &RemovalReport, app_name: Option<String>) -> Option<String> {
    // A run that removed nothing is not history. Skipping it keeps refused and
    // cancelled attempts out of the log — and keeps the test suite, which
    // deliberately executes plans that must be refused, from writing to the
    // real journal in the user's home directory.
    if report.removed_count() == 0 {
        return None;
    }
    let path = journal_path()?;
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let entry = UndoEntry {
        id: format!("{}-{}", now_secs(), report.removed_count()),
        timestamp: now_secs(),
        app_name,
        items: report
            .outcomes
            .iter()
            .filter(|o| o.removed)
            .map(|o| UndoItem {
                original: o.path.clone(),
                trashed: o.trashed_to.clone(),
            })
            .collect(),
        paths: Vec::new(),
        bytes_freed: report.bytes_freed,
        restorable: 0,
    };
    let line = serde_json::to_string(&entry).ok()?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    writeln!(f, "{line}").ok()?;
    Some(entry.id)
}

/// Read the journal, newest first.
pub fn history() -> Vec<UndoEntry> {
    let Some(path) = journal_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut entries: Vec<UndoEntry> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<UndoEntry>(l).ok())
        .map(|mut e| {
            e.restorable = e.restorable_count();
            e
        })
        .collect();
    entries.reverse();
    entries
}

/// What happened to one item during a restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreOutcome {
    pub original: PathBuf,
    pub restored: bool,
    pub error: Option<String>,
}

/// Put a recorded removal back where it came from.
///
/// Refuses to overwrite: if something already exists at the original path, that
/// item is reported as skipped rather than clobbered. Restoring is meant to
/// undo a mistake, not to create a new one.
pub fn restore(entry_id: &str) -> Vec<RestoreOutcome> {
    let Some(entry) = history().into_iter().find(|e| e.id == entry_id) else {
        return Vec::new();
    };

    entry
        .all_items()
        .into_iter()
        .map(|item| {
            let fail = |why: &str| RestoreOutcome {
                original: item.original.clone(),
                restored: false,
                error: Some(why.to_string()),
            };

            let Some(trashed) = item.trashed.clone() else {
                return fail("no record of where this went in the trash");
            };
            if !trashed.exists() {
                return fail("no longer in the trash");
            }
            if registry_key_of(&item.original).is_none() && item.original.exists() {
                return fail("something already exists at the original location");
            }
            // A registry key comes back by importing the file it was exported
            // to, not by moving anything.
            if let Some(key) = registry_key_of(&item.original) {
                return match import_registry(&trashed) {
                    Ok(()) => RestoreOutcome {
                        original: item.original,
                        restored: true,
                        error: None,
                    },
                    Err(e) => fail(&format!("could not import {key}: {e}")),
                };
            }

            let Some(parent) = item.original.parent() else {
                return fail("the original location no longer makes sense");
            };
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return fail(&format!("could not recreate the folder: {e}"));
                }
            }
            match fs::rename(&trashed, &item.original) {
                Ok(()) => RestoreOutcome {
                    original: item.original,
                    restored: true,
                    error: None,
                },
                Err(e) => fail(&e.to_string()),
            }
        })
        .collect()
}

/// Recognise a recorded item as a registry key rather than a file.
fn registry_key_of(original: &std::path::Path) -> Option<String> {
    let text = original.to_string_lossy().to_string();
    let hive = text.split('\\').next()?.to_lowercase();
    matches!(
        hive.as_str(),
        "hkcu" | "hkey_current_user" | "hklm" | "hkey_local_machine"
    )
    .then_some(text)
}

/// Put a key back from the `.reg` file it was exported to.
#[cfg(target_os = "windows")]
fn import_registry(backup: &std::path::Path) -> Result<(), String> {
    let out = crate::proc::command("reg")
        .arg("import")
        .arg(backup)
        .output()
        .map_err(|e| format!("could not run reg import: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(if err.is_empty() {
        "reg import failed".into()
    } else {
        err
    })
}

#[cfg(not(target_os = "windows"))]
fn import_registry(_backup: &std::path::Path) -> Result<(), String> {
    Err("registry keys only exist on Windows".into())
}
