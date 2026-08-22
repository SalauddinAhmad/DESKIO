//! Removing things the user cannot write to.
//!
//! Launch daemons, privileged helpers and installer receipts live outside the
//! user's home directory, so a normal process cannot move them. These need an
//! administrator password.
//!
//! Rather than deleting them outright as root, they are moved — still as root —
//! into a timestamped folder inside the user's Trash, which is then handed back
//! to the user. Nothing is destroyed, the user can look inside and put anything
//! back by hand, and emptying the Trash disposes of it normally.
//!
//! Finder's "Put Back" does not work for these, because the move is a plain
//! `mv` rather than a Finder trash operation. The undo journal records where
//! everything came from, and the UI must say so rather than implying otherwise.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ask for an administrator password once and move every path into a quarantine
/// folder in the user's Trash. Returns the folder it created.
#[cfg(target_os = "macos")]
pub fn trash_elevated(paths: &[PathBuf], stamp: &str) -> Result<PathBuf, String> {
    use std::process::Command;

    if paths.is_empty() {
        return Err("nothing to remove".into());
    }
    let home = dirs::home_dir().ok_or("no home directory")?;
    let trash = home.join(".Trash");
    std::fs::create_dir_all(&trash)
        .map_err(|e| format!("could not prepare {}: {e}", trash.display()))?;

    // The destination has to be a directory this call has just created, not one
    // that was already sitting there. A predictable name that something else
    // could pre-place — as a symlink, say — would have `mv`, running as root,
    // write through it to wherever that link pointed.
    //
    // `create_dir` fails outright if the path exists at all, symlink included,
    // so a collision is refused rather than followed; the suffix makes
    // arranging one impractical in the first place.
    let dest = trash.join(format!(
        "BHUninstaller {stamp} ({}-{})",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir(&dest).map_err(|e| format!("could not prepare {}: {e}", dest.display()))?;

    // Confirm what we just made is a real directory and not something
    // substituted between the two calls.
    let meta = std::fs::symlink_metadata(&dest)
        .map_err(|e| format!("could not verify {}: {e}", dest.display()))?;
    if meta.is_symlink() || !meta.is_dir() {
        return Err("the quarantine folder was not what we created — refusing".into());
    }

    // Every path is passed as a separate argument and quoted by AppleScript's
    // `quoted form of`. Nothing is ever interpolated into the shell string —
    // a single quote in a filename would otherwise be arbitrary code as root.
    let script = r#"on run argv
    set destPath to item 1 of argv
    set uidgid to item 2 of argv
    set cmd to "/bin/mv -f"
    repeat with i from 3 to count of argv
        set cmd to cmd & " " & quoted form of (item i of argv)
    end repeat
    set cmd to cmd & " " & quoted form of destPath
    set cmd to cmd & " && /usr/sbin/chown -R " & uidgid & " " & quoted form of destPath
    do shell script cmd with administrator privileges
end run"#;

    // SAFETY: getuid/getgid cannot fail and take no arguments.
    let uidgid = unsafe { format!("{}:{}", libc::getuid(), libc::getgid()) };

    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.arg("-e").arg(script).arg("--").arg(&dest).arg(uidgid);
    for p in paths {
        cmd.arg(p);
    }

    let out = cmd
        .output()
        .map_err(|e| format!("could not run osascript: {e}"))?;
    if out.status.success() {
        return Ok(dest);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    // Cancelling the password prompt is a choice, not a failure to report as one.
    if err.contains("-128") || err.to_lowercase().contains("user canceled") {
        let _ = std::fs::remove_dir(&dest);
        return Err("cancelled".into());
    }
    let _ = std::fs::remove_dir(&dest);
    Err(err.trim().to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn trash_elevated(_paths: &[PathBuf], _stamp: &str) -> Result<PathBuf, String> {
    // Windows: re-launch the removal helper with a UAC elevation prompt.
    // Linux: pkexec, or delegate to the package manager which handles its own
    // privilege escalation.
    Err("elevated removal is not implemented on this platform yet".into())
}

/// True when this path needs the elevated route.
pub fn needs_elevation(path: &Path) -> bool {
    crate::safety::requires_admin(path)
}
