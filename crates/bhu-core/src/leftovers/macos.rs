//! Where macOS apps leave things.

use super::{
    classify, classify_in_vendor_dir, is_vendor_dir, resolve_sharing, AppTokens, OrphanGroup,
};
use crate::fsutil;
use crate::model::*;
use crate::safety;
use std::path::{Path, PathBuf};

/// Every directory an app is likely to have written to, with the kind of thing
/// found there. Roots that do not exist are skipped silently.
fn roots() -> Vec<(PathBuf, LeftoverKind)> {
    let mut v: Vec<(PathBuf, LeftoverKind)> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let lib = home.join("Library");
        v.extend([
            (lib.join("Preferences"), LeftoverKind::Preferences),
            (lib.join("Preferences/ByHost"), LeftoverKind::Preferences),
            (lib.join("Caches"), LeftoverKind::Caches),
            (
                lib.join("Application Support"),
                LeftoverKind::ApplicationSupport,
            ),
            (
                lib.join("Application Support/CrashReporter"),
                LeftoverKind::CrashReport,
            ),
            (lib.join("Containers"), LeftoverKind::Container),
            (lib.join("Group Containers"), LeftoverKind::GroupContainer),
            (lib.join("Application Scripts"), LeftoverKind::Container),
            (lib.join("Logs"), LeftoverKind::Logs),
            (
                lib.join("Logs/DiagnosticReports"),
                LeftoverKind::CrashReport,
            ),
            (
                lib.join("Saved Application State"),
                LeftoverKind::SavedState,
            ),
            (lib.join("LaunchAgents"), LeftoverKind::LaunchAgent),
            (lib.join("WebKit"), LeftoverKind::WebData),
            (lib.join("HTTPStorages"), LeftoverKind::WebData),
            (lib.join("Cookies"), LeftoverKind::Cookies),
            (lib.join("Internet Plug-Ins"), LeftoverKind::Extension),
            (lib.join("PreferencePanes"), LeftoverKind::Extension),
            (lib.join("Screen Savers"), LeftoverKind::Extension),
            (lib.join("Services"), LeftoverKind::Extension),
        ]);
    }

    // System-wide locations. Everything here needs an admin prompt to remove.
    let sys = Path::new("/Library");
    v.extend([
        (
            sys.join("Application Support"),
            LeftoverKind::ApplicationSupport,
        ),
        (sys.join("Caches"), LeftoverKind::Caches),
        (sys.join("Preferences"), LeftoverKind::Preferences),
        (sys.join("Logs"), LeftoverKind::Logs),
        (sys.join("LaunchAgents"), LeftoverKind::LaunchAgent),
        (sys.join("LaunchDaemons"), LeftoverKind::LaunchDaemon),
        (
            sys.join("PrivilegedHelperTools"),
            LeftoverKind::PrivilegedHelper,
        ),
        (sys.join("Extensions"), LeftoverKind::Extension),
        (sys.join("Internet Plug-Ins"), LeftoverKind::Extension),
        (sys.join("PreferencePanes"), LeftoverKind::Extension),
        (sys.join("Screen Savers"), LeftoverKind::Extension),
        (
            PathBuf::from("/private/var/db/receipts"),
            LeftoverKind::Receipt,
        ),
    ]);

    v
}

/// Names that live in these roots but belong to macOS itself, not to any app
/// the user installed. Never offered for removal.
fn is_system_owned(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("com.apple.")
        || lower.starts_with(".")
        || matches!(
            lower.as_str(),
            "crashreporter"
                | "mobilesync"
                | "addressbook"
                | "syncservices"
                | "coresimulator"
                | "icdd"
                | "knowledge"
                | "caches"
                | "clouddocs"
                | "accountsd"
        )
}

/// Walk every root once, collecting whatever the given tokens match.
fn collect(tokens: &AppTokens) -> (Vec<Leftover>, Vec<String>) {
    let mut found: Vec<Leftover> = Vec::new();
    let mut names: Vec<String> = Vec::new();

    for (root, kind) in roots() {
        for path in fsutil::children(&root) {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if is_system_owned(&name) {
                continue;
            }
            // Never surface something the safety layer would refuse anyway —
            // showing a user an item that cannot be removed is just noise.
            if safety::check_removable(&path).is_err() {
                continue;
            }
            let Some(m) = classify(&name, tokens) else {
                // Not the app's own — but it may be the app's *vendor's* folder,
                // holding one directory per product. `Application Support/Google`
                // contains Chrome's entire profile alongside Drive's. The folder
                // itself is never removable; the app's own directory inside it is,
                // and it is usually the largest leftover the app has.
                if path.is_dir() && is_vendor_dir(&name, tokens) {
                    for child in fsutil::children(&path) {
                        let Some(cname) = child
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(str::to_string)
                        else {
                            continue;
                        };
                        if safety::check_removable(&child).is_err() {
                            continue;
                        }
                        let Some(cm) = classify_in_vendor_dir(&cname, tokens) else {
                            continue;
                        };
                        let (csize, cunknown) = fsutil::size_with_confidence(&child);
                        found.push(Leftover {
                            size_bytes: csize,
                            size_unknown: cunknown,
                            is_directory: child.is_dir(),
                            kind,
                            confidence: cm.confidence,
                            reason: cm.reason,
                            requires_admin: safety::requires_admin(&child),
                            shared_with: Vec::new(),
                            name: cname.clone(),
                            path: child,
                        });
                        names.push(cname);
                    }
                }
                continue;
            };

            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            found.push(Leftover {
                size_bytes: size,
                size_unknown,
                is_directory: path.is_dir(),
                kind,
                confidence: m.confidence,
                reason: m.reason,
                requires_admin: safety::requires_admin(&path),
                shared_with: Vec::new(),
                name: name.clone(),
                path,
            });
            names.push(name);
        }
    }
    (found, names)
}

pub fn for_app(app: &InstalledApp, all_apps: &[InstalledApp]) -> Vec<Leftover> {
    let mut tokens = AppTokens::from_app(app);
    let others: Vec<AppTokens> = all_apps
        .iter()
        .filter(|a| a.id != app.id)
        .map(AppTokens::from_app)
        .collect();

    // First pass with what the app tells us about itself.
    let (mut found, names) = collect(&tokens);

    // Second pass with what the filesystem told us about its vendor. Only worth
    // doing when the first pass actually learned something new.
    let (vendors, slugs) = super::derive_extra_tokens(&names, &tokens, &others);
    if !vendors.is_empty() || !slugs.is_empty() {
        tokens.extra_vendors = vendors;
        tokens.extra_slugs = slugs;
        let (more, _) = collect(&tokens);
        for l in more {
            match found.iter_mut().find(|existing| existing.path == l.path) {
                // Already seen — but the second pass may have found stronger
                // evidence for it, and the user should be shown the best reason
                // we have, not the first one we happened to hit.
                Some(existing) if l.confidence > existing.confidence => *existing = l,
                Some(_) => {}
                None => found.push(l),
            }
        }
    }

    let names: Vec<String> = found.iter().map(|l| l.name.clone()).collect();
    resolve_sharing(&mut found, &names, &others);

    // A vendor-level match that nothing else claims is this app's own folder
    // after all, and can be trusted.
    for l in found.iter_mut() {
        if l.shared_with.is_empty() && l.confidence == Confidence::Medium {
            l.confidence = Confidence::High;
        }
    }

    found.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(b.size_bytes.cmp(&a.size_bytes))
    });
    found
}

pub fn orphans(all_apps: &[InstalledApp]) -> Vec<OrphanGroup> {
    let installed: Vec<AppTokens> = all_apps.iter().map(AppTokens::from_app).collect();
    let mut groups: Vec<OrphanGroup> = Vec::new();

    for (root, kind) in roots() {
        for path in fsutil::children(&root) {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let name = name.as_str();
            if is_system_owned(name) || safety::check_removable(&path).is_err() {
                continue;
            }
            // Owned by something still installed: not an orphan.
            if installed.iter().any(|t| classify(name, t).is_some()) {
                continue;
            }
            let Some(owner) = probable_owner(name) else {
                continue;
            };

            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            let leftover = Leftover {
                name: name.to_string(),
                size_bytes: size,
                size_unknown,
                is_directory: path.is_dir(),
                kind,
                // Orphans are never pre-ticked. We are inferring an owner that
                // is no longer here to confirm it, and the user may simply have
                // moved the app rather than deleted it.
                confidence: Confidence::Medium,
                reason: format!("left behind by \"{owner}\", which is no longer installed"),
                requires_admin: safety::requires_admin(&path),
                shared_with: Vec::new(),
                path,
            };

            match groups.iter_mut().find(|g| g.name == owner) {
                Some(g) => {
                    g.size_bytes += leftover.size_bytes;
                    g.items.push(leftover);
                }
                None => groups.push(OrphanGroup {
                    name: owner,
                    size_bytes: leftover.size_bytes,
                    items: vec![leftover],
                }),
            }
        }
    }

    groups.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
    groups
}

/// Guess which app a stray file belonged to.
///
/// Only reverse-DNS names are used. A bare folder name like `Adobe` or `data`
/// is far too ambiguous to attribute to a deleted app, so those are left alone
/// rather than guessed at.
fn probable_owner(name: &str) -> Option<String> {
    let base = name
        .strip_suffix(".plist")
        .or_else(|| name.strip_suffix(".savedState"))
        .unwrap_or(name);

    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let tld = parts[0].to_lowercase();
    if !matches!(
        tld.as_str(),
        "com" | "org" | "net" | "io" | "co" | "dev" | "app" | "me" | "us" | "eu" | "de" | "uk"
    ) {
        return None;
    }
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    // Group by vendor + product, ignoring helper suffixes, so an app's main
    // bundle and its helpers land in one row.
    Some(parts[..3.min(parts.len())].join("."))
}
