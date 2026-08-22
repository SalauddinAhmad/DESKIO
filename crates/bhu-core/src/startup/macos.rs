//! macOS startup items.
//!
//! ## How disabling works, and why it is done this way
//!
//! An agent could be stopped by deleting its `.plist`, or by editing a
//! `Disabled` key into it. Both mean writing to — or destroying — a file the
//! app's own installer owns, and the second is quietly ignored by modern
//! launchd anyway.
//!
//! Instead the engine uses launchd's own override database via
//! `launchctl enable` / `launchctl disable`. Nothing the user installed is
//! touched, the state survives reboots, and re-enabling is exact.

use super::{StartupItem, StartupKind};
use crate::fsutil;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn uid() -> u32 {
    // SAFETY: getuid cannot fail and takes no arguments.
    unsafe { libc::getuid() }
}

fn roots() -> Vec<(PathBuf, StartupKind)> {
    let mut v = vec![
        (
            PathBuf::from("/Library/LaunchAgents"),
            StartupKind::LaunchAgent,
        ),
        (
            PathBuf::from("/Library/LaunchDaemons"),
            StartupKind::LaunchDaemon,
        ),
    ];
    if let Some(home) = dirs::home_dir() {
        v.push((home.join("Library/LaunchAgents"), StartupKind::LaunchAgent));
    }
    v
}

/// Labels launchd has been told not to run, per domain.
fn disabled_labels(domain: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(res) = Command::new("/bin/launchctl")
        .args(["print-disabled", domain])
        .output()
    else {
        return out;
    };
    // The wording has changed across releases: older systems print
    //   "com.example.agent" => true
    // and macOS 26/27 print
    //   "com.example.agent" => disabled
    // Accept both, or the switch silently reports the wrong position.
    for line in String::from_utf8_lossy(&res.stdout).lines() {
        let line = line.trim();
        let Some((label, state)) = line.split_once("=>") else {
            continue;
        };
        let state = state.trim().trim_end_matches(';').trim();
        if state.eq_ignore_ascii_case("true") || state.eq_ignore_ascii_case("disabled") {
            out.insert(label.trim().trim_matches('"').to_string());
        }
    }
    out
}

pub fn list() -> Vec<StartupItem> {
    let user_disabled = disabled_labels(&format!("gui/{}", uid()));
    let system_disabled = disabled_labels("system");

    let mut items: Vec<StartupItem> = Vec::new();

    for (root, kind) in roots() {
        for path in fsutil::children(&root) {
            if path.extension().and_then(|e| e.to_str()) != Some("plist") {
                continue;
            }
            let Some(item) = read_job(&path, kind, &user_disabled, &system_disabled) else {
                continue;
            };
            // Apple's own agents are not the user's to manage, and turning one
            // off can break the system in ways that are hard to diagnose.
            if item.id.starts_with("com.apple.") {
                continue;
            }
            if items.iter().any(|i| i.id == item.id && i.kind == item.kind) {
                continue;
            }
            items.push(item);
        }
    }

    items.extend(login_items());
    items
}

fn read_job(
    path: &Path,
    kind: StartupKind,
    user_disabled: &HashSet<String>,
    system_disabled: &HashSet<String>,
) -> Option<StartupItem> {
    let dict = plist::Value::from_file(path).ok()?.into_dictionary()?;

    let label = dict
        .get("Label")
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()))?;

    // The binary it runs: `Program`, else the first of `ProgramArguments`.
    let program = dict
        .get("Program")
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .or_else(|| {
            dict.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_string())
                .map(str::to_string)
        });

    let disabled_in_db = match kind {
        StartupKind::LaunchDaemon => system_disabled.contains(&label),
        _ => user_disabled.contains(&label),
    };
    // A `Disabled` key in the file itself is the older mechanism; respect it
    // when reporting state so the switch matches reality.
    let disabled_in_file = dict
        .get("Disabled")
        .and_then(|v| v.as_boolean())
        .unwrap_or(false);

    let requires_admin = kind == StartupKind::LaunchDaemon || crate::safety::requires_admin(path);

    Some(StartupItem {
        name: friendly_name(&label, program.as_deref()),
        id: label,
        kind,
        path: Some(path.to_path_buf()),
        program,
        enabled: !(disabled_in_db || disabled_in_file),
        can_toggle: true,
        locked_reason: None,
        requires_admin,
        app_id: None,
    })
}

/// A label like `com.microsoft.autoupdate.helper` reads better as its last
/// meaningful component when we have nothing better, but the binary's own name
/// is usually clearer still.
fn friendly_name(label: &str, program: Option<&str>) -> String {
    if let Some(p) = program {
        // `/Applications/Foo.app/Contents/MacOS/Foo` -> `Foo`
        if let Some(app) = p.split("/Contents/").next() {
            if app.ends_with(".app") {
                if let Some(stem) = Path::new(app).file_stem() {
                    return stem.to_string_lossy().to_string();
                }
            }
        }
    }
    label.to_string()
}

/// Login items, read through System Events.
///
/// This is the only supported way to see them, and it needs Automation
/// permission. If that is refused the list is simply empty — the rest of the
/// section still works, and nothing here is important enough to block on.
fn login_items() -> Vec<StartupItem> {
    let script = r#"tell application "System Events"
    set out to ""
    repeat with li in login items
        set out to out & (name of li) & "\t" & (path of li) & "\t" & (hidden of li) & "\n"
    end repeat
    return out
end tell"#;

    let Ok(res) = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output()
    else {
        return Vec::new();
    };
    if !res.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&res.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let path = parts.next().unwrap_or("").trim().to_string();
            Some(StartupItem {
                id: format!("loginitem:{name}"),
                name,
                kind: StartupKind::LoginItem,
                path: (!path.is_empty()).then(|| PathBuf::from(&path)),
                program: (!path.is_empty()).then_some(path),
                enabled: true,
                // A login item has no off switch — it is either in the list or
                // not. Removing one is offered separately rather than dressed
                // up as a toggle that cannot be undone by flicking it back.
                can_toggle: false,
                locked_reason: Some(
                    "Login items can only be added or removed, not switched off. \
                     Use Remove, or manage them in System Settings › General › Login Items."
                        .into(),
                ),
                requires_admin: false,
                app_id: None,
            })
        })
        .collect()
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<(), String> {
    let domain = match item.kind {
        StartupKind::LaunchDaemon => "system".to_string(),
        _ => format!("gui/{}", uid()),
    };
    let target = format!("{domain}/{}", item.id);
    let verb = if enabled { "enable" } else { "disable" };

    if item.requires_admin {
        return set_enabled_elevated(verb, &target, item, enabled);
    }

    let out = Command::new("/bin/launchctl")
        .args([verb, &target])
        .output()
        .map_err(|e| format!("could not run launchctl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("launchctl {verb} failed")
        } else {
            err
        });
    }

    // Best effort: make it take effect now rather than at the next login. A
    // failure here is not reported — the persistent state above is what the
    // switch actually promises.
    if let Some(path) = &item.path {
        let _ = if enabled {
            Command::new("/bin/launchctl")
                .arg("bootstrap")
                .arg(&domain)
                .arg(path)
                .output()
        } else {
            Command::new("/bin/launchctl")
                .args(["bootout", &target])
                .output()
        };
    }
    Ok(())
}

/// Daemons live in the system domain, so changing one needs a password.
fn set_enabled_elevated(
    verb: &str,
    target: &str,
    item: &StartupItem,
    enabled: bool,
) -> Result<(), String> {
    let script = r#"on run argv
    set cmd to "/bin/launchctl " & quoted form of (item 1 of argv) & " " & quoted form of (item 2 of argv)
    if (count of argv) > 2 then
        if (item 1 of argv) is "enable" then
            set cmd to cmd & " ; /bin/launchctl bootstrap system " & quoted form of (item 3 of argv)
        else
            set cmd to cmd & " ; /bin/launchctl bootout " & quoted form of (item 2 of argv)
        end if
    end if
    do shell script cmd with administrator privileges
end run"#;

    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.arg("-e").arg(script).arg("--").arg(verb).arg(target);
    if let Some(path) = &item.path {
        cmd.arg(path);
    }
    let _ = enabled;

    let out = cmd
        .output()
        .map_err(|e| format!("could not run osascript: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("-128") || err.to_lowercase().contains("user canceled") {
        return Err("cancelled".into());
    }
    Err(err.trim().to_string())
}
