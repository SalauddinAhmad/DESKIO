//! # BHUninstaller engine
//!
//! Finds installed applications, works out what they have left scattered around
//! the system, and removes both — safely, reversibly, and only after the user
//! has seen exactly what will go.
//!
//! The whole engine is UI-free and platform-neutral. Everything OS-specific
//! lives in [`discovery`] and [`leftovers`] platform adapters; adding a platform
//! means writing two files, not a second application.
//!
//! ## The shape of a removal
//!
//! ```text
//!   discovery::installed_apps()  ->  Vec<InstalledApp>
//!   leftovers::for_app(app)      ->  Vec<Leftover>      (each with a confidence + a reason)
//!   removal::build_plan(..)      ->  RemovalPlan        (the dry run the user reads)
//!   removal::execute(&plan)      ->  RemovalReport      (moves to trash, writes the undo journal)
//! ```
//!
//! There is deliberately no shortcut from an app to a deletion. A plan always
//! sits in between, and every path in it is re-checked against [`safety`] at the
//! moment it is touched.

pub mod access;
pub mod cleaner;
pub mod discovery;
pub mod elevate;
pub mod extensions;
pub mod fsutil;
pub mod leftovers;
pub mod model;
pub mod removal;
pub mod safety;
#[cfg(feature = "updates")]
pub mod selfupdate;
pub mod settings;
pub mod startup;
pub mod trash_bin;
pub mod undo;
#[cfg(feature = "updates")]
pub mod updates;
pub mod version;

pub use model::*;

/// Base64 for the icon blobs handed to the UI.
#[cfg(target_os = "macos")]
pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Everything installed, sorted by name.
pub fn scan_apps() -> Vec<InstalledApp> {
    discovery::installed_apps(discovery::ScanOptions::default())
}

/// Build the dry run for uninstalling one app, including its leftovers.
///
/// `all_apps` is required, not optional: knowing what *else* is installed is
/// what lets the scanner tell an app's own vendor folder apart from one it
/// shares with its siblings.
pub fn plan_uninstall(app: &InstalledApp, all_apps: &[InstalledApp]) -> RemovalPlan {
    let leftovers = leftovers::for_app(app, all_apps);
    let mut plan = removal::build_plan(app.clone(), leftovers);
    plan.delegated_command = discovery::uninstall_command(app);
    plan
}

/// Leftovers belonging to apps that are no longer installed.
///
/// Ownership is checked against *every* app on the machine, including the
/// system ones we never offer to remove. Otherwise anything belonging to an app
/// we deliberately hid from the list would look abandoned, and we would offer
/// to delete the working files of software that is very much still installed.
pub fn scan_orphans() -> Vec<leftovers::OrphanGroup> {
    let all = discovery::installed_apps(discovery::ScanOptions {
        compute_sizes: false,
        include_system: true,
    });
    leftovers::orphans(&all)
}

/// The version of this engine, for the UI's about box and the update check.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
