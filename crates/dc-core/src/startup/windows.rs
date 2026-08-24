//! Windows startup items.
//!
//! Compile-verified for the Windows target, not yet run on Windows.
//!
//! Disabling goes through `StartupApproved`, the same mechanism Task Manager
//! uses. The app's own `Run` value is left untouched — deleting it would stop
//! the program starting but could never be undone, and this section promises a
//! switch, not a removal.

use super::{StartupItem, StartupKind};
use crate::fsutil;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::{RegKey, HKEY};

const RUN: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_RUN: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// The first byte of a StartupApproved value carries the state: an even value
/// means enabled, an odd one means the user turned it off.
fn is_disabled(hive: HKEY, name: &str) -> bool {
    let root = RegKey::predef(hive);
    let Ok(key) = root.open_subkey(APPROVED_RUN) else {
        return false;
    };
    match key.get_raw_value(name) {
        Ok(value) => value.bytes.first().map(|b| b % 2 == 1).unwrap_or(false),
        Err(_) => false,
    }
}

fn set_disabled(hive: HKEY, name: &str, disabled: bool) -> Result<(), String> {
    let root = RegKey::predef(hive);
    let (key, _) = root
        .create_subkey(APPROVED_RUN)
        .map_err(|e| format!("could not open the startup approval key: {e}"))?;

    // Twelve bytes: a state byte, then a timestamp Windows fills in. Writing
    // zeroes for the timestamp is what Task Manager itself does for an entry
    // it has never seen before.
    let mut bytes = vec![0u8; 12];
    bytes[0] = if disabled { 3 } else { 2 };

    let value = winreg::RegValue {
        vtype: REG_BINARY,
        bytes,
    };
    key.set_raw_value(name, &value)
        .map_err(|e| format!("could not write the startup approval key: {e}"))
}

pub fn list() -> Vec<StartupItem> {
    let mut items = Vec::new();

    for (hive, hive_name, path, admin) in [
        (HKEY_CURRENT_USER, "HKCU", RUN, false),
        (HKEY_LOCAL_MACHINE, "HKLM", RUN, true),
        (HKEY_LOCAL_MACHINE, "HKLM", RUN_WOW, true),
    ] {
        let root = RegKey::predef(hive);
        let Ok(key) = root.open_subkey(path) else {
            continue;
        };
        for (name, value) in key.enum_values().flatten() {
            let program = value.to_string();
            items.push(StartupItem {
                id: format!("{hive_name}:{name}"),
                name: name.clone(),
                kind: StartupKind::RegistryRun,
                path: Some(PathBuf::from(format!(r"{hive_name}\{path}"))),
                program: Some(program),
                enabled: !is_disabled(hive, &name),
                can_toggle: true,
                locked_reason: None,
                requires_admin: admin,
                app_id: None,
            });
        }
    }

    // Shortcuts dropped in a Startup folder. There is no approval key for
    // these — the only way to stop one is to move the shortcut, which this
    // section does not do, so they are listed as managed.
    let mut startup_dirs: Vec<PathBuf> = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        startup_dirs
            .push(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup"));
    }
    if let Some(programdata) = std::env::var_os("PROGRAMDATA") {
        startup_dirs.push(
            PathBuf::from(programdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup"),
        );
    }
    for dir in startup_dirs {
        for path in fsutil::children(&dir) {
            let Some(name) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            if name.starts_with("desktop") {
                continue;
            }
            items.push(StartupItem {
                id: format!("startupfolder:{}", path.display()),
                name,
                kind: StartupKind::RegistryRun,
                program: Some(path.to_string_lossy().to_string()),
                path: Some(path),
                enabled: true,
                can_toggle: false,
                locked_reason: Some(
                    "This is a shortcut in your Startup folder. Remove it from there to stop \
                     it running — there is no switch for these."
                        .into(),
                ),
                requires_admin: false,
                app_id: None,
            });
        }
    }

    items
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<(), String> {
    let (hive_name, name) = item
        .id
        .split_once(':')
        .ok_or("this startup item cannot be changed")?;
    let hive = match hive_name {
        "HKCU" => HKEY_CURRENT_USER,
        "HKLM" => HKEY_LOCAL_MACHINE,
        _ => return Err("this startup item cannot be changed".into()),
    };
    set_disabled(hive, name, !enabled)
}
