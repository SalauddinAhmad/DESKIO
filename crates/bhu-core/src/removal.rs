//! Planning and executing removals.
//!
//! Two separate steps on purpose: `build_plan` produces something the user
//! reads and edits, and `execute` acts only on what that plan says is selected.
//! There is no path from "user clicked Uninstall" straight to a filesystem
//! change without a plan in between.

use crate::model::*;
use crate::safety;
use crate::trash_bin;
use crate::undo;
use std::fs;

/// Build the dry run for uninstalling an app.
///
/// The app's own bundle is always the first item and is always pre-selected;
/// leftovers are selected according to their confidence.
pub fn build_plan(app: InstalledApp, leftovers: Vec<Leftover>) -> RemovalPlan {
    let mut items: Vec<RemovalItem> = Vec::with_capacity(leftovers.len() + 1);

    if let Some(path) = app.path.clone() {
        items.push(RemovalItem {
            name: app.name.clone(),
            size_bytes: app.size_bytes,
            size_unknown: false,
            is_directory: path.is_dir(),
            requires_admin: safety::requires_admin(&path),
            path,
            kind: LeftoverKind::Other,
            confidence: Confidence::High,
            reason: "the application itself".into(),
            selected: true,
        });
    }

    items.extend(leftovers.into_iter().map(RemovalItem::from));

    // Highest confidence first, then largest — so the things the user most
    // needs to scrutinise are not buried at the bottom of a long list.
    items.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(b.size_bytes.cmp(&a.size_bytes))
    });

    RemovalPlan {
        delegated_command: None,
        app: Some(app),
        items,
    }
}

/// Build a plan for orphaned leftovers — the "Remaining Files" case, where
/// there is no app to uninstall because it is already gone.
pub fn build_orphan_plan(leftovers: Vec<Leftover>) -> RemovalPlan {
    let mut items: Vec<RemovalItem> = leftovers.into_iter().map(RemovalItem::from).collect();
    items.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(b.size_bytes.cmp(&a.size_bytes))
    });
    RemovalPlan {
        app: None,
        items,
        delegated_command: None,
    }
}

/// Execute the selected items of a plan.
///
/// Each item is re-validated against the safety rules and re-checked on disk
/// immediately before it is touched. A plan can be minutes old by the time the
/// user clicks Remove; the filesystem may have changed under it.
///
/// `opts` carries the trash behaviour — see [`crate::trash_bin`] for why the
/// sound setting also decides whether Finder can put items back.
///
/// Items the user cannot write to are set aside and moved in one privileged
/// batch at the end, so the password is asked for once rather than per file —
/// and only after everything that needs no password has already succeeded.
pub fn execute(plan: &RemovalPlan, opts: RemovalOptions) -> RemovalReport {
    let mut outcomes = Vec::new();
    let mut bytes_freed = 0u64;
    let mut deferred: Vec<(std::path::PathBuf, u64)> = Vec::new();

    // Where the platform owns the uninstall, its own uninstaller runs first and
    // the sweep only happens if it succeeded. Removing files underneath a
    // failed or cancelled uninstaller would leave the system describing
    // software that is half gone — worse than not having started.
    if let Some(command) = &plan.delegated_command {
        if let Err(e) = run_delegated(command) {
            return RemovalReport {
                outcomes: plan
                    .selected_items()
                    .map(|i| RemovalOutcome {
                        path: i.path.clone(),
                        removed: false,
                        trashed_to: None,
                        error: Some(e.clone()),
                    })
                    .collect(),
                bytes_freed: 0,
                undo_id: None,
            };
        }
    }

    for item in plan.selected_items() {
        // The safety check runs here, at the point of no return.
        if let Err(e) = safety::check_removable(&item.path) {
            outcomes.push(RemovalOutcome {
                path: item.path.clone(),
                removed: false,
                trashed_to: None,
                error: Some(e.to_string()),
            });
            continue;
        }
        // `symlink_metadata` so a symlink is examined as itself rather than
        // followed to whatever it points at.
        let Ok(meta) = fs::symlink_metadata(&item.path) else {
            outcomes.push(RemovalOutcome {
                path: item.path.clone(),
                removed: false,
                trashed_to: None,
                error: Some("no longer exists".into()),
            });
            continue;
        };
        let size = if meta.is_symlink() {
            0
        } else {
            crate::fsutil::size_on_disk(&item.path)
        };

        if crate::elevate::needs_elevation(&item.path) {
            deferred.push((item.path.clone(), size));
            continue;
        }

        match trash_bin::move_to_trash(&item.path, opts.sound) {
            Ok(trashed_to) => {
                bytes_freed += size;
                outcomes.push(RemovalOutcome {
                    path: item.path.clone(),
                    removed: true,
                    trashed_to,
                    error: None,
                });
            }
            Err(e) => outcomes.push(RemovalOutcome {
                path: item.path.clone(),
                removed: false,
                trashed_to: None,
                error: Some(e.to_string()),
            }),
        }
    }

    if !deferred.is_empty() {
        let paths: Vec<std::path::PathBuf> = deferred.iter().map(|(p, _)| p.clone()).collect();
        match crate::elevate::trash_elevated(&paths, &timestamp()) {
            Ok(dest) => {
                for (path, size) in deferred {
                    bytes_freed += size;
                    let landed = path.file_name().map(|n| dest.join(n));
                    outcomes.push(RemovalOutcome {
                        path,
                        removed: true,
                        trashed_to: landed,
                        error: None,
                    });
                }
            }
            Err(e) => {
                for (path, _) in deferred {
                    outcomes.push(RemovalOutcome {
                        path,
                        removed: false,
                        trashed_to: None,
                        error: Some(e.clone()),
                    });
                }
            }
        }
    }

    let mut report = RemovalReport {
        outcomes,
        bytes_freed,
        undo_id: None,
    };
    report.undo_id = undo::record(&report, plan.app.as_ref().map(|a| a.name.clone()));
    report
}

/// Run the platform's own uninstaller and wait for it.
///
/// NOT YET EXERCISED — there is no delegated uninstall on macOS, so this path
/// only runs on Windows and Linux and has not been tried on either.
fn run_delegated(command: &str) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", command]);
        c
    };

    let status = cmd
        .status()
        .map_err(|e| format!("could not start the uninstaller: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "the application's own uninstaller did not finish (exit {}). Nothing else was removed.",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".into())
        ))
    }
}

/// A human-readable stamp for the quarantine folder name.
fn timestamp() -> String {
    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("/bin/date")
            .arg("+%Y-%m-%d %H.%M.%S")
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "removal".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> InstalledApp {
        InstalledApp {
            id: "com.example.app".into(),
            name: "Example".into(),
            path: Some("/Applications/Example.app".into()),
            bundle_id: Some("com.example.app".into()),
            executable: None,
            version: None,
            publisher: None,
            size_bytes: 100,
            source: AppSource::Applications,
            icon_png_base64: None,
            created_at: None,
            modified_at: None,
            last_opened_at: None,
            notarized: None,
            is_running: false,
            is_system: false,
        }
    }

    fn leftover(name: &str, conf: Confidence, shared: Vec<String>) -> Leftover {
        Leftover {
            path: dirs::home_dir().unwrap().join("Library/Caches").join(name),
            name: name.into(),
            size_bytes: 10,
            size_unknown: false,
            is_directory: true,
            kind: LeftoverKind::Caches,
            confidence: conf,
            reason: "test".into(),
            requires_admin: false,
            shared_with: shared,
        }
    }

    #[test]
    fn only_high_confidence_leftovers_are_preselected() {
        let plan = build_plan(
            app(),
            vec![
                leftover("high", Confidence::High, vec![]),
                leftover("medium", Confidence::Medium, vec![]),
                leftover("low", Confidence::Low, vec![]),
            ],
        );
        // The app bundle plus the one high-confidence leftover.
        assert_eq!(plan.selected_count(), 2);
        let selected: Vec<_> = plan.selected_items().map(|i| i.name.as_str()).collect();
        assert!(selected.contains(&"high"));
        assert!(!selected.contains(&"medium"));
        assert!(!selected.contains(&"low"));
    }

    #[test]
    fn shared_vendor_directories_are_never_preselected() {
        // Even at High confidence: if another installed app also lives in this
        // directory, removing it would take that app's data with it.
        let plan = build_plan(
            app(),
            vec![leftover(
                "Google",
                Confidence::High,
                vec!["Google Chrome".into()],
            )],
        );
        let selected: Vec<_> = plan.selected_items().map(|i| i.name.as_str()).collect();
        assert!(!selected.contains(&"Google"));
    }

    #[test]
    fn execute_refuses_a_plan_that_points_at_a_protected_path() {
        let mut plan = build_orphan_plan(vec![]);
        plan.items.push(RemovalItem {
            path: dirs::home_dir().unwrap().join("Documents"),
            name: "Documents".into(),
            size_bytes: 0,
            size_unknown: false,
            is_directory: true,
            kind: LeftoverKind::Other,
            confidence: Confidence::High,
            reason: "malicious or buggy plan".into(),
            requires_admin: false,
            selected: true,
        });
        let report = execute(&plan, RemovalOptions::default());
        assert_eq!(report.removed_count(), 0);
        assert_eq!(report.failed().count(), 1);
    }
}
