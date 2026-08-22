//! Headless driver for the BHUninstaller engine.
//!
//! Its real job is verification: the engine's judgement about what belongs to
//! what has to be checked against a real machine's `~/Library`, and reading that
//! in a terminal is far quicker than clicking through a UI. It is also the only
//! way to exercise the engine on a server or in CI.

use bhu_core::discovery::ScanOptions;
use bhu_core::model::*;
use bhu_core::{discovery, removal};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let positional: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .collect();

    match positional.first().copied() {
        Some("list") | None => cmd_list(json),
        Some("info") => cmd_info(positional.get(1).copied(), json),
        Some("plan") => cmd_plan(positional.get(1).copied(), json),
        Some("orphans") => cmd_orphans(json),
        Some("history") => cmd_history(json),
        Some("restore") => cmd_restore(positional.get(1).copied()),
        Some("startup") => {
            cmd_startup(positional.get(1).copied(), positional.get(2).copied(), json)
        }
        Some("extensions") => cmd_extensions(json),
        Some("updates") => cmd_updates(json),
        Some("cleanup") => cmd_cleanup(json),
        Some("access") => cmd_access(json),
        Some("remove") => cmd_remove(positional.get(1).copied(), &args),
        Some(other) => {
            eprintln!("unknown command: {other}");
            usage();
            std::process::exit(2);
        }
    }
}

fn usage() {
    eprintln!(
        "bhu — BHUninstaller engine

  bhu list                 list installed applications
  bhu info <app>           detailed information about one app
  bhu plan <app>           show what uninstalling it would remove (dry run)
  bhu orphans              leftovers of apps that are no longer installed
  bhu history              past removals
  bhu restore <id>         put a past removal back where it came from
  bhu startup              things that run when you log in
  bhu startup on|off <id>  turn one of them on or off
  bhu extensions           browser extensions, plugins, panes, installers
  bhu remove <app> --yes   execute the plan (moves everything to the Trash)

  --json                   machine-readable output
  --sound                  play the Finder trash sound while removing
  --force                  sweep the files even if the app's own uninstaller fails
"
    );
}

/// Resolve a user-typed name to an app. Exact id, then exact name, then a
/// unique case-insensitive prefix — refusing when it is ambiguous rather than
/// picking one, since the next step deletes things.
fn resolve<'a>(apps: &'a [InstalledApp], query: &str) -> Option<&'a InstalledApp> {
    if let Some(a) = apps.iter().find(|a| a.id == query) {
        return Some(a);
    }
    if let Some(a) = apps.iter().find(|a| a.name.eq_ignore_ascii_case(query)) {
        return Some(a);
    }
    let q = query.to_lowercase();
    let matches: Vec<&InstalledApp> = apps
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&q))
        .collect();
    match matches.len() {
        1 => Some(matches[0]),
        0 => {
            eprintln!("no app matching \"{query}\"");
            None
        }
        _ => {
            eprintln!("\"{query}\" is ambiguous — it matches:");
            for m in matches {
                eprintln!("  {} ({})", m.name, m.id);
            }
            None
        }
    }
}

fn cmd_list(json: bool) {
    let apps = discovery::installed_apps(ScanOptions::default());
    if json {
        println!("{}", serde_json::to_string_pretty(&apps).unwrap());
        return;
    }
    println!("{} applications\n", apps.len());
    for a in &apps {
        println!(
            "{:<40} {:>12}  {}",
            truncate(&a.name, 40),
            human_size(a.size_bytes),
            a.bundle_id.as_deref().unwrap_or("-")
        );
    }
}

fn cmd_info(query: Option<&str>, json: bool) {
    let Some(query) = query else {
        usage();
        std::process::exit(2)
    };
    let apps = discovery::installed_apps(ScanOptions::default());
    let Some(app) = resolve(&apps, query) else {
        std::process::exit(1)
    };
    let mut app = app.clone();
    discovery::enrich(&mut app);
    // The icon is large and unreadable in a terminal.
    app.icon_png_base64 = app
        .icon_png_base64
        .map(|s| format!("<{} bytes of png>", s.len()));

    if json {
        println!("{}", serde_json::to_string_pretty(&app).unwrap());
        return;
    }
    println!("{}", app.name);
    println!("  Bundle id    {}", app.bundle_id.as_deref().unwrap_or("-"));
    println!("  Version      {}", app.version.as_deref().unwrap_or("-"));
    println!("  Developer    {}", app.publisher.as_deref().unwrap_or("-"));
    println!(
        "  Verification {}",
        match app.notarized {
            Some(true) => "Notarized",
            Some(false) => "Not notarized",
            None => "-",
        }
    );
    println!("  Size         {}", human_size(app.size_bytes));
    println!(
        "  Running      {}",
        if app.is_running { "yes" } else { "no" }
    );
    println!(
        "  Path         {}",
        app.path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
}

fn cmd_plan(query: Option<&str>, json: bool) {
    let Some(query) = query else {
        usage();
        std::process::exit(2)
    };
    let apps = discovery::installed_apps(ScanOptions::default());
    let Some(app) = resolve(&apps, query) else {
        std::process::exit(1)
    };
    let mut app = app.clone();
    discovery::enrich(&mut app);
    let plan = bhu_core::plan_uninstall(&app, &apps);

    if json {
        println!("{}", serde_json::to_string_pretty(&plan).unwrap());
        return;
    }
    print_plan(&plan);
}

fn print_plan(plan: &RemovalPlan) {
    if let Some(app) = &plan.app {
        println!("Uninstalling {}\n", app.name);
    }
    println!(
        "{} items found, {} selected ({} of {})\n",
        plan.items.len(),
        plan.selected_count(),
        human_size(plan.selected_bytes()),
        human_size(plan.total_bytes())
    );
    for item in &plan.items {
        println!(
            "  [{}] {:<8} {:>10}  {}",
            if item.selected { "x" } else { " " },
            format!("{:?}", item.confidence).to_lowercase(),
            if item.size_unknown {
                "unknown".to_string()
            } else {
                human_size(item.size_bytes)
            },
            item.path.display()
        );
        println!(
            "      {}{}",
            item.reason,
            if item.requires_admin {
                "  (needs admin)"
            } else {
                ""
            }
        );
    }
    if plan.needs_admin() {
        println!("\nSome selected items are outside your home folder and need an administrator password.");
    }
}

fn cmd_orphans(json: bool) {
    let groups = bhu_core::scan_orphans();
    if json {
        println!("{}", serde_json::to_string_pretty(&groups).unwrap());
        return;
    }
    let total: u64 = groups.iter().map(|g| g.size_bytes).sum();
    println!(
        "{} leftover groups from apps that are no longer installed, {} total\n",
        groups.len(),
        human_size(total)
    );
    for g in &groups {
        println!(
            "{:<45} {:>10}  {} item(s)",
            truncate(&g.name, 45),
            human_size(g.size_bytes),
            g.items.len()
        );
        for i in &g.items {
            println!("      {}", i.path.display());
        }
    }
}

fn cmd_history(json: bool) {
    let entries = bhu_core::undo::history();
    if json {
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        return;
    }
    if entries.is_empty() {
        println!("No removals recorded yet.");
        return;
    }
    for e in entries {
        let items = e.all_items();
        println!(
            "{:<22}  {:<28} {:>10}  {} item(s), {} restorable",
            e.id,
            truncate(e.app_name.as_deref().unwrap_or("(leftovers)"), 28),
            human_size(e.bytes_freed),
            items.len(),
            e.restorable_count()
        );
    }
    println!("\nPut one back with:  bhu restore <id>");
}

fn cmd_startup(action: Option<&str>, id: Option<&str>, json: bool) {
    use bhu_core::startup;
    let items = startup::list();

    match action {
        Some(verb @ ("on" | "off")) => {
            let Some(id) = id else {
                usage();
                std::process::exit(2)
            };
            let Some(item) = items.iter().find(|i| i.id == id) else {
                eprintln!("no startup item with id \"{id}\"");
                std::process::exit(1);
            };
            match startup::set_enabled(item, verb == "on") {
                Ok(()) => println!(
                    "{} is now {}",
                    item.name,
                    if verb == "on" { "enabled" } else { "disabled" }
                ),
                Err(e) => {
                    eprintln!("could not change {}: {e}", item.name);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            if json {
                println!("{}", serde_json::to_string_pretty(&items).unwrap());
                return;
            }
            println!("{} startup item(s)\n", items.len());
            for i in &items {
                println!(
                    "  [{}] {:<34} {:<16} {}",
                    if i.enabled { "on " } else { "off" },
                    truncate(&i.name, 34),
                    i.kind.label(),
                    i.id
                );
                if let Some(p) = &i.program {
                    println!(
                        "      {}{}",
                        p,
                        if i.requires_admin {
                            "  (needs admin)"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
    }
}

fn cmd_access(json: bool) {
    let r = bhu_core::access::report();
    if json {
        println!("{}", serde_json::to_string_pretty(&r).unwrap());
        return;
    }
    println!(
        "Full Disk Access: {}\n{} of {} probed location(s) unreadable\n",
        if r.granted { "granted" } else { "not granted" },
        r.blocked.len(),
        r.checked
    );
    for b in &r.blocked {
        println!("  {}", b.path.display());
        println!("      {}", b.consequence);
    }
}

fn cmd_cleanup(json: bool) {
    let groups = bhu_core::cleaner::scan();
    if json {
        println!("{}", serde_json::to_string_pretty(&groups).unwrap());
        return;
    }
    let reclaimable: u64 = groups
        .iter()
        .filter(|g| g.removable)
        .map(|g| g.size_bytes)
        .sum();
    println!("{} reclaimable\n", human_size(reclaimable));
    for g in &groups {
        println!(
            "{:<24} {:>4} item(s) {:>10}{}",
            g.label,
            g.items.len(),
            human_size(g.size_bytes),
            if g.removable { "" } else { "  (reported only)" }
        );
        for i in g.items.iter().take(5) {
            println!(
                "      {:<44} {:>10}",
                truncate(&i.name, 44),
                human_size(i.size_bytes)
            );
        }
        if g.items.len() > 5 {
            println!("      … and {} more", g.items.len() - 5);
        }
    }
}

fn cmd_updates(json: bool) {
    let apps = discovery::installed_apps(ScanOptions {
        compute_sizes: false,
        include_system: false,
    });
    let found = bhu_core::updates::check(&apps);
    if json {
        println!("{}", serde_json::to_string_pretty(&found).unwrap());
        return;
    }
    let outdated: Vec<_> = found.iter().filter(|u| u.outdated).collect();
    println!(
        "{} of {} app(s) have a newer version available; {} had no version source at all\n",
        outdated.len(),
        found.len(),
        apps.len() - found.len()
    );
    for u in &found {
        println!(
            "  {} {:<30} {:>14} -> {:<14} {}",
            if u.outdated { "!" } else { " " },
            truncate(&u.name, 30),
            u.current_version.as_deref().unwrap_or("?"),
            u.latest_version,
            u.source.label()
        );
    }
}

fn cmd_extensions(json: bool) {
    let groups = bhu_core::extensions::list();
    if json {
        println!("{}", serde_json::to_string_pretty(&groups).unwrap());
        return;
    }
    for g in &groups {
        println!(
            "{:<26} {:>4} item(s)  {:>10}",
            g.label,
            g.items.len(),
            human_size(g.size_bytes)
        );
        for i in g.items.iter().take(6) {
            println!(
                "      {:<44} {:>10}",
                truncate(&i.name, 44),
                human_size(i.size_bytes)
            );
        }
        if g.items.len() > 6 {
            println!("      … and {} more", g.items.len() - 6);
        }
    }
}

fn cmd_restore(id: Option<&str>) {
    let Some(id) = id else {
        usage();
        std::process::exit(2)
    };
    let outcomes = bhu_core::undo::restore(id);
    if outcomes.is_empty() {
        eprintln!("no removal with id \"{id}\" — run `bhu history` to see them");
        std::process::exit(1);
    }
    let ok = outcomes.iter().filter(|o| o.restored).count();
    println!("Put back {ok} of {} item(s).", outcomes.len());
    for o in outcomes.iter().filter(|o| !o.restored) {
        println!(
            "  {}: {}",
            o.original.display(),
            o.error.as_deref().unwrap_or("unknown")
        );
    }
}

fn cmd_remove(query: Option<&str>, args: &[String]) {
    let Some(query) = query else {
        usage();
        std::process::exit(2)
    };
    let apps = discovery::installed_apps(ScanOptions::default());
    let Some(app) = resolve(&apps, query) else {
        std::process::exit(1)
    };
    let mut app = app.clone();
    discovery::enrich(&mut app);
    let plan = bhu_core::plan_uninstall(&app, &apps);
    print_plan(&plan);

    if !args.iter().any(|a| a == "--yes") {
        println!("\nDry run. Nothing was removed. Add --yes to carry this out.");
        return;
    }
    if app.is_running {
        eprintln!("\n{} is running. Quit it first.", app.name);
        std::process::exit(1);
    }

    let opts = RemovalOptions {
        sound: args.iter().any(|a| a == "--sound") || bhu_core::settings::load().removal_sound,
        force: args.iter().any(|a| a == "--force"),
    };
    let report = removal::execute(&plan, opts);
    println!(
        "\nMoved {} item(s) to the Trash, freeing {}.",
        report.removed_count(),
        human_size(report.bytes_freed)
    );
    for f in report.failed() {
        println!(
            "  could not remove {}: {}",
            f.path.display(),
            f.error.as_deref().unwrap_or("unknown")
        );
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}
