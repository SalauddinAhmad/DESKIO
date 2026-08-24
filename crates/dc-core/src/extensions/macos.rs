//! Where macOS keeps extensions.

use super::{ExtensionCategory, ExtensionItem};
use crate::fsutil;
use crate::safety;
use std::path::{Path, PathBuf};

/// Bundle-style extensions live in matching pairs of user and system folders.
fn bundle_roots() -> Vec<(PathBuf, ExtensionCategory, &'static str)> {
    let mut v: Vec<(PathBuf, ExtensionCategory, &'static str)> = vec![
        (
            PathBuf::from("/Library/Screen Savers"),
            ExtensionCategory::ScreenSaver,
            "saver",
        ),
        (
            PathBuf::from("/Library/PreferencePanes"),
            ExtensionCategory::SettingsPane,
            "prefPane",
        ),
        (
            PathBuf::from("/Library/Internet Plug-Ins"),
            ExtensionCategory::InternetPlugin,
            "plugin",
        ),
        (
            PathBuf::from("/Library/Widgets"),
            ExtensionCategory::Widget,
            "wdgt",
        ),
    ];
    if let Some(home) = dirs::home_dir() {
        let lib = home.join("Library");
        v.extend([
            (
                lib.join("Screen Savers"),
                ExtensionCategory::ScreenSaver,
                "saver",
            ),
            (
                lib.join("PreferencePanes"),
                ExtensionCategory::SettingsPane,
                "prefPane",
            ),
            (
                lib.join("Internet Plug-Ins"),
                ExtensionCategory::InternetPlugin,
                "plugin",
            ),
            (lib.join("Widgets"), ExtensionCategory::Widget, "wdgt"),
        ]);
    }
    v
}

pub fn list() -> Vec<ExtensionItem> {
    let mut out = Vec::new();
    out.extend(bundles());
    out.extend(installers());
    out.extend(browser_extensions());
    out
}

fn bundles() -> Vec<ExtensionItem> {
    let mut out = Vec::new();
    for (root, category, ext) in bundle_roots() {
        for path in fsutil::children(&root) {
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
            let Some(name) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            // Apple's own are part of the system, not something to offer up.
            if safety::check_removable(&path).is_err() {
                continue;
            }
            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            out.push(ExtensionItem {
                id: path.to_string_lossy().to_string(),
                name,
                category,
                size_bytes: size,
                size_unknown,
                is_directory: path.is_dir(),
                requires_admin: safety::requires_admin(&path),
                detail: Some(format!("{} in {}", category.label(), root.display())),
                path,
            });
        }
    }
    out
}

/// Disk images and installer packages sitting in Downloads.
///
/// Only the shapes the safety layer will actually allow are listed — a regular
/// file directly in Downloads with an installer extension. Anything else in
/// there is none of this app's business.
fn installers() -> Vec<ExtensionItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let downloads = home.join("Downloads");

    fsutil::children(&downloads)
        .into_iter()
        .filter(|p| safety::check_removable(p).is_ok())
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().to_string();
            let (size, size_unknown) = fsutil::size_with_confidence(&path);
            let kind = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            Some(ExtensionItem {
                id: path.to_string_lossy().to_string(),
                name,
                category: ExtensionCategory::InstallationFiles,
                size_bytes: size,
                size_unknown,
                is_directory: false,
                requires_admin: false,
                detail: Some(match kind.as_str() {
                    "dmg" => "Disk image in Downloads".to_string(),
                    "pkg" | "mpkg" => "Installer package in Downloads".to_string(),
                    "iso" => "Disc image in Downloads".to_string(),
                    other => format!("{} file in Downloads", other.to_uppercase()),
                }),
                path,
            })
        })
        .collect()
}

/// Chromium-family and Firefox extensions.
///
/// These live inside the browser profile, which macOS protects: without Full
/// Disk Access the profile cannot be read at all and this comes back empty.
/// That is reported as an empty category rather than an error — the banner
/// already tells the user what to do about it.
fn browser_extensions() -> Vec<ExtensionItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let support = home.join("Library/Application Support");
    let mut out = Vec::new();

    let chromium: [(&str, PathBuf); 4] = [
        ("Google Chrome", support.join("Google/Chrome")),
        ("Microsoft Edge", support.join("Microsoft Edge")),
        ("Brave", support.join("BraveSoftware/Brave-Browser")),
        ("Opera", support.join("com.operasoftware.Opera")),
    ];

    for (browser, root) in chromium {
        for profile in profile_dirs(&root) {
            let ext_root = profile.join("Extensions");
            for ext_dir in fsutil::children(&ext_root) {
                let Some(id) = ext_dir.file_name().map(|n| n.to_string_lossy().to_string()) else {
                    continue;
                };
                if id.starts_with('.') {
                    continue;
                }
                let (size, size_unknown) = fsutil::size_with_confidence(&ext_dir);
                let name = super::chromium_extension_name(&ext_dir).unwrap_or_else(|| id.clone());
                out.push(ExtensionItem {
                    id: ext_dir.to_string_lossy().to_string(),
                    name,
                    category: ExtensionCategory::BrowserExtension,
                    size_bytes: size,
                    size_unknown,
                    is_directory: true,
                    requires_admin: safety::requires_admin(&ext_dir),
                    detail: Some(format!("{browser} extension")),
                    path: ext_dir,
                });
            }
        }
    }

    // Firefox keeps each add-on as a single .xpi named after its id.
    let firefox = support.join("Firefox/Profiles");
    for profile in fsutil::children(&firefox) {
        for xpi in fsutil::children(&profile.join("extensions")) {
            if xpi.extension().and_then(|e| e.to_str()) != Some("xpi") {
                continue;
            }
            let Some(name) = xpi.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            let (size, size_unknown) = fsutil::size_with_confidence(&xpi);
            out.push(ExtensionItem {
                id: xpi.to_string_lossy().to_string(),
                name,
                category: ExtensionCategory::BrowserExtension,
                size_bytes: size,
                size_unknown,
                is_directory: false,
                requires_admin: false,
                detail: Some("Firefox add-on".into()),
                path: xpi,
            });
        }
    }

    out.retain(|i| safety::check_removable(&i.path).is_ok());
    out
}

/// Chromium profile directories: `Default`, `Profile 1`, and so on.
fn profile_dirs(root: &Path) -> Vec<PathBuf> {
    fsutil::children(root)
        .into_iter()
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            p.is_dir() && (name == "Default" || name.starts_with("Profile "))
        })
        .collect()
}
