//! The Mac App Store lookup API.
//!
//! Only used for apps that came from the App Store, identified by the receipt
//! inside the bundle. The lookup is by bundle id, which the app publishes
//! itself; nothing about the user is included.

use super::{Found, UpdateSource};
use crate::model::{AppSource, InstalledApp};

pub fn latest(app: &InstalledApp) -> Option<Found> {
    if app.source != AppSource::MacAppStore {
        return None;
    }
    let bundle_id = app.bundle_id.as_ref()?;
    let url = format!("https://itunes.apple.com/lookup?bundleId={bundle_id}");
    let body = super::fetch(&url)?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let result = json.get("results")?.as_array()?.first()?;

    Some(Found {
        version: result.get("version")?.as_str()?.to_string(),
        source: UpdateSource::MacAppStore,
        url: result
            .get("trackViewUrl")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}
