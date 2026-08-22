//! Finding what is installed.
//!
//! Each platform module returns the same `Vec<InstalledApp>`. Nothing outside
//! these modules may contain OS-specific code.

use crate::model::InstalledApp;

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

/// Tuning for a scan. Sizes are the expensive part — a full `/Applications`
/// sweep walks every file of every app — so the UI can ask for a fast list
/// first and fill sizes in afterwards.
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    pub compute_sizes: bool,
    /// Include apps shipped with the OS. Off by default: they are not
    /// removable and listing them only invites a user to try.
    pub include_system: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            compute_sizes: true,
            include_system: false,
        }
    }
}

/// Every application installed on this machine.
pub fn installed_apps(opts: ScanOptions) -> Vec<InstalledApp> {
    let mut apps = imp::installed_apps(opts);
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Fill in the expensive per-app details shown in the detail pane: publisher,
/// notarisation, icon, last-opened date, running state.
///
/// Kept separate from the list scan because each of these costs a subprocess or
/// a Spotlight query, which is fine for one selected app and far too slow for
/// several hundred.
pub fn enrich(app: &mut InstalledApp) {
    imp::enrich(app);
}

/// The platform's own uninstall command for this app, where the platform owns
/// the uninstall.
///
/// On Windows and Linux an installer or package manager knows how to undo what
/// it did; removing its files ourselves would leave the system describing
/// software that is no longer there. So the command runs first, and only then
/// does the leftover sweep happen. macOS has no such thing — an app is a
/// folder — and returns `None`.
pub fn uninstall_command(app: &InstalledApp) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        imp::uninstall_command(app)
    }
}

/// Just the app's icon, as base64 PNG.
///
/// Separate from [`enrich`] because the list view wants every icon but none of
/// the other expensive details.
pub fn icon(app: &InstalledApp) -> Option<String> {
    imp::icon(app)
}

/// Icons for a whole list, extracted in parallel.
///
/// Each icon costs a subprocess, so doing several hundred in sequence takes
/// long enough that the list sits there showing placeholders. Spreading them
/// across threads turns that into a couple of seconds.
pub fn icons(apps: &[InstalledApp]) -> std::collections::HashMap<String, String> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8);
    let chunk = apps.len().div_ceil(workers).max(1);

    let mut out = std::collections::HashMap::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = apps
            .chunks(chunk)
            .map(|c| {
                scope.spawn(move || {
                    c.iter()
                        .filter_map(|a| imp::icon(a).map(|i| (a.id.clone(), i)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            out.extend(h.join().unwrap_or_default());
        }
    });
    out
}
