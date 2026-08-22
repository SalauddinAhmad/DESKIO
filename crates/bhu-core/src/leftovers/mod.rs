//! Finding what an app leaves behind.
//!
//! The matching logic lives here and is shared by all three platforms; only the
//! list of places to look is OS-specific.
//!
//! ## Why this is the delicate part
//!
//! Leftover matching is guesswork. The app is gone, or about to be, and all we
//! have to go on is that some file's name resembles some identifier of the app.
//! A directory called `Google` belongs to Chrome, Drive, Earth and the updater
//! all at once; deleting it because the user is uninstalling Drive would take
//! Chrome's entire profile with it.
//!
//! So the rule here is: a match is only `High` when the evidence is an *exact*
//! identifier, and any candidate that also matches another installed app is
//! demoted and flagged as shared, no matter how strong the evidence looked.

use crate::model::*;

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

/// The identifiers we try to match a stray file against.
#[derive(Debug, Clone)]
pub struct AppTokens {
    /// e.g. `org.freedownloadmanager.fdm6`
    pub bundle_id: Option<String>,
    /// The bundle id minus its last component, e.g. `org.freedownloadmanager`.
    /// Shared between all of a vendor's apps, which is exactly why a match on
    /// it is never trusted on its own.
    pub vendor_prefix: Option<String>,
    /// e.g. `Free Download Manager`
    pub display_name: String,
    /// e.g. `freedownloadmanager`
    pub name_slug: String,
    /// e.g. `Softdeluxe`, from the code-signing authority.
    pub publisher_slug: Option<String>,
    /// The main binary's name, e.g. `fdm`.
    pub executable: Option<String>,
    /// Vendor namespaces learned from the filesystem during a first pass —
    /// see [`derive_extra_tokens`]. e.g. `com.softdeluxe`.
    pub extra_vendors: Vec<String>,
    /// Vendor words learned the same way, e.g. `softdeluxe`.
    pub extra_slugs: Vec<String>,
    /// The last component of the bundle id: `Chrome`, `drivefs`, `fdm6`.
    /// Only meaningful inside a vendor folder, where the vendor is implied by
    /// the parent directory.
    pub bundle_leaf: Option<String>,
    /// The display name with the vendor word taken off the front:
    /// `Google Chrome` -> `chrome`. Again, only used inside a vendor folder.
    pub residual_slug: Option<String>,
    /// Words that name a folder belonging to this app's vendor: `google`,
    /// `softdeluxe`. A folder matching one of these is descended into, never
    /// removed.
    pub vendor_words: Vec<String>,
}

/// First words of app names that say nothing about who made them.
const STOP_WORDS: &[&str] = &[
    "free", "open", "the", "app", "mac", "pro", "new", "easy", "smart", "super", "my", "auto",
    "quick", "simple", "best", "cloud", "desktop", "studio",
];

/// Binary names too generic to match on. `~/Library/Application Support/Main`
/// belongs to nobody in particular.
const GENERIC_EXECUTABLES: &[&str] = &[
    "main",
    "app",
    "application",
    "electron",
    "launcher",
    "run",
    "start",
    "helper",
    "updater",
    "installer",
    "bin",
    "exe",
    "java",
    "python",
    "node",
    "stub",
];

/// Lowercase, strip everything that is not a letter or digit. `Free Download
/// Manager` and `free-download-manager` both become `freedownloadmanager`.
pub fn slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

impl AppTokens {
    pub fn from_app(app: &InstalledApp) -> Self {
        let bundle_id = app.bundle_id.clone().filter(|b| !b.is_empty());
        let bundle_id_ref = bundle_id.clone();
        let vendor_prefix = bundle_id.as_ref().and_then(|b| {
            let parts: Vec<&str> = b.split('.').collect();
            // Only meaningful with at least three components; `com.foo` on its
            // own is a top-level vendor namespace, not an app identifier.
            (parts.len() >= 3).then(|| parts[..parts.len() - 1].join("."))
        });
        let vendor_prefix_ref = vendor_prefix.clone();
        let name_slug_val = slug(&app.name);
        // `Google Chrome` under a `Google` folder is just `Chrome`.
        let residual = vendor_words_for(&app.name, &vendor_prefix_ref, &app.publisher)
            .into_iter()
            .find_map(|w| {
                name_slug_val
                    .strip_prefix(&w)
                    .filter(|r| r.len() >= 3)
                    .map(str::to_string)
            });

        AppTokens {
            bundle_id,
            vendor_prefix,
            display_name: app.name.clone(),
            name_slug: slug(&app.name),
            publisher_slug: app
                .publisher
                .as_ref()
                .map(|p| slug(p))
                .filter(|p| p.len() >= 4),
            executable: app.executable.clone().filter(|e| {
                e.len() >= 3 && !GENERIC_EXECUTABLES.contains(&e.to_lowercase().as_str())
            }),
            extra_vendors: Vec::new(),
            extra_slugs: Vec::new(),
            bundle_leaf: bundle_id_ref
                .as_ref()
                .and_then(|b| b.rsplit('.').next())
                .map(slug)
                .filter(|l| l.len() >= 3),
            residual_slug: residual,
            vendor_words: vendor_words_for(&app.name, &vendor_prefix_ref, &app.publisher),
        }
    }
}

/// Words that would name this app's vendor folder.
fn vendor_words_for(
    display_name: &str,
    vendor_prefix: &Option<String>,
    publisher: &Option<String>,
) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut add = |w: String| {
        if w.len() >= 4 && !STOP_WORDS.contains(&w.as_str()) && !words.contains(&w) {
            words.push(w);
        }
    };
    // `com.google.Chrome` -> `google`
    if let Some(v) = vendor_prefix {
        if let Some(last) = v.rsplit('.').next() {
            add(slug(last));
        }
    }
    // `Google Chrome` -> `google`, but only when there is a second word for the
    // vendor word to be qualifying. A single-word app name is the app, not a vendor.
    let parts: Vec<&str> = display_name.split_whitespace().collect();
    if parts.len() >= 2 {
        add(slug(parts[0]));
    }
    if let Some(p) = publisher {
        add(slug(p));
        if let Some(first) = p.split_whitespace().next() {
            add(slug(first));
        }
    }
    words
}

/// Match a file found *inside* a known vendor folder.
///
/// The parent directory has already established the vendor, so the only
/// question left is whether this particular child is the app — and that is
/// answered with exact matches only. `Application Support/Google/Chrome` is
/// Chrome's; `Application Support/Google/DriveFS` is Drive's; neither may claim
/// the `Google` folder itself.
pub fn classify_in_vendor_dir(child_name: &str, tokens: &AppTokens) -> Option<Match> {
    let base = strip_group_prefix(strip_byhost_uuid(strip_wrapper_extension(child_name)));
    let base_slug = slug(base);
    if base_slug.len() < 3 {
        return None;
    }

    let hit = |what: &str| {
        Some(Match {
            confidence: Confidence::High,
            reason: format!("inside this app's vendor folder, named after its {what}"),
            via_vendor: false,
        })
    };

    if let Some(bid) = &tokens.bundle_id {
        if base.eq_ignore_ascii_case(bid) {
            return hit("bundle id");
        }
    }
    if tokens.bundle_leaf.as_deref() == Some(base_slug.as_str()) {
        return hit("bundle id");
    }
    if tokens.residual_slug.as_deref() == Some(base_slug.as_str()) {
        return hit("name");
    }
    if base_slug == tokens.name_slug {
        return hit("name");
    }
    None
}

/// True when this folder looks like it belongs to the app's vendor rather than
/// to the app — a folder to look inside, never one to remove.
pub fn is_vendor_dir(name: &str, tokens: &AppTokens) -> bool {
    let s = slug(name);
    tokens.vendor_words.contains(&s) || tokens.extra_slugs.contains(&s)
}

/// Strip the extensions that wrap an identifier rather than being part of it,
/// so `org.example.app.plist` and `org.example.app.savedState` both reduce to
/// `org.example.app`.
fn strip_wrapper_extension(name: &str) -> &str {
    for ext in [
        ".plist",
        ".savedState",
        ".sfl3",
        ".sfl2",
        ".binarycookies",
        ".log",
        ".lockfile",
    ] {
        if let Some(base) = name.strip_suffix(ext) {
            return base;
        }
    }
    name
}

/// Remove the `ByHost` hardware-UUID suffix macOS appends to some preference
/// files: `com.example.app.0123ABCD-....plist`.
fn strip_byhost_uuid(name: &str) -> &str {
    let parts: Vec<&str> = name.rsplitn(2, '.').collect();
    if parts.len() == 2 {
        let last = parts[0];
        let looks_like_uuid =
            last.len() >= 32 && last.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if looks_like_uuid {
            return parts[1];
        }
    }
    name
}

/// macOS group containers are named `TEAMID.group.com.vendor.app`. The team id
/// tells us nothing, and leaving it on hides the bundle id behind it.
fn strip_group_prefix(name: &str) -> &str {
    let Some((head, rest)) = name.split_once(".group.") else {
        return name;
    };
    let is_team_id = head.len() == 10 && head.chars().all(|c| c.is_ascii_alphanumeric());
    if is_team_id {
        rest
    } else {
        name
    }
}

/// How strongly a filename matches an app, and why.
#[derive(Debug, Clone)]
pub struct Match {
    pub confidence: Confidence,
    pub reason: String,
    /// True when the evidence was a vendor namespace rather than the app's own
    /// identifier — these get demoted if any other app shares the vendor.
    pub via_vendor: bool,
}

/// Decide whether `file_name` belongs to the app described by `tokens`.
///
/// Returns `None` when there is no evidence at all, which is the common case —
/// most of `~/Library` has nothing to do with any given app.
pub fn classify(file_name: &str, tokens: &AppTokens) -> Option<Match> {
    let base = strip_group_prefix(strip_byhost_uuid(strip_wrapper_extension(file_name)));
    let base_slug = slug(base);

    // 1. Exact bundle identifier. The only evidence strong enough to tick a box
    //    on the user's behalf without further checks.
    if let Some(bid) = &tokens.bundle_id {
        if base.eq_ignore_ascii_case(bid) {
            return Some(Match {
                confidence: Confidence::High,
                reason: format!("exactly matches the bundle id {bid}"),
                via_vendor: false,
            });
        }
        // A helper or subsystem of the app: `com.example.app.helper`.
        if base.len() > bid.len()
            && base
                .to_lowercase()
                .starts_with(&format!("{}.", bid.to_lowercase()))
        {
            return Some(Match {
                confidence: Confidence::High,
                reason: format!("belongs to the bundle id {bid}"),
                via_vendor: false,
            });
        }
    }

    // 2. Vendor namespace. Real evidence, but shared across everything the
    //    vendor ships — only trusted once we know no other installed app
    //    shares it (see `resolve_sharing`).
    if let Some(vendor) = &tokens.vendor_prefix {
        let lower = base.to_lowercase();
        let vlower = vendor.to_lowercase();
        if lower == vlower || lower.starts_with(&format!("{vlower}.")) {
            return Some(Match {
                confidence: Confidence::Medium,
                reason: format!("matches the vendor namespace {vendor}"),
                via_vendor: true,
            });
        }
    }

    // 3. The app's display name, exactly. `Free Download Manager` as a folder
    //    name. Short names are excluded: a three-letter app name matches far
    //    too much.
    if tokens.name_slug.len() >= 4 && base_slug == tokens.name_slug {
        return Some(Match {
            confidence: Confidence::High,
            reason: format!("named exactly after the app ({})", tokens.display_name),
            via_vendor: false,
        });
    }

    // 4. The publisher's name, e.g. a `Softdeluxe` folder for an app signed by
    //    Softdeluxe. Vendor-level evidence, so treated like case 2.
    if let Some(pub_slug) = &tokens.publisher_slug {
        if base_slug == *pub_slug {
            return Some(Match {
                confidence: Confidence::Medium,
                reason: "named after the app's developer".to_string(),
                via_vendor: true,
            });
        }
    }

    // 5. The app's own binary name, as a whole word. Apps name crash reports
    //    and support folders after the executable rather than the bundle id —
    //    `fdm_<uuid>.plist` for a binary called `fdm`. Only exact matches or
    //    matches followed by a separator count, so `fdm` never matches `fdmail`.
    if let Some(exe) = &tokens.executable {
        let lower = base.to_lowercase();
        let elower = exe.to_lowercase();
        let separated = ["_", "-", ".", " "]
            .iter()
            .any(|sep| lower.starts_with(&format!("{elower}{sep}")));
        if lower == elower || separated {
            return Some(Match {
                confidence: Confidence::Medium,
                reason: format!("named after the app's binary ({exe})"),
                via_vendor: false,
            });
        }
    }

    // 6. Vendor names learned from the filesystem in the first pass — the
    //    publisher an app files things under is often not the one it is signed
    //    by. Vendor-level evidence, so demoted if anything else shares it.
    for vendor in &tokens.extra_vendors {
        let lower = base.to_lowercase();
        let vlower = vendor.to_lowercase();
        if lower == vlower || lower.starts_with(&format!("{vlower}.")) {
            return Some(Match {
                confidence: Confidence::Medium,
                reason: format!("matches the vendor namespace {vendor}"),
                via_vendor: true,
            });
        }
    }
    for extra in &tokens.extra_slugs {
        if base_slug == *extra {
            return Some(Match {
                confidence: Confidence::Medium,
                reason: "named after the app's vendor".to_string(),
                via_vendor: true,
            });
        }
    }

    // 7. Contains the app name. Weak, and only worth reporting for names long
    //    enough that a coincidence is unlikely.
    if tokens.name_slug.len() >= 6 && base_slug.contains(&tokens.name_slug) {
        return Some(Match {
            confidence: Confidence::Low,
            reason: format!("name contains \"{}\"", tokens.display_name),
            via_vendor: false,
        });
    }

    None
}

/// Learn vendor identifiers from the filesystem itself.
///
/// An app is often filed under a company name that appears nowhere in its own
/// metadata. Free Download Manager is signed by an individual and its bundle id
/// is `org.freedownloadmanager.fdm6`, yet it keeps its data in
/// `Application Support/Softdeluxe` — the only trace of that name anywhere is a
/// preference file called `com.softdeluxe.Free Download Manager.plist`.
///
/// So: after a first pass, any *reverse-DNS* filename we matched by the app's
/// own display name tells us a vendor namespace we did not know about. Feeding
/// it back in as a token finds the rest of that vendor's folders.
///
/// A learned vendor is discarded when any *other* installed app sits under it.
/// Without that check, `com.google.drivefs.plist` would teach the Google Drive
/// scan to claim every `google` folder on the disk, Chrome's profile included.
pub fn derive_extra_tokens(
    matched_names: &[String],
    tokens: &AppTokens,
    others: &[AppTokens],
) -> (Vec<String>, Vec<String>) {
    let mut vendors: Vec<String> = Vec::new();
    let mut slugs: Vec<String> = Vec::new();

    for name in matched_names {
        let base = strip_byhost_uuid(strip_wrapper_extension(name));
        let parts: Vec<&str> = base.split('.').collect();
        if parts.len() < 3 {
            continue;
        }
        // Only learn from a file that carries the app's own name — otherwise
        // we would be learning from a coincidence.
        let tail_slug = slug(&parts[2..].join("."));
        if tokens.name_slug.len() < 4 || tail_slug != tokens.name_slug {
            continue;
        }

        let vendor = parts[..2].join(".");
        let vendor_word = slug(parts[1]);
        if vendor_word.len() < 4 {
            continue;
        }
        // Already known, or shared with another installed app: skip.
        if Some(&vendor) == tokens.vendor_prefix.as_ref() {
            continue;
        }
        let shared = others.iter().any(|o| {
            o.bundle_id
                .as_deref()
                .map(|b| {
                    b.to_lowercase()
                        .starts_with(&format!("{}.", vendor.to_lowercase()))
                })
                .unwrap_or(false)
                || o.vendor_prefix
                    .as_deref()
                    .map(|v| v.eq_ignore_ascii_case(&vendor))
                    .unwrap_or(false)
                || o.name_slug == vendor_word
        });
        if shared {
            continue;
        }
        if !vendors.contains(&vendor) {
            vendors.push(vendor);
        }
        if !slugs.contains(&vendor_word) {
            slugs.push(vendor_word);
        }
    }
    (vendors, slugs)
}

/// Demote any candidate that another installed app can claim just as strongly.
///
/// This is the guard that stops uninstalling Google Drive from taking Chrome's
/// preferences with it: both match `com.google.Chrome.plist`, so the weaker
/// claim loses and the UI names who else lives there.
///
/// "Just as strongly" is the important part. `com.google.drivefs.plist` matches
/// Drive by its exact bundle id and Chrome only by the shared `com.google`
/// namespace — a specific claim beats a namespace claim, so Drive keeps it.
/// Without that comparison every app from a multi-app vendor would end up
/// unable to clean up after itself.
pub fn resolve_sharing(leftovers: &mut [Leftover], file_names: &[String], others: &[AppTokens]) {
    for (leftover, name) in leftovers.iter_mut().zip(file_names.iter()) {
        let mut shared: Vec<String> = Vec::new();
        for other in others {
            if let Some(m) = classify(name, other) {
                if m.confidence >= leftover.confidence {
                    shared.push(other.display_name.clone());
                }
            }
        }
        if !shared.is_empty() {
            leftover.shared_with = shared;
            // Shared evidence can never be strong evidence.
            leftover.confidence = Confidence::Low;
            leftover.reason = format!(
                "{} — but this is also used by {}",
                leftover.reason,
                leftover.shared_with.join(", ")
            );
        }
    }
}

/// Everything the given app has left around the system.
pub fn for_app(app: &InstalledApp, all_apps: &[InstalledApp]) -> Vec<Leftover> {
    imp::for_app(app, all_apps)
}

/// Leftovers belonging to apps that are no longer installed — the "Remaining
/// Files" section.
pub fn orphans(all_apps: &[InstalledApp]) -> Vec<OrphanGroup> {
    imp::orphans(all_apps)
}

/// Orphaned leftovers, grouped by the app they appear to have belonged to.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrphanGroup {
    /// Best guess at the owner: a bundle id, vendor name or folder name.
    pub name: String,
    pub items: Vec<Leftover>,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(bundle: &str, name: &str, publisher: Option<&str>) -> AppTokens {
        AppTokens {
            bundle_id: Some(bundle.into()),
            vendor_prefix: {
                let parts: Vec<&str> = bundle.split('.').collect();
                (parts.len() >= 3).then(|| parts[..parts.len() - 1].join("."))
            },
            display_name: name.into(),
            name_slug: slug(name),
            publisher_slug: publisher.map(slug),
            executable: None,
            extra_vendors: Vec::new(),
            extra_slugs: Vec::new(),
            bundle_leaf: bundle.rsplit('.').next().map(slug),
            residual_slug: None,
            vendor_words: vendor_words_for(
                name,
                &{
                    let parts: Vec<&str> = bundle.split('.').collect();
                    (parts.len() >= 3).then(|| parts[..parts.len() - 1].join("."))
                },
                &publisher.map(str::to_string),
            ),
        }
    }

    #[test]
    fn exact_bundle_id_is_high_confidence() {
        let t = tokens(
            "org.freedownloadmanager.fdm6",
            "Free Download Manager",
            None,
        );
        let m = classify("org.freedownloadmanager.fdm6.plist", &t).unwrap();
        assert_eq!(m.confidence, Confidence::High);
        let m = classify("org.freedownloadmanager.fdm6", &t).unwrap();
        assert_eq!(m.confidence, Confidence::High);
    }

    #[test]
    fn byhost_preference_files_still_match() {
        let t = tokens("com.example.app", "Example", None);
        let m = classify(
            "com.example.app.00000000-0000-1000-8000-0011223344AA.plist",
            &t,
        )
        .unwrap();
        assert_eq!(m.confidence, Confidence::High);
    }

    #[test]
    fn vendor_namespace_is_only_medium() {
        let t = tokens("com.google.drivefs", "Google Drive", None);
        let m = classify("com.google", &t).unwrap();
        assert_eq!(m.confidence, Confidence::Medium);
        assert!(m.via_vendor);
    }

    #[test]
    fn publisher_folder_is_medium() {
        let t = tokens(
            "org.freedownloadmanager.fdm6",
            "Free Download Manager",
            Some("Softdeluxe"),
        );
        let m = classify("Softdeluxe", &t).unwrap();
        assert_eq!(m.confidence, Confidence::Medium);
    }

    #[test]
    fn short_names_do_not_match_loosely() {
        let t = tokens("com.example.zoo", "Zoo", None);
        assert!(classify("Zookeeper Data", &t).is_none());
        assert!(classify("zoom.us", &t).is_none());
    }

    #[test]
    fn unrelated_files_do_not_match() {
        let t = tokens(
            "org.freedownloadmanager.fdm6",
            "Free Download Manager",
            None,
        );
        assert!(classify("com.apple.finder.plist", &t).is_none());
        assert!(classify("Firefox", &t).is_none());
    }

    #[test]
    fn a_bare_vendor_word_is_claimed_by_nobody() {
        // `~/Library/Application Support/Google` holds Chrome, Drive and the
        // updater. No single token matches it, so neither app can claim it and
        // it is never offered for removal at all — which is the outcome we want.
        let drive = tokens("com.google.drivefs", "Google Drive", Some("Google LLC"));
        let chrome = tokens("com.google.Chrome", "Google Chrome", Some("Google LLC"));
        assert!(classify("Google", &drive).is_none());
        assert!(classify("Google", &chrome).is_none());
    }

    #[test]
    fn a_shared_vendor_namespace_is_demoted_and_flagged() {
        // Where a name *does* match two apps — here the `com.google` vendor
        // namespace — sharing resolution is what stops either one claiming it.
        let drive = tokens("com.google.drivefs", "Google Drive", Some("Google LLC"));
        let chrome = tokens("com.google.Chrome", "Google Chrome", Some("Google LLC"));

        let m = classify("com.google", &drive).expect("vendor namespace matches");
        assert_eq!(m.confidence, Confidence::Medium);
        assert!(m.via_vendor);

        let mut lefts = vec![Leftover {
            path: "/tmp/com.google".into(),
            name: "com.google".into(),
            size_bytes: 1,
            size_unknown: false,
            is_directory: true,
            kind: LeftoverKind::ApplicationSupport,
            confidence: m.confidence,
            reason: m.reason,
            requires_admin: false,
            shared_with: vec![],
        }];
        resolve_sharing(&mut lefts, &["com.google".to_string()], &[chrome]);

        assert_eq!(lefts[0].confidence, Confidence::Low);
        assert_eq!(lefts[0].shared_with, vec!["Google Chrome".to_string()]);
        // And being Low, it can never be pre-ticked.
        assert!(!lefts[0].confidence.preselected());
    }

    #[test]
    fn an_unshared_vendor_namespace_stays_claimable() {
        // The other side of the same rule: nothing else is installed under
        // org.freedownloadmanager, so its folder really is FDM's.
        let fdm = tokens(
            "org.freedownloadmanager.fdm6",
            "Free Download Manager",
            None,
        );
        let firefox = tokens("org.mozilla.firefox", "Firefox", None);

        let mut lefts = vec![Leftover {
            path: "/tmp/org.freedownloadmanager".into(),
            name: "org.freedownloadmanager".into(),
            size_bytes: 1,
            size_unknown: false,
            is_directory: true,
            kind: LeftoverKind::ApplicationSupport,
            confidence: classify("org.freedownloadmanager", &fdm)
                .unwrap()
                .confidence,
            reason: "vendor".into(),
            requires_admin: false,
            shared_with: vec![],
        }];
        resolve_sharing(
            &mut lefts,
            &["org.freedownloadmanager".to_string()],
            &[firefox],
        );
        assert!(lefts[0].shared_with.is_empty());
        assert_eq!(lefts[0].confidence, Confidence::Medium);
    }
}
