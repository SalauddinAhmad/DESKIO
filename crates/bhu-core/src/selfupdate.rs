//! Checking GitHub for a newer BHUninstaller.
//!
//! Deliberately modest: it looks at the project's own releases page, compares
//! version numbers, and hands the user the download. It does not install
//! anything behind their back, and it does not phone home — the request carries
//! nothing but a user agent.
//!
//! ## Rules this follows
//!
//! - **Only GitHub.** The URL taken from the API must be exactly `github.com`
//!   or `objects.githubusercontent.com` — a lookalike such as
//!   `github.com.example.net` is rejected, so a tampered API response cannot
//!   point the download somewhere else. GitHub itself then redirects to its
//!   asset CDN, which is followed; the check is on the URL we are *given*, and
//!   that is the one an attacker would have to control.
//! - **Numbers, not strings.** `1.10` is newer than `1.9`; string comparison
//!   says otherwise and would nag forever.
//! - **Silence on failure.** Being offline is normal. A failed check is never
//!   reported as "you are up to date" — that is a different and much more
//!   misleading statement.
//! - **Throttled.** The unauthenticated GitHub API allows 60 requests an hour
//!   per *IP*, shared by everyone behind it. Checking on every launch would be
//!   rude to the user's whole network.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO: &str = "wpexpertinbd/BHUninstaller";
const ALLOWED_HOSTS: &[&str] = &["github.com", "objects.githubusercontent.com"];

/// How long to leave between automatic checks.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub notes: String,
    /// The release page, for someone who would rather look before downloading.
    pub page_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// The newer release, when there is one.
    pub update: Option<Release>,
    /// What is running now.
    pub current: String,
    /// Set when the check could not be completed. Distinct from "no update":
    /// the UI must not turn a failure into reassurance.
    pub error: Option<String>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Whether an automatic check is due.
pub fn check_due(last_check: i64) -> bool {
    let elapsed = now_secs().saturating_sub(last_check);
    elapsed >= CHECK_INTERVAL.as_secs() as i64
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        // A 404 means "no releases published yet", which is an answer rather
        // than a failure. Without this ureq turns it into an error and the user
        // is told the check broke.
        .http_status_as_error(false)
        .user_agent(concat!("BHUninstaller/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

/// Is this a URL we are willing to download from?
fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    // Exact match only. `starts_with` would accept github.com.evil.example.com.
    ALLOWED_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h))
}

/// Ask GitHub for the newest release.
pub fn check() -> CheckResult {
    let current = crate::VERSION.to_string();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let body = match agent().get(&url).call() {
        Ok(mut response) => {
            if response.status() != 200 {
                // A project with no releases yet answers 404. That is not an
                // error worth showing anyone.
                return CheckResult {
                    update: None,
                    current,
                    error: (response.status() != 404)
                        .then(|| format!("GitHub answered {}", response.status())),
                };
            }
            match response
                .body_mut()
                .with_config()
                .limit(1024 * 1024)
                .read_to_string()
            {
                Ok(text) => text,
                Err(e) => {
                    return CheckResult {
                        update: None,
                        current,
                        error: Some(e.to_string()),
                    }
                }
            }
        }
        Err(e) => {
            return CheckResult {
                update: None,
                current,
                error: Some(e.to_string()),
            }
        }
    };

    let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
        return CheckResult {
            update: None,
            current,
            error: Some("could not read GitHub's answer".into()),
        };
    };

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim_start_matches('v')
        .to_string();

    if tag.is_empty() || !crate::version::is_newer(&tag, &current) {
        return CheckResult {
            update: None,
            current,
            error: None,
        };
    }

    let asset = pick_asset(&json);
    CheckResult {
        update: Some(Release {
            version: tag,
            notes: json
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            page_url: json
                .get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("https://github.com/{REPO}/releases"))
                .to_string(),
            asset_name: asset.as_ref().map(|a| a.0.clone()),
            asset_url: asset.as_ref().map(|a| a.1.clone()),
            asset_size: asset.as_ref().map(|a| a.2).unwrap_or(0),
        }),
        current,
        error: None,
    }
}

/// The asset for this platform: (name, url, size).
fn pick_asset(json: &serde_json::Value) -> Option<(String, String, u64)> {
    let wanted: &[&str] = if cfg!(target_os = "macos") {
        &[".dmg", ".pkg"]
    } else if cfg!(target_os = "windows") {
        &[".msi", ".exe"]
    } else {
        &[".appimage", ".deb", ".rpm"]
    };

    let assets = json.get("assets")?.as_array()?;
    for suffix in wanted {
        for asset in assets {
            let name = asset.get("name")?.as_str().unwrap_or_default();
            if !name.to_lowercase().ends_with(suffix) {
                continue;
            }
            let url = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if !host_allowed(url) {
                continue;
            }
            let size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            return Some((name.to_string(), url.to_string(), size));
        }
    }
    None
}

/// Download a release asset and return where it landed.
///
/// Nothing is executed. The file is handed to the user's file manager, which is
/// where the decision to install belongs.
pub fn download(release: &Release) -> Result<PathBuf, String> {
    let url = release
        .asset_url
        .as_ref()
        .ok_or("this release has no download for this platform")?;
    let name = release
        .asset_name
        .as_deref()
        .unwrap_or("BHUninstaller-update");

    if !host_allowed(url) {
        return Err("that download is not hosted on GitHub — refusing it".into());
    }

    let mut response = agent()
        .get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    if response.status() != 200 {
        return Err(format!(
            "download failed: GitHub answered {}",
            response.status()
        ));
    }

    // 250 MB is far larger than any build of this app; the cap is only there so
    // an unexpected response cannot fill the disk.
    let bytes = response
        .body_mut()
        .with_config()
        .limit(250 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("download failed: {e}"))?;

    // Per-process, so two runs cannot collide and nothing can pre-place a
    // symlink at a path we are about to write to.
    let dir = std::env::temp_dir().join(format!("BHUninstaller-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // The name comes from GitHub, so it is not trusted as a path.
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(safe);
    // `create_new` refuses an existing file — including a symlink — rather than
    // writing through it.
    let _ = std::fs::remove_file(&path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("could not save the download: {e}"))?;
    std::io::Write::write_all(&mut file, &bytes)
        .map_err(|e| format!("could not save the download: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_hosts_are_accepted() {
        assert!(host_allowed(
            "https://github.com/x/y/releases/download/v1/a.dmg"
        ));
        assert!(host_allowed("https://objects.githubusercontent.com/blob"));
    }

    #[test]
    fn lookalike_hosts_are_refused() {
        // The whole point of an exact match rather than a prefix test.
        assert!(!host_allowed("https://github.com.evil.example.com/a.dmg"));
        assert!(!host_allowed("https://notgithub.com/a.dmg"));
        assert!(!host_allowed("https://evil.example.com/github.com/a.dmg"));
        assert!(!host_allowed("https://user@evil.example.com/a.dmg"));
    }

    #[test]
    fn plaintext_is_refused() {
        assert!(!host_allowed("http://github.com/x/y/a.dmg"));
    }

    #[test]
    fn a_check_is_due_only_after_the_interval() {
        let now = now_secs();
        assert!(!check_due(now));
        assert!(check_due(now - CHECK_INTERVAL.as_secs() as i64 - 1));
        // Never checked before.
        assert!(check_due(0));
    }
}
