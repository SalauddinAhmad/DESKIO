//! Things that extend other software rather than standing on their own:
//! browser extensions, screen savers, settings panes, internet plugins,
//! widgets — and the installers left in Downloads after they were used.
//!
//! Removal goes through the same plan-and-review path as everything else; this
//! module only finds things.

use crate::fsutil;
use crate::model::{Confidence, Leftover, LeftoverKind};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCategory {
    InstallationFiles,
    BrowserExtension,
    ScreenSaver,
    SettingsPane,
    InternetPlugin,
    Widget,
}

impl ExtensionCategory {
    pub fn label(self) -> &'static str {
        match self {
            ExtensionCategory::InstallationFiles => "Installation Files",
            ExtensionCategory::BrowserExtension => "Web Browser Extensions",
            ExtensionCategory::ScreenSaver => "Screen Savers",
            ExtensionCategory::SettingsPane => "Settings Panes",
            ExtensionCategory::InternetPlugin => "Internet Plugins",
            ExtensionCategory::Widget => "Widgets",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ExtensionCategory::InstallationFiles => {
                "Disk images and installer packages in your Downloads folder. \
                 The apps they installed are unaffected."
            }
            ExtensionCategory::BrowserExtension => "Add-ons installed into your browsers.",
            ExtensionCategory::ScreenSaver => "Screen savers installed on this Mac.",
            ExtensionCategory::SettingsPane => "Panes added to System Settings by other software.",
            ExtensionCategory::InternetPlugin => "Plugins loaded by browsers and other apps.",
            ExtensionCategory::Widget => "Widgets installed by other software.",
        }
    }

    /// Every category, in the order the UI shows them.
    pub fn all() -> [ExtensionCategory; 6] {
        [
            ExtensionCategory::InstallationFiles,
            ExtensionCategory::BrowserExtension,
            ExtensionCategory::ScreenSaver,
            ExtensionCategory::SettingsPane,
            ExtensionCategory::InternetPlugin,
            ExtensionCategory::Widget,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionItem {
    pub id: String,
    pub name: String,
    pub category: ExtensionCategory,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub size_unknown: bool,
    pub is_directory: bool,
    pub requires_admin: bool,
    /// Extra context for the row — the browser it belongs to, the version of an
    /// installer, and so on.
    pub detail: Option<String>,
}

impl ExtensionItem {
    /// As a removal-plan line, so this section reuses the same review sheet and
    /// the same safety checks as everything else.
    pub fn to_leftover(&self) -> Leftover {
        Leftover {
            path: self.path.clone(),
            name: self.name.clone(),
            size_bytes: self.size_bytes,
            size_unknown: self.size_unknown,
            is_directory: self.is_directory,
            kind: match self.category {
                ExtensionCategory::InstallationFiles => LeftoverKind::Other,
                _ => LeftoverKind::Extension,
            },
            // Never pre-ticked. Unlike a leftover, every one of these is
            // something the user installed on purpose and may still want.
            confidence: Confidence::Medium,
            reason: self
                .detail
                .clone()
                .unwrap_or_else(|| self.category.label().to_string()),
            requires_admin: self.requires_admin,
            shared_with: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionGroup {
    pub category: ExtensionCategory,
    pub label: String,
    pub description: String,
    pub items: Vec<ExtensionItem>,
    pub size_bytes: u64,
}

/// The human name of a Chromium extension, from the newest version's manifest.
pub(crate) fn chromium_extension_name(ext_dir: &Path) -> Option<String> {
    let version_dir = fsutil::children(ext_dir)
        .into_iter()
        .filter(|p| p.is_dir())
        .max_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))?;

    let manifest = std::fs::read_to_string(version_dir.join("manifest.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    let name = json.get("name")?.as_str()?.to_string();

    // Localised manifests store a placeholder and keep the real string in
    // _locales; resolve it so the user sees a name rather than `__MSG_appName__`.
    if let Some(key) = name
        .strip_prefix("__MSG_")
        .and_then(|s| s.strip_suffix("__"))
    {
        let default_locale = json
            .get("default_locale")
            .and_then(|v| v.as_str())
            .unwrap_or("en");
        for locale in [default_locale, "en", "en_US"] {
            let path = version_dir
                .join("_locales")
                .join(locale)
                .join("messages.json");
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(messages) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(m) = messages
                .get(key)
                .and_then(|v| v.get("message"))
                .and_then(|v| v.as_str())
            {
                return Some(m.to_string());
            }
        }
        return None;
    }
    Some(name)
}

/// Everything, grouped by category. Empty categories are still returned, so the
/// UI can show "No items" rather than making a category disappear.
pub fn list() -> Vec<ExtensionGroup> {
    let items = imp::list();
    ExtensionCategory::all()
        .into_iter()
        .map(|category| {
            let mut items: Vec<ExtensionItem> = items
                .iter()
                .filter(|i| i.category == category)
                .cloned()
                .collect();
            items.sort_by_key(|i| std::cmp::Reverse(i.size_bytes));
            ExtensionGroup {
                category,
                label: category.label().to_string(),
                description: category.description().to_string(),
                size_bytes: items.iter().map(|i| i.size_bytes).sum(),
                items,
            }
        })
        .collect()
}
