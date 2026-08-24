//! The app's own update feed.
//!
//! Sparkle is the framework most independent Mac apps use to update
//! themselves, and an app that uses it names its feed in `Info.plist`. That
//! makes this the most authoritative source available — it is the developer's
//! own answer — though only a minority of apps ship one.

use super::{Found, UpdateSource};
use crate::model::InstalledApp;

pub fn latest(app: &InstalledApp) -> Option<Found> {
    let feed = feed_url(app)?;
    let xml = super::fetch(&feed)?;
    let version = newest_version(&xml)?;
    Some(Found {
        version,
        source: UpdateSource::Sparkle,
        url: Some(feed),
    })
}

#[cfg(target_os = "macos")]
fn feed_url(app: &InstalledApp) -> Option<String> {
    let info = app.path.as_ref()?.join("Contents/Info.plist");
    let dict = plist::Value::from_file(info).ok()?.into_dictionary()?;
    // `SUFeedURL` is the standard key; a few apps use the original casing.
    for key in ["SUFeedURL", "SUFeedUrl"] {
        if let Some(url) = dict.get(key).and_then(|v| v.as_string()) {
            return Some(url.to_string());
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn feed_url(_app: &InstalledApp) -> Option<String> {
    None
}

/// Pull the highest version out of an appcast.
///
/// The feed is RSS with Sparkle's own attributes. Rather than pulling in an XML
/// parser for a handful of attributes, the version strings are extracted
/// directly and compared — an appcast lists every release, and they are not
/// reliably in order, so the newest is chosen rather than the first.
fn newest_version(xml: &str) -> Option<String> {
    // `shortVersionString` is the version a user recognises ("9.2.4").
    // `version` is the build number ("2269"). They must never be mixed: a build
    // number always compares as vastly newer than a version, so an app on 9.2.4
    // would be told to update to 2269. Only fall back to the build number when
    // the feed offers nothing else.
    best_of(xml, "sparkle:shortVersionString").or_else(|| best_of(xml, "sparkle:version"))
}

/// The highest value of one Sparkle field across the whole feed.
fn best_of(xml: &str, field: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut consider = |v: &str| {
        let v = v.trim();
        if v.is_empty() {
            return;
        }
        match &best {
            Some(current) if !crate::version::is_newer(v, current) => {}
            _ => best = Some(v.to_string()),
        }
    };

    // Attribute form: sparkle:shortVersionString="2.1.0"
    let attr = format!("{field}=\"");
    let mut rest = xml;
    while let Some(start) = rest.find(&attr) {
        rest = &rest[start + attr.len()..];
        if let Some(end) = rest.find('"') {
            consider(&rest[..end]);
        }
    }

    // Element form: <sparkle:shortVersionString>2.1.0</sparkle:shortVersionString>
    let open = format!("<{field}>");
    let close = format!("</{field}>");
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        rest = &rest[start + open.len()..];
        if let Some(end) = rest.find(&close) {
            consider(&rest[..end]);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_attribute_form() {
        let xml = r#"<rss><channel>
            <item><enclosure sparkle:shortVersionString="2.0.1" url="x"/></item>
            <item><enclosure sparkle:shortVersionString="2.1.0" url="y"/></item>
        </channel></rss>"#;
        assert_eq!(newest_version(xml).as_deref(), Some("2.1.0"));
    }

    #[test]
    fn reads_the_element_form() {
        let xml = "<item><sparkle:shortVersionString>3.4</sparkle:shortVersionString></item>";
        assert_eq!(newest_version(xml).as_deref(), Some("3.4"));
    }

    #[test]
    fn picks_the_newest_not_the_first() {
        // Appcasts are not reliably ordered, and taking the first entry is how
        // an updater ends up offering an older release than the one installed.
        let xml = r#"<i sparkle:shortVersionString="1.10"/><i sparkle:shortVersionString="1.9"/>"#;
        assert_eq!(newest_version(xml).as_deref(), Some("1.10"));
    }

    #[test]
    fn a_build_number_never_stands_in_for_a_version() {
        // Real shape of App Cleaner's feed: both fields present. Taking the
        // build number would tell someone on 9.2.4 to update to 2269.
        let xml = r#"<item sparkle:shortVersionString="9.2.5" sparkle:version="2269"/>"#;
        assert_eq!(newest_version(xml).as_deref(), Some("9.2.5"));
    }

    #[test]
    fn the_build_number_is_used_only_when_there_is_nothing_better() {
        let xml = r#"<item sparkle:version="2269"/>"#;
        assert_eq!(newest_version(xml).as_deref(), Some("2269"));
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(newest_version("<rss></rss>").is_none());
    }
}
