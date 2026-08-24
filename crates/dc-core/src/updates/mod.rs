//! Which installed apps have a newer version available.
//!
//! The reference product maintains its own version database. We do not want to
//! maintain one, and we especially do not want to send anyone's list of
//! installed software to a server to find out. So versions are read from
//! sources that already exist and are queried per app:
//!
//! 1. **The app's own Sparkle feed.** `SUFeedURL` in its `Info.plist`. This is
//!    the app telling us where it publishes updates. Authoritative, but only
//!    about 5% of installed apps ship one.
//! 2. **The Homebrew cask API.** One 3 KB request per app. The full index is
//!    18 MB, so it is never downloaded — instead the cask token is guessed from
//!    the bundle's filename and the answer is *verified* by checking that the
//!    cask's `artifacts` actually name this `.app`. A wrong guess is discarded
//!    rather than reported.
//! 3. **The App Store lookup API**, for apps installed from the Mac App Store.
//!
//! Nothing about the user is transmitted: each request is a public lookup of a
//! name that the app itself publishes.

use crate::model::InstalledApp;
use serde::{Deserialize, Serialize};

mod cask;
mod sparkle;
mod store;
pub(crate) use crate::version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSource {
    Sparkle,
    HomebrewCask,
    MacAppStore,
}

impl UpdateSource {
    pub fn label(self) -> &'static str {
        match self {
            UpdateSource::Sparkle => "The developer's own update feed",
            UpdateSource::HomebrewCask => "Homebrew",
            UpdateSource::MacAppStore => "Mac App Store",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub app_id: String,
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: String,
    pub source: UpdateSource,
    /// Where the user can get it. Never opened automatically.
    pub url: Option<String>,
    pub outdated: bool,
}

/// Check every app that has a version we can compare against.
///
/// Apps we cannot find a source for are simply absent from the result — an
/// unknown version is not the same as being up to date, and the UI says so.
pub fn check(apps: &[InstalledApp]) -> Vec<UpdateInfo> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8);
    let chunk = apps.len().div_ceil(workers).max(1);

    let mut out: Vec<UpdateInfo> = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = apps
            .chunks(chunk)
            .map(|c| scope.spawn(move || c.iter().filter_map(check_one).collect::<Vec<_>>()))
            .collect();
        for h in handles {
            out.extend(h.join().unwrap_or_default());
        }
    });

    // Outdated first, then alphabetical — the point of the screen is the ones
    // that need attention.
    out.sort_by(|a, b| {
        b.outdated
            .cmp(&a.outdated)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

fn check_one(app: &InstalledApp) -> Option<UpdateInfo> {
    // Nothing to compare against.
    let current = app.version.clone()?;

    let found = sparkle::latest(app)
        .or_else(|| store::latest(app))
        .or_else(|| cask::latest(app))?;

    Some(UpdateInfo {
        app_id: app.id.clone(),
        name: app.name.clone(),
        outdated: version::is_newer(&found.version, &current),
        current_version: Some(current),
        latest_version: found.version,
        source: found.source,
        url: found.url,
    })
}

/// What a source found.
pub(crate) struct Found {
    pub version: String,
    pub source: UpdateSource,
    pub url: Option<String>,
}

/// Shared HTTP access.
///
/// Only https is ever fetched — a Sparkle feed URL comes out of an app's own
/// `Info.plist`, which is not something to follow blindly to a plaintext host.
pub(crate) fn fetch(url: &str) -> Option<String> {
    if !url.starts_with("https://") {
        return None;
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(12)))
        // Let a 404 come back as a response rather than an error: "this repo
        // has no releases yet" is an answer, not a failure, and reporting it as
        // one would tell the user something is broken when nothing is.
        .http_status_as_error(false)
        .user_agent(concat!("DESKIO/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();

    let mut response = agent.get(url).call().ok()?;
    if response.status() != 200 {
        return None;
    }
    // Cap the read: a source that returns something enormous is a source we
    // want no part of.
    response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .ok()
}
