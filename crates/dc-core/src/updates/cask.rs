//! Homebrew's cask index, one cask at a time.
//!
//! The full index is 18 MB, which is far too much to pull for a version check.
//! A single cask is about 3 KB, so the token is guessed from the app and then
//! **verified**: the cask has to list this exact `.app` among its artifacts.
//! Without that check a guess like `opera` could quietly report the version of
//! something entirely different.

use super::{Found, UpdateSource};
use crate::model::InstalledApp;

pub fn latest(app: &InstalledApp) -> Option<Found> {
    let bundle_name = app
        .path
        .as_ref()?
        .file_name()?
        .to_string_lossy()
        .to_string();

    for token in candidate_tokens(app, &bundle_name) {
        let url = format!("https://formulae.brew.sh/api/cask/{token}.json");
        let Some(body) = super::fetch(&url) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
            continue;
        };
        if !names_this_app(&json, &bundle_name) {
            continue;
        }
        let raw = json.get("version").and_then(|v| v.as_str())?;
        // Casks with no upstream version use this placeholder; it tells us
        // nothing and must not be compared against anything.
        if raw.is_empty() || raw == "latest" {
            return None;
        }
        let version = upstream_version(raw);
        return Some(Found {
            version: version.to_string(),
            source: UpdateSource::HomebrewCask,
            url: json
                .get("homepage")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        });
    }
    None
}

/// Strip Homebrew's own packaging revision.
///
/// A cask version is `<upstream>,<revision>` — and the revision is often a
/// build id or a commit hash, e.g. `1.34493.1,255293a41a25d54c…`. Comparing the
/// whole string against the app's real version makes every such app look
/// permanently out of date, because the hash contributes extra digits that
/// always look like a higher version number.
fn upstream_version(raw: &str) -> &str {
    raw.split(',').next().unwrap_or(raw)
}

/// Does this cask actually install the app we are asking about?
fn names_this_app(json: &serde_json::Value, bundle_name: &str) -> bool {
    let Some(artifacts) = json.get("artifacts").and_then(|v| v.as_array()) else {
        return false;
    };
    artifacts.iter().any(|artifact| {
        artifact
            .get("app")
            .and_then(|v| v.as_array())
            .map(|apps| {
                apps.iter().any(|a| {
                    a.as_str()
                        .map(|s| s.eq_ignore_ascii_case(bundle_name))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

/// Homebrew tokens are the app's name, lowercased, with runs of anything
/// non-alphanumeric collapsed to a single hyphen. Both the bundle's filename
/// and its display name are tried, since they often differ — Visual Studio
/// Code presents itself as "Code" but ships as `Visual Studio Code.app`.
fn candidate_tokens(app: &InstalledApp, bundle_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let stem = bundle_name.trim_end_matches(".app");
    for source in [stem, app.name.as_str()] {
        let token = tokenise(source);
        if token.len() >= 2 && !out.contains(&token) {
            out.push(token);
        }
    }
    out
}

fn tokenise(s: &str) -> String {
    let mut out = String::new();
    let mut pending_sep = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            out.extend(c.to_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_follow_the_homebrew_convention() {
        assert_eq!(tokenise("Google Chrome"), "google-chrome");
        assert_eq!(tokenise("Visual Studio Code"), "visual-studio-code");
        assert_eq!(tokenise("Microsoft Edge"), "microsoft-edge");
        assert_eq!(tokenise("Firefox"), "firefox");
        assert_eq!(tokenise("balenaEtcher"), "balenaetcher");
        assert_eq!(tokenise("  Opera  "), "opera");
    }

    #[test]
    fn a_cask_must_name_the_exact_app_to_be_accepted() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"artifacts":[{"app":["Google Chrome.app"]},{"zap":[]}]}"#)
                .unwrap();
        assert!(names_this_app(&json, "Google Chrome.app"));
        assert!(names_this_app(&json, "google chrome.app"));
        // The guard that stops a wrong token reporting someone else's version.
        assert!(!names_this_app(&json, "Google Drive.app"));
        assert!(!names_this_app(&json, "Chromium.app"));
    }

    #[test]
    fn homebrew_revisions_are_stripped_before_comparing() {
        assert_eq!(
            upstream_version("1.34493.1,255293a41a25d54c5177aa9614fb4cd620e70b78"),
            "1.34493.1"
        );
        assert_eq!(upstream_version("4.3.9,147,1742287964"), "4.3.9");
        assert_eq!(
            upstream_version("151.0.4129.101,664c0246-6450-44a6"),
            "151.0.4129.101"
        );
        assert_eq!(upstream_version("2026.2.0"), "2026.2.0");
    }

    #[test]
    fn an_app_matching_its_cask_exactly_is_not_reported_as_outdated() {
        // The bug this guards: a commit hash in the revision contributes extra
        // digits, so the whole string always compares as newer.
        let installed = "1.34493.1";
        let cask = "1.34493.1,255293a41a25d54c5177aa9614fb4cd620e70b78";
        assert!(crate::version::is_newer(cask, installed));
        assert!(!crate::version::is_newer(upstream_version(cask), installed));
    }

    #[test]
    fn a_cask_with_no_app_artifact_is_not_a_match() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"artifacts":[{"pkg":["thing.pkg"]}]}"#).unwrap();
        assert!(!names_this_app(&json, "Thing.app"));
    }
}
