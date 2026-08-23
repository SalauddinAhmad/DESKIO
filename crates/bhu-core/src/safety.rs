//! The last line of defence.
//!
//! Every path is checked here immediately before it is moved to the trash —
//! not only when it was scanned. A scanner bug, a stale plan, a symlink swapped
//! between scan and execute, or a malformed path from the UI must all be caught
//! at this point.
//!
//! The rule is deliberately conservative: if a path is not clearly *inside* a
//! removable location, it is refused. Refusing a legitimate leftover is a minor
//! annoyance; allowing one wrong path destroys a user's data.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, thiserror::Error)]
#[error("refusing to remove {path}: {reason}")]
pub struct SafetyError {
    pub path: PathBuf,
    pub reason: String,
}

fn deny(path: &Path, reason: impl Into<String>) -> SafetyError {
    SafetyError {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

/// Directories that may contain removable items but must never themselves be
/// removed, and under which we additionally refuse to remove a *direct* child
/// that is one of the standard user folders.
fn protected_exact() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    let mut push = |s: &str| v.push(PathBuf::from(s));

    // Cross-platform filesystem roots and top-level system directories.
    for p in [
        "/", "/bin", "/sbin", "/usr", "/etc", "/var", "/tmp", "/opt", "/dev", "/proc", "/sys",
        "/boot", "/root", "/home", "/srv", "/run", "/mnt", "/media",
    ] {
        push(p);
    }

    // macOS.
    for p in [
        "/Applications",
        "/Applications/Utilities",
        "/Library",
        "/System",
        "/Users",
        "/Volumes",
        "/private",
        "/private/var",
        "/private/tmp",
        "/private/etc",
        "/usr/local",
        "/opt/homebrew",
    ] {
        push(p);
    }
    // The macOS shared-library subfolders are containers for leftovers; the
    // folders themselves are structural.
    for sub in MAC_LIBRARY_SUBDIRS {
        push(&format!("/Library/{sub}"));
    }

    // Windows.
    for p in [
        r"C:\",
        r"C:\Windows",
        r"C:\Windows\System32",
        r"C:\Program Files",
        r"C:\Program Files (x86)",
        r"C:\ProgramData",
        r"C:\Users",
        r"C:\Users\Public",
    ] {
        push(p);
    }

    if let Some(home) = dirs::home_dir() {
        v.push(home.clone());
        // Standard user folders — never removable, on any platform.
        for sub in [
            "Documents",
            "Desktop",
            "Downloads",
            "Pictures",
            "Movies",
            "Music",
            "Videos",
            "Public",
            "Applications",
            "Library",
            "iCloud Drive",
            ".config",
            ".local",
            ".cache",
            ".ssh",
            ".gnupg",
            ".var",
            "AppData",
            "OneDrive",
        ] {
            v.push(home.join(sub));
        }
        // macOS: each ~/Library container folder.
        for sub in MAC_LIBRARY_SUBDIRS {
            v.push(home.join("Library").join(sub));
        }
        // Linux: the roots that hold per-app config.
        for sub in [".local/share", ".local/state", ".config/autostart"] {
            v.push(home.join(sub));
        }
        // Windows: the roots that hold per-app data.
        for sub in [r"AppData\Roaming", r"AppData\Local", r"AppData\LocalLow"] {
            v.push(home.join(sub));
        }
    }
    v
}

/// `~/Library/<sub>` and `/Library/<sub>` folders that hold per-app data.
/// These are scan roots — their *children* are removable, they are not.
pub const MAC_LIBRARY_SUBDIRS: &[&str] = &[
    "Application Support",
    "Application Scripts",
    "Caches",
    "Containers",
    "Group Containers",
    "Preferences",
    "Preferences/ByHost",
    "Logs",
    "LaunchAgents",
    "LaunchDaemons",
    "PrivilegedHelperTools",
    "Saved Application State",
    "WebKit",
    "HTTPStorages",
    "Cookies",
    "Internet Plug-Ins",
    "PreferencePanes",
    "Screen Savers",
    "Services",
    "Extensions",
    "StartupItems",
    "Widgets",
    "Frameworks",
    "Fonts",
];

/// Trees we refuse to touch at any depth.
fn protected_prefixes() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [
        "/System",
        "/bin",
        "/sbin",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/share",
        "/etc",
        "/dev",
        "/proc",
        "/sys",
        "/boot",
        "/private/etc",
        // Everything under /var is system state; the one exception below
        // (installer receipts) is carved back out.
        "/var",
        "/private/var",
        "/Library/Apple",
        r"C:\Windows",
        r"C:\Program Files\WindowsApps",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();

    if let Some(home) = dirs::home_dir() {
        // Never reach into the user's actual documents, no matter what an app
        // claims to own there. This is the rule that stops a "clean up
        // leftovers" pass from eating someone's work.
        for sub in [
            "Documents",
            "Desktop",
            "Downloads",
            "Pictures",
            "Movies",
            "Music",
            "Videos",
            ".ssh",
            ".gnupg",
        ] {
            v.push(home.join(sub));
        }
    }
    v
}

/// Trees that are safe to remove from despite sitting under a protected prefix.
fn allowed_exceptions() -> Vec<PathBuf> {
    vec![
        // Installer receipts are genuine leftovers and are meant to be pruned
        // when the package they describe is gone.
        PathBuf::from("/private/var/db/receipts"),
        PathBuf::from("/var/db/receipts"),
    ]
}

/// Downloaded installers that may be removed from `~/Downloads`.
///
/// `~/Downloads` is otherwise refused outright, and that stays true for
/// everything else in it. This carve-out is deliberately as narrow as it can
/// be: a **regular file**, sitting **directly** in Downloads, whose extension
/// is one of a fixed list of installer formats. A document, a folder, or
/// anything one level deeper is still refused, so no plan — however wrong —
/// can reach a user's actual downloads.
const INSTALLER_EXTENSIONS: &[&str] = &["dmg", "pkg", "mpkg", "iso", "msi", "exe", "deb", "rpm"];

fn is_removable_installer(path: &Path) -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let downloads = home.join("Downloads");
    if path.parent() != Some(downloads.as_path()) {
        return false;
    }
    if !path.is_file() {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| INSTALLER_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Minimum number of path components (excluding the root) a removable path must
/// have. `/Applications/Foo.app` has two; anything shallower is structural.
const MIN_COMPONENTS: usize = 2;

/// Check whether a path may be removed.
///
/// Call this immediately before removal, every time — never rely on a check
/// performed at scan time.
pub fn check_removable(path: &Path) -> Result<(), SafetyError> {
    if path.as_os_str().is_empty() {
        return Err(deny(path, "empty path"));
    }
    if !path.is_absolute() {
        return Err(deny(path, "path is not absolute"));
    }

    // A `..` component means the real target is not what the path claims. We do
    // not normalise it away and continue — we refuse, because a plan should
    // never have contained one.
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(deny(path, "path contains a '..' component"));
    }

    let depth = path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    if depth < MIN_COMPONENTS {
        return Err(deny(
            path,
            format!("path is only {depth} level(s) deep; refusing anything shallower than {MIN_COMPONENTS}"),
        ));
    }

    // Wildcards, globs and shell metacharacters have no business in a path we
    // are about to delete.
    let as_str = path.to_string_lossy();
    if as_str.contains('*') || as_str.contains('?') {
        return Err(deny(path, "path contains a wildcard"));
    }

    for p in protected_exact() {
        if path == p {
            return Err(deny(path, "this is a system or standard user folder"));
        }
    }

    let installer = is_removable_installer(path);
    let exceptions = allowed_exceptions();
    for prefix in protected_prefixes() {
        if path.starts_with(&prefix) {
            let excepted = installer
                || exceptions
                    .iter()
                    .any(|e| path.starts_with(e) && e.starts_with(&prefix));
            if !excepted {
                return Err(deny(
                    path,
                    format!("path is inside the protected tree {}", prefix.display()),
                ));
            }
        }
    }

    Ok(())
}

/// True when removing this path will need an administrator password.
///
/// This asks the operating system whether we can actually write to the
/// containing directory, rather than assuming anything outside `$HOME` is
/// privileged. On macOS `/Applications` is group-writable by admin users, so
/// the assumption would make us prompt for a password on almost every
/// uninstall — which trains people to type it without reading.
pub fn requires_admin(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return true;
    };
    !is_writable(parent)
}

#[cfg(unix)]
fn is_writable(dir: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c` is a valid NUL-terminated C string for the duration of the call.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn is_writable(dir: &Path) -> bool {
    // Windows has no cheap equivalent; fall back to the home-directory rule.
    match dirs::home_dir() {
        Some(home) => dir.starts_with(home),
        None => false,
    }
}

/// Registry subtrees that hold entries for the whole system rather than for one
/// application. Nothing inside them at this level is ever removable.
const RESERVED_KEYS: &[&str] = &[
    "classes",
    "microsoft",
    "policies",
    "windows",
    "clients",
    "registeredapplications",
    "wow6432node",
    "intel",
    "odbc",
    "khronos",
];

/// Check whether a registry key may be removed.
///
/// The registry is not a filesystem and a wrong write there is worse than a
/// wrong file delete: it can leave other software subtly broken with nothing to
/// point at. So the rule is narrower than for files — only keys that live where
/// applications keep their own settings are eligible at all:
///
/// ```text
///   HKCU\Software\<Vendor>[\<Product>…]
///   HKLM\SOFTWARE\<Vendor>[\<Product>…]      (and the WOW6432Node mirror)
/// ```
///
/// Everything else — `SYSTEM`, `SAM`, `HARDWARE`, the hive roots, `Software`
/// itself, and the shared subtrees above — is refused outright, whatever a plan
/// claims. As with files, this runs immediately before the key is touched.
pub fn check_registry_removable(key: &str) -> Result<(), SafetyError> {
    let as_path = PathBuf::from(key);
    let deny = |reason: &str| {
        Err(SafetyError {
            path: as_path.clone(),
            reason: reason.to_string(),
        })
    };

    if key.trim().is_empty() {
        return deny("empty registry key");
    }
    if key.contains('*') || key.contains('?') {
        return deny("registry key contains a wildcard");
    }

    let parts: Vec<&str> = key
        .split('\\')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.contains(&"..") {
        return deny("registry key contains a '..' component");
    }
    if parts.len() < 3 {
        return deny("registry key is too shallow to belong to one application");
    }

    // Only the two hives where applications keep their own settings.
    let hive = parts[0].to_lowercase();
    if !matches!(
        hive.as_str(),
        "hkcu" | "hkey_current_user" | "hklm" | "hkey_local_machine"
    ) {
        return deny("only HKCU and HKLM are ever touched");
    }
    if !parts[1].eq_ignore_ascii_case("software") {
        return deny("only keys under Software are ever touched");
    }

    // The 32-bit mirror is the same namespace one level down.
    let mut rest = &parts[2..];
    if rest
        .first()
        .map(|p| p.eq_ignore_ascii_case("wow6432node"))
        .unwrap_or(false)
    {
        rest = &rest[1..];
    }
    let Some(first) = rest.first() else {
        return deny("this is a shared registry root, not an application's key");
    };
    if RESERVED_KEYS.contains(&first.to_lowercase().as_str()) {
        return deny(&format!(
            "Software\\{first} holds entries for the whole system"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        dirs::home_dir().expect("home dir")
    }

    #[test]
    fn registry_refuses_everything_outside_an_application_area() {
        for key in [
            "",
            "HKCU",
            "HKCU\\Software",
            "HKLM\\SOFTWARE",
            "HKLM\\SYSTEM\\CurrentControlSet\\Services\\Tcpip",
            "HKCR\\.txt",
            "HKEY_USERS\\S-1-5-21\\Software\\Foo",
            "HKCU\\Software\\*",
            "HKCU\\Software\\..\\..\\SYSTEM",
        ] {
            assert!(
                check_registry_removable(key).is_err(),
                "should have refused {key}"
            );
        }
    }

    #[test]
    fn registry_refuses_the_shared_subtrees() {
        for key in [
            "HKCU\\Software\\Microsoft",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "HKCU\\Software\\Classes",
            "HKLM\\SOFTWARE\\Policies\\Microsoft",
            "HKLM\\SOFTWARE\\WOW6432Node",
            "HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Office",
            "HKCU\\Software\\RegisteredApplications",
        ] {
            assert!(
                check_registry_removable(key).is_err(),
                "should have refused {key}"
            );
        }
    }

    #[test]
    fn registry_allows_an_application_s_own_key() {
        for key in [
            "HKCU\\Software\\Softdeluxe\\Free Download Manager",
            "HKCU\\Software\\FreeDownloadManager",
            "HKLM\\SOFTWARE\\VS Revo Group\\Revo Uninstaller Pro",
            "HKLM\\SOFTWARE\\WOW6432Node\\Acme\\Widget",
            "HKEY_CURRENT_USER\\Software\\Acme\\Widget\\Settings",
        ] {
            assert!(
                check_registry_removable(key).is_ok(),
                "should have allowed {key}"
            );
        }
    }

    #[test]
    fn refuses_filesystem_root_and_shallow_paths() {
        assert!(check_removable(Path::new("/")).is_err());
        assert!(check_removable(Path::new("/Applications")).is_err());
        assert!(check_removable(Path::new("/Library")).is_err());
        assert!(check_removable(Path::new("/Users")).is_err());
    }

    #[test]
    fn refuses_the_home_directory_and_its_standard_folders() {
        assert!(check_removable(&home()).is_err());
        assert!(check_removable(&home().join("Documents")).is_err());
        assert!(check_removable(&home().join("Library")).is_err());
        assert!(check_removable(&home().join("Library/Caches")).is_err());
        assert!(check_removable(&home().join("Library/Preferences")).is_err());
    }

    #[test]
    fn refuses_anything_inside_the_users_own_documents() {
        assert!(check_removable(&home().join("Documents/taxes/2026.numbers")).is_err());
        assert!(check_removable(&home().join("Desktop/anything")).is_err());
        assert!(check_removable(&home().join(".ssh/id_ed25519")).is_err());
    }

    #[test]
    fn refuses_system_trees() {
        assert!(check_removable(Path::new("/System/Library/CoreServices")).is_err());
        assert!(check_removable(Path::new("/usr/bin/ssh")).is_err());
        assert!(check_removable(Path::new("/etc/hosts")).is_err());
    }

    #[test]
    fn refuses_traversal_and_wildcards() {
        assert!(check_removable(Path::new("/Applications/../System")).is_err());
        assert!(check_removable(Path::new("/Applications/*.app")).is_err());
        assert!(check_removable(Path::new("relative/path")).is_err());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn allows_real_leftovers() {
        assert!(check_removable(Path::new("/Applications/Free Download Manager.app")).is_ok());
        assert!(check_removable(
            &home().join("Library/Preferences/org.freedownloadmanager.fdm6.plist")
        )
        .is_ok());
        assert!(check_removable(&home().join("Library/Caches/Softdeluxe")).is_ok());
        assert!(check_removable(&home().join("Library/Containers/com.example.app")).is_ok());
    }

    /// The same rule stated without any macOS path in it: something a couple of
    /// levels inside the user's own data directories is removable, wherever
    /// those directories happen to live.
    #[test]
    fn allows_a_leftover_inside_the_users_app_data() {
        #[cfg(target_os = "windows")]
        let leftover = home().join(r"AppData\Roaming\Acme\Widget");
        #[cfg(target_os = "linux")]
        let leftover = home().join(".config/acme-widget");
        #[cfg(target_os = "macos")]
        let leftover = home().join("Library/Application Support/Acme");

        assert!(check_removable(&leftover).is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn allows_installer_receipts_despite_living_under_private_var() {
        assert!(
            check_removable(Path::new("/private/var/db/receipts/com.example.pkg.plist")).is_ok()
        );
        // ...but not the rest of /private/var.
        assert!(check_removable(Path::new("/private/var/db/something-else")).is_err());
    }

    #[test]
    fn the_downloads_carve_out_reaches_installers_and_nothing_else() {
        let dl = home().join("Downloads");
        // Not real files, so the is_file() check refuses them — which is itself
        // the point: the carve-out only ever applies to something that exists
        // and is a plain file.
        assert!(check_removable(&dl.join("Some App.dmg")).is_err());
        // And these can never qualify, whatever else is true of them.
        assert!(check_removable(&dl.join("taxes-2026.pdf")).is_err());
        assert!(check_removable(&dl.join("photos")).is_err());
        assert!(check_removable(&dl.join("nested/installer.dmg")).is_err());
        assert!(check_removable(&dl).is_err());
    }

    #[test]
    fn a_real_installer_in_downloads_is_allowed_but_a_real_document_is_not() {
        let dl = home().join("Downloads");
        if !dl.is_dir() {
            return;
        }
        let dmg = dl.join("bhu-safety-probe.dmg");
        let doc = dl.join("bhu-safety-probe.pdf");
        let sub = dl.join("bhu-safety-probe-dir");
        std::fs::write(&dmg, b"x").ok();
        std::fs::write(&doc, b"x").ok();
        std::fs::create_dir_all(sub.join("inner")).ok();
        let nested = sub.join("inner/installer.dmg");
        std::fs::write(&nested, b"x").ok();

        let dmg_ok = check_removable(&dmg).is_ok();
        let doc_ok = check_removable(&doc).is_ok();
        let nested_ok = check_removable(&nested).is_ok();

        std::fs::remove_file(&dmg).ok();
        std::fs::remove_file(&doc).ok();
        std::fs::remove_dir_all(&sub).ok();

        assert!(
            dmg_ok,
            "an installer directly in Downloads should be removable"
        );
        assert!(!doc_ok, "a document in Downloads must never be removable");
        assert!(
            !nested_ok,
            "an installer in a subfolder must not be removable"
        );
    }

    #[test]
    fn admin_is_decided_by_real_write_permission() {
        // What matters is whether the *containing* directory is writable, so
        // this uses a directory that genuinely exists on the machine running
        // the test rather than one that happens to exist on the author's.
        assert!(
            !requires_admin(&home().join("bhu-permission-probe")),
            "a path in the user's own home should not need a password"
        );

        #[cfg(target_os = "windows")]
        let system_path = Path::new(r"C:\Windows\System32\bhu-probe");
        #[cfg(not(target_os = "windows"))]
        let system_path = Path::new("/usr/lib/bhu-probe");

        assert!(
            requires_admin(system_path),
            "a path in a system directory should need a password"
        );
    }
}
