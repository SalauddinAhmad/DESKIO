//! Platform-neutral types shared by every scanner, the UI, and the CLI.
//!
//! Nothing in this file is OS-specific. Platform adapters produce these types;
//! the removal engine and the UI only ever see these types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// How sure we are that an item really belongs to the app being removed.
///
/// This drives what is ticked by default in the review sheet. Getting it wrong
/// in the optimistic direction is how an uninstaller destroys user data, so the
/// scanners are required to justify every `High` with an exact identifier match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Fuzzy/name-substring match. Shown, never ticked by default.
    Low,
    /// Display-name or vendor-directory match. Shown, not ticked by default.
    Medium,
    /// Exact bundle-id / package-id / registry-key match. Ticked by default.
    High,
}

impl Confidence {
    /// Only `High` is safe to pre-select for the user.
    pub fn preselected(self) -> bool {
        self == Confidence::High
    }
}

/// Where an installed app was found. Determines how it must be uninstalled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppSource {
    /// macOS: `/Applications` (or a subfolder of it).
    Applications,
    /// macOS: `~/Applications`.
    UserApplications,
    /// macOS: installed from the Mac App Store.
    MacAppStore,
    /// macOS: managed by Setapp.
    Setapp,
    /// macOS: installed by a `.pkg`, tracked in `/var/db/receipts`.
    PkgReceipt,
    /// Windows: an `Uninstall` registry key (HKLM/HKCU, native or WOW6432Node).
    WindowsRegistry,
    /// Windows: an MSI product.
    WindowsMsi,
    /// Windows: an Appx/MSIX package.
    WindowsAppx,
    /// Linux: dpkg/apt.
    Dpkg,
    /// Linux: rpm/dnf.
    Rpm,
    /// Linux: pacman.
    Pacman,
    /// Linux: flatpak.
    Flatpak,
    /// Linux: snap.
    Snap,
    /// Linux: a bare AppImage file.
    AppImage,
    Unknown,
}

impl AppSource {
    /// True when the platform owns the uninstall and we must delegate to it
    /// (a package manager, or the vendor's own uninstaller) rather than
    /// deleting files ourselves.
    pub fn delegates_uninstall(self) -> bool {
        matches!(
            self,
            AppSource::WindowsRegistry
                | AppSource::WindowsMsi
                | AppSource::WindowsAppx
                | AppSource::Dpkg
                | AppSource::Rpm
                | AppSource::Pacman
                | AppSource::Flatpak
                | AppSource::Snap
        )
    }
}

/// An application that is currently installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    /// Stable key for this app across scans. Bundle id on macOS, registry key
    /// name on Windows, package name on Linux. Falls back to the path.
    pub id: String,
    pub name: String,
    /// The `.app` bundle, install directory, or package root. `None` for
    /// packages with no single owning directory.
    pub path: Option<PathBuf>,
    /// macOS `CFBundleIdentifier`. The single most useful matching token we have.
    pub bundle_id: Option<String>,
    /// The main binary's name — `CFBundleExecutable` on macOS, the `.exe` name
    /// on Windows. Apps name their stray files after it surprisingly often
    /// (`fdm_<uuid>.plist` for a binary called `fdm`), so it is worth matching on.
    pub executable: Option<String>,
    pub version: Option<String>,
    /// Signing authority on macOS, `Publisher` on Windows, maintainer on Linux.
    pub publisher: Option<String>,
    /// Total size on disk of the app itself, excluding leftovers.
    pub size_bytes: u64,
    pub source: AppSource,
    /// PNG icon, base64. Populated lazily — list scans leave this `None`.
    pub icon_png_base64: Option<String>,
    pub created_at: Option<i64>,
    pub modified_at: Option<i64>,
    pub last_opened_at: Option<i64>,
    /// macOS notarisation status. `None` when not checked or not applicable.
    pub notarized: Option<bool>,
    /// True when the app is running right now and must be quit before removal.
    pub is_running: bool,
    /// True when this is an OS-supplied app that must never be offered for removal.
    pub is_system: bool,
    /// Who the app was installed for, where the platform distinguishes it —
    /// "All users" or "You" on Windows. The same product can legitimately be
    /// installed both ways, which otherwise looks like a duplicate entry with
    /// no way to tell the two apart.
    #[serde(default)]
    pub scope: Option<String>,
}

/// What kind of leftover a path is. Used for grouping in the UI and for
/// deciding whether admin rights are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftoverKind {
    Preferences,
    Caches,
    ApplicationSupport,
    Container,
    GroupContainer,
    Logs,
    SavedState,
    LaunchAgent,
    LaunchDaemon,
    PrivilegedHelper,
    Cookies,
    WebData,
    Receipt,
    Extension,
    RegistryKey,
    CrashReport,
    Other,
}

impl LeftoverKind {
    pub fn label(self) -> &'static str {
        match self {
            LeftoverKind::Preferences => "Preferences",
            LeftoverKind::Caches => "Caches",
            LeftoverKind::ApplicationSupport => "Application Support",
            LeftoverKind::Container => "Container",
            LeftoverKind::GroupContainer => "Group Container",
            LeftoverKind::Logs => "Logs",
            LeftoverKind::SavedState => "Saved Application State",
            LeftoverKind::LaunchAgent => "Launch Agent",
            LeftoverKind::LaunchDaemon => "Launch Daemon",
            LeftoverKind::PrivilegedHelper => "Privileged Helper",
            LeftoverKind::Cookies => "Cookies",
            LeftoverKind::WebData => "Web Data",
            LeftoverKind::Receipt => "Installer Receipt",
            LeftoverKind::Extension => "Extension",
            LeftoverKind::RegistryKey => "Registry Key",
            LeftoverKind::CrashReport => "Crash Report",
            LeftoverKind::Other => "Other",
        }
    }
}

/// A file or folder left behind by an app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leftover {
    pub path: PathBuf,
    /// Display name — the final path component.
    pub name: String,
    pub size_bytes: u64,
    /// True when the size could not be measured because the directory is
    /// unreadable — on macOS, the signature of missing Full Disk Access.
    /// The UI must show "size unknown", never "Zero KB": telling someone a
    /// four-gigabyte folder is empty is worse than admitting we cannot see it.
    pub size_unknown: bool,
    pub is_directory: bool,
    pub kind: LeftoverKind,
    pub confidence: Confidence,
    /// Human-readable justification, shown in the UI so the user can judge for
    /// themselves. e.g. "matches bundle id org.freedownloadmanager.fdm6".
    /// Every scanner MUST fill this in — an unexplained deletion is not
    /// something we ask a user to approve.
    pub reason: String,
    /// True when removing this needs an admin prompt (anything outside `$HOME`).
    pub requires_admin: bool,
    /// Set when this path also matches a *different* installed app, i.e. it is
    /// a shared vendor directory. Such items are never pre-selected and the UI
    /// warns about them explicitly.
    pub shared_with: Vec<String>,
    /// Set when this is a registry key rather than a file, e.g.
    /// `HKCU\Software\Vendor\Product`. A key cannot go to the Recycle Bin, so
    /// removal exports it to a `.reg` file first — that export *is* its undo.
    /// When this is set, `path` carries the same string for display only.
    #[serde(default)]
    pub registry_key: Option<String>,
}

/// One line of a removal plan: an app bundle, a leftover, or a delegated
/// uninstaller invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalItem {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub size_unknown: bool,
    pub is_directory: bool,
    pub kind: LeftoverKind,
    pub confidence: Confidence,
    pub reason: String,
    pub requires_admin: bool,
    /// Whether this item is ticked. Defaults to `confidence.preselected()`.
    pub selected: bool,
    /// Set when this is a registry key rather than a file. See [`Leftover`].
    #[serde(default)]
    pub registry_key: Option<String>,
}

impl From<Leftover> for RemovalItem {
    fn from(l: Leftover) -> Self {
        RemovalItem {
            selected: l.confidence.preselected() && l.shared_with.is_empty(),
            path: l.path,
            name: l.name,
            size_bytes: l.size_bytes,
            size_unknown: l.size_unknown,
            is_directory: l.is_directory,
            kind: l.kind,
            confidence: l.confidence,
            reason: l.reason,
            requires_admin: l.requires_admin,
            registry_key: l.registry_key,
        }
    }
}

/// The dry run. Nothing is ever removed without the user seeing one of these
/// first — this is what the review sheet renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalPlan {
    pub app: Option<InstalledApp>,
    pub items: Vec<RemovalItem>,
    /// A delegated uninstall command that must run before the file sweep
    /// (the vendor's uninstaller on Windows, or a package manager).
    pub delegated_command: Option<String>,
}

impl RemovalPlan {
    pub fn selected_items(&self) -> impl Iterator<Item = &RemovalItem> {
        self.items.iter().filter(|i| i.selected)
    }
    pub fn selected_count(&self) -> usize {
        self.selected_items().count()
    }
    pub fn selected_bytes(&self) -> u64 {
        self.selected_items().map(|i| i.size_bytes).sum()
    }
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.size_bytes).sum()
    }
    pub fn needs_admin(&self) -> bool {
        self.selected_items().any(|i| i.requires_admin)
    }
}

/// Outcome of removing one item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalOutcome {
    pub path: PathBuf,
    pub removed: bool,
    /// The item was already gone before we got to it — usually because the
    /// application's own uninstaller had just removed it. That is the outcome
    /// the user wanted, so it counts as success, but it is reported separately
    /// rather than claimed as something this app moved.
    #[serde(default)]
    pub already_gone: bool,
    /// Where the item ended up in the trash, when that could be determined.
    /// This is what makes putting it back possible without Finder's help.
    #[serde(default)]
    pub trashed_to: Option<PathBuf>,
    /// Populated when `removed` is false — why it was skipped or failed.
    pub error: Option<String>,
}

/// How to carry out a removal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct RemovalOptions {
    /// Play the Finder trash sound per item. See [`crate::trash_bin`] for what
    /// this actually changes — it is not only a sound.
    pub sound: bool,
    /// Sweep the files even if the application's own uninstaller failed.
    ///
    /// Off by default, because removing files underneath a half-finished
    /// uninstaller is usually worse than stopping. It exists for the case
    /// where the uninstaller is simply broken or refuses to run, and the files
    /// are all that is left to clear.
    pub force: bool,
}

/// Result of executing a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalReport {
    pub outcomes: Vec<RemovalOutcome>,
    pub bytes_freed: u64,
    /// Id of the undo journal entry written for this removal.
    pub undo_id: Option<String>,
    /// Set when the application's own uninstaller failed and the sweep was
    /// abandoned. The interface uses this to offer going ahead anyway.
    #[serde(default)]
    pub delegated_failed: Option<String>,
    /// The application's own uninstaller ran and finished. Worth saying: most
    /// of the work was its, and what this app removed was the leftovers.
    #[serde(default)]
    pub delegated_ran: bool,
}

impl RemovalReport {
    pub fn removed_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.removed && !o.already_gone)
            .count()
    }

    /// Items that had already been removed by something else.
    pub fn already_gone_count(&self) -> usize {
        self.outcomes.iter().filter(|o| o.already_gone).count()
    }
    pub fn failed(&self) -> impl Iterator<Item = &RemovalOutcome> {
        self.outcomes.iter().filter(|o| !o.removed)
    }
}

/// Convert a `SystemTime` to unix seconds for serialisation.
pub fn unix_secs(t: Option<SystemTime>) -> Option<i64> {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Human-readable byte size, matching the reference app's style ("180.5 MB").
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    let b = bytes as f64;
    if bytes == 0 {
        return "Zero KB".into();
    }
    let (val, unit) = if b < KB {
        (b, "bytes")
    } else if b < KB * KB {
        (b / KB, "KB")
    } else if b < KB * KB * KB {
        (b / (KB * KB), "MB")
    } else {
        (b / (KB * KB * KB), "GB")
    };
    if unit == "bytes" || val >= 100.0 {
        format!("{:.0} {}", val, unit)
    } else {
        format!("{:.1} {}", val, unit)
    }
}
