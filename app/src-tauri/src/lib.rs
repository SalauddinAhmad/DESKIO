//! Tauri host for BHUninstaller.
//!
//! Every command here is a thin wrapper over `bhu-core`. No scanning logic, no
//! safety rules and no matching live in this crate — that all belongs to the
//! engine, so that the Windows and Linux builds get identical behaviour from
//! the same code.

use bhu_core::access::AccessReport;
use bhu_core::cleaner::JunkGroup;
use bhu_core::extensions::ExtensionGroup;
use bhu_core::leftovers::OrphanGroup;
use bhu_core::model::*;
use bhu_core::selfupdate::{CheckResult, Release};
use bhu_core::settings::Settings;
use bhu_core::startup::StartupItem;
use bhu_core::undo::{RestoreOutcome, UndoEntry};
use bhu_core::updates::UpdateInfo;
use bhu_core::{discovery, leftovers, removal};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/// Scans are expensive, so the results are kept for the lifetime of the window.
/// The app list in particular is needed by every leftover scan, since knowing
/// what else is installed is what stops one app claiming another's files.
#[derive(Default)]
struct Cache {
    apps: Mutex<Vec<InstalledApp>>,
    orphans: Mutex<Vec<OrphanGroup>>,
    startup: Mutex<Vec<StartupItem>>,
    extensions: Mutex<Vec<ExtensionGroup>>,
    updates: Mutex<Vec<UpdateInfo>>,
    junk: Mutex<Vec<JunkGroup>>,
    /// Every path the backend has offered up in a plan.
    ///
    /// The interface can deselect items but never invent them, so a path
    /// arriving at `execute_plan` that was never offered did not come from a
    /// plan this app produced. The engine's blocklist would still refuse
    /// system and personal paths, but it would happily remove any ordinary
    /// file — so the plan is checked against what was actually offered rather
    /// than taken on trust.
    offered: Mutex<HashSet<PathBuf>>,
}

impl Cache {
    /// Record what a plan put in front of the user.
    fn offer(&self, plan: &RemovalPlan) {
        let mut offered = self.offered.lock().unwrap();
        offered.clear();
        for item in &plan.items {
            offered.insert(item.path.clone());
        }
    }
}

#[tauri::command]
fn list_apps(refresh: bool, cache: State<Cache>) -> Vec<InstalledApp> {
    let mut apps = cache.apps.lock().unwrap();
    if apps.is_empty() || refresh {
        *apps = discovery::installed_apps(discovery::ScanOptions::default());
    }
    apps.clone()
}

/// Icons for the list, fetched after the first paint so the window appears
/// immediately rather than waiting on one subprocess per app.
#[tauri::command]
fn app_icons(cache: State<Cache>) -> HashMap<String, String> {
    // Scan first if this somehow arrives before the list — an empty result here
    // would leave every row showing a placeholder with no way to recover.
    let apps = {
        let mut guard = cache.apps.lock().unwrap();
        if guard.is_empty() {
            *guard = discovery::installed_apps(discovery::ScanOptions::default());
        }
        guard.clone()
    };
    discovery::icons(&apps)
}

/// The expensive per-app details for the detail pane: developer, notarisation,
/// last opened, whether it is running.
#[tauri::command]
fn app_details(id: String, cache: State<Cache>) -> Option<InstalledApp> {
    let app = cache
        .apps
        .lock()
        .unwrap()
        .iter()
        .find(|a| a.id == id)
        .cloned()?;
    let mut app = app;
    discovery::enrich(&mut app);
    Some(app)
}

/// The dry run: what uninstalling this app would remove.
#[tauri::command]
fn plan_uninstall(id: String, cache: State<Cache>) -> Option<RemovalPlan> {
    let apps = cache.apps.lock().unwrap().clone();
    let app = apps.iter().find(|a| a.id == id)?.clone();
    let mut app = app;
    discovery::enrich(&mut app);
    // Goes through the engine's own planner rather than assembling a plan here:
    // that is what attaches the platform's uninstall command. Building it
    // by hand meant Windows never ran the vendor's uninstaller and tried to
    // delete Program Files itself instead.
    let plan = bhu_core::plan_uninstall(&app, &apps);
    cache.offer(&plan);
    Some(plan)
}

#[tauri::command]
fn orphan_groups(refresh: bool, cache: State<Cache>) -> Vec<OrphanGroup> {
    let mut groups = cache.orphans.lock().unwrap();
    if groups.is_empty() || refresh {
        *groups = bhu_core::scan_orphans();
    }
    groups.clone()
}

/// A plan for the selected orphan groups.
#[tauri::command]
fn plan_orphans(names: Vec<String>, cache: State<Cache>) -> RemovalPlan {
    let groups = cache.orphans.lock().unwrap().clone();
    let items: Vec<Leftover> = groups
        .into_iter()
        .filter(|g| names.contains(&g.name))
        .flat_map(|g| g.items)
        .collect();
    let mut plan = removal::build_orphan_plan(items);
    // The user picked these groups deliberately, so start with everything
    // ticked — but they still see the full list before anything moves.
    for item in plan.items.iter_mut() {
        item.selected = true;
    }
    cache.offer(&plan);
    plan
}

/// Carry out a plan. Everything goes to the Trash.
#[tauri::command]
fn execute_plan(plan: RemovalPlan, force: bool, cache: State<Cache>) -> RemovalReport {
    // Nothing is removed that this app did not itself put in front of the user.
    let offered = cache.offered.lock().unwrap().clone();
    let mut plan = plan;
    let mut refused: Vec<RemovalOutcome> = Vec::new();
    plan.items.retain(|item| {
        if !item.selected || offered.contains(&item.path) {
            return true;
        }
        refused.push(RemovalOutcome {
            path: item.path.clone(),
            removed: false,
            trashed_to: None,
            error: Some("this was not part of the plan you were shown — refusing".into()),
        });
        false
    });

    let opts = RemovalOptions {
        sound: bhu_core::settings::load().removal_sound,
        force,
    };
    let mut report = removal::execute(&plan, opts);
    report.outcomes.extend(refused);
    // Whatever just moved is gone from the disk; the caches must not keep
    // claiming otherwise.
    cache.apps.lock().unwrap().clear();
    cache.orphans.lock().unwrap().clear();
    cache.extensions.lock().unwrap().clear();
    cache.startup.lock().unwrap().clear();
    cache.updates.lock().unwrap().clear();
    cache.junk.lock().unwrap().clear();
    report
}

#[tauri::command]
fn startup_items(refresh: bool, cache: State<Cache>) -> Vec<StartupItem> {
    let mut items = cache.startup.lock().unwrap();
    if items.is_empty() || refresh {
        *items = bhu_core::startup::list();
    }
    items.clone()
}

/// Turn a startup item on or off. Always reversible — nothing is deleted.
#[tauri::command]
fn set_startup_enabled(id: String, enabled: bool, cache: State<Cache>) -> Result<(), String> {
    let item = cache
        .startup
        .lock()
        .unwrap()
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .ok_or("that startup item is no longer there")?;
    bhu_core::startup::set_enabled(&item, enabled)?;
    // Re-read rather than assuming the change took: launchd is the source of
    // truth for this, not us.
    cache.startup.lock().unwrap().clear();
    Ok(())
}

#[tauri::command]
fn extension_groups(refresh: bool, cache: State<Cache>) -> Vec<ExtensionGroup> {
    let mut groups = cache.extensions.lock().unwrap();
    if groups.is_empty() || refresh {
        *groups = bhu_core::extensions::list();
    }
    groups.clone()
}

/// A plan for the selected extensions. Everything the user ticked arrives
/// pre-selected, but they still see the full list with paths before it runs.
#[tauri::command]
fn plan_extensions(ids: Vec<String>, cache: State<Cache>) -> RemovalPlan {
    let groups = cache.extensions.lock().unwrap().clone();
    let items: Vec<Leftover> = groups
        .iter()
        .flat_map(|g| g.items.iter())
        .filter(|i| ids.contains(&i.id))
        .map(|i| i.to_leftover())
        .collect();
    let mut plan = removal::build_orphan_plan(items);
    for item in plan.items.iter_mut() {
        item.selected = true;
    }
    plan
}

#[tauri::command]
fn junk_groups(refresh: bool, cache: State<Cache>) -> Vec<JunkGroup> {
    let mut groups = cache.junk.lock().unwrap();
    if groups.is_empty() || refresh {
        *groups = bhu_core::cleaner::scan();
    }
    groups.clone()
}

/// A plan for the selected junk. Reported-only categories are filtered out
/// here as well as in the UI — the Trash must not become removable because
/// something sent the wrong id.
#[tauri::command]
fn plan_cleanup(ids: Vec<String>, cache: State<Cache>) -> RemovalPlan {
    let groups = cache.junk.lock().unwrap().clone();
    let items: Vec<Leftover> = groups
        .iter()
        .filter(|g| g.removable)
        .flat_map(|g| g.items.iter())
        .filter(|i| ids.contains(&i.id))
        .map(|i| i.to_leftover())
        .collect();
    let mut plan = removal::build_orphan_plan(items);
    for item in plan.items.iter_mut() {
        item.selected = true;
    }
    cache.offer(&plan);
    plan
}

/// Look up newer versions. The only command in the app that touches the
/// network, and only ever to public endpoints — see `bhu_core::updates`.
#[tauri::command]
fn check_updates(refresh: bool, cache: State<Cache>) -> Vec<UpdateInfo> {
    {
        let cached = cache.updates.lock().unwrap();
        if !cached.is_empty() && !refresh {
            return cached.clone();
        }
    }
    let apps = {
        let mut guard = cache.apps.lock().unwrap();
        if guard.is_empty() {
            *guard = discovery::installed_apps(discovery::ScanOptions::default());
        }
        guard.clone()
    };
    let found = bhu_core::updates::check(&apps);
    *cache.updates.lock().unwrap() = found.clone();
    found
}

#[tauri::command]
fn removal_history() -> Vec<UndoEntry> {
    bhu_core::undo::history()
}

/// Put a past removal back where it came from.
#[tauri::command]
fn restore_removal(id: String, cache: State<Cache>) -> Vec<RestoreOutcome> {
    let outcomes = bhu_core::undo::restore(&id);
    if outcomes.iter().any(|o| o.restored) {
        cache.apps.lock().unwrap().clear();
        cache.orphans.lock().unwrap().clear();
    }
    outcomes
}

#[tauri::command]
fn get_settings() -> Settings {
    bhu_core::settings::load()
}

#[tauri::command]
fn set_settings(settings: Settings) -> Result<(), String> {
    bhu_core::settings::save(&settings)
}

/// Whether the operating system is withholding anything, and what.
#[tauri::command]
fn access_report() -> AccessReport {
    bhu_core::access::report()
}

#[tauri::command]
fn full_disk_access() -> bool {
    bhu_core::access::report().granted
}

/// Relaunch, so a newly granted permission takes effect.
///
/// macOS decides Full Disk Access per process: a grant made while the app is
/// running does not reach the running process, which is why the system itself
/// offers "Quit & Reopen". Doing it from here saves the user working that out.
#[tauri::command]
fn relaunch(app: tauri::AppHandle) {
    app.restart();
}

/// Whether this platform can restore from the trash without the user going to
/// the file manager. The UI must not offer a button that does nothing.
#[tauri::command]
fn can_restore() -> bool {
    bhu_core::trash_bin::can_restore_programmatically()
}

/// Look for a newer BHUninstaller.
///
/// `force` is a manual check and always runs. Without it the check only
/// happens when enabled and when a day has passed.
#[tauri::command]
fn check_app_update(force: bool) -> CheckResult {
    let mut settings = bhu_core::settings::load();
    if !force
        && (!settings.auto_check_updates
            || !bhu_core::selfupdate::check_due(settings.last_update_check))
    {
        return CheckResult {
            update: None,
            current: bhu_core::VERSION.to_string(),
            error: None,
        };
    }

    let result = bhu_core::selfupdate::check();
    // Only a check that actually completed resets the clock, so a spell offline
    // does not postpone the next attempt by a day.
    if result.error.is_none() {
        settings.last_update_check = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let _ = bhu_core::settings::save(&settings);
    }
    result
}

/// Download the update and return where it was saved. Nothing is run.
#[tauri::command]
fn download_app_update(release: Release) -> Result<String, String> {
    bhu_core::selfupdate::download(&release).map(|p| p.to_string_lossy().to_string())
}

/// Start the downloaded installer and close BHUninstaller.
///
/// The app has to be gone before an installer can replace it — on Windows the
/// installer simply refuses while it is running — so this launches it and then
/// exits rather than leaving the user to work that out.
///
/// The path is not taken on trust even though it came from `download_app_update`
/// a moment earlier: it must still be inside the download directory this
/// process created, and carry an installer extension. Running an arbitrary
/// path handed over from the interface is not something to leave open.
#[tauri::command]
fn install_update(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    let expected_parent =
        std::env::temp_dir().join(format!("BHUninstaller-update-{}", std::process::id()));
    if path.parent() != Some(expected_parent.as_path()) {
        return Err("that file was not downloaded by this app — refusing to run it".into());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "dmg" | "pkg" | "msi" | "exe" | "deb" | "rpm") {
        return Err(format!("{ext} is not an installer — refusing to run it"));
    }
    if !path.is_file() {
        return Err("the download is no longer there".into());
    }

    let mut command = installer_command(&path, &ext);
    command
        .spawn()
        .map_err(|e| format!("could not start the installer: {e}"))?;

    // Give it a moment to appear before this window disappears, so the user
    // does not watch the app vanish with nothing yet on screen.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        handle.exit(0);
    });
    Ok(())
}

fn installer_command(path: &std::path::Path, ext: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        // An .msi is data, not a program: it needs msiexec to run it.
        if ext == "msi" {
            let mut c = std::process::Command::new("msiexec");
            c.arg("/i").arg(path);
            return c;
        }
        return std::process::Command::new(path);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = ext;
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg(path);
        c
    }
    #[cfg(target_os = "linux")]
    {
        let _ = ext;
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    }
}

#[tauri::command]
fn engine_version() -> String {
    bhu_core::VERSION.to_string()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Cache::default())
        .invoke_handler(tauri::generate_handler![
            list_apps,
            app_icons,
            app_details,
            plan_uninstall,
            orphan_groups,
            plan_orphans,
            execute_plan,
            startup_items,
            set_startup_enabled,
            extension_groups,
            check_updates,
            junk_groups,
            plan_cleanup,
            plan_extensions,
            removal_history,
            restore_removal,
            get_settings,
            set_settings,
            full_disk_access,
            access_report,
            relaunch,
            can_restore,
            engine_version,
            check_app_update,
            download_app_update,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running BHUninstaller");
}
