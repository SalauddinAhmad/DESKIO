//! Windows extensions.
//!
//! Compile-verified for the Windows target, not yet run on Windows.

use super::{ExtensionCategory, ExtensionItem};
use crate::fsutil;
use crate::safety;
use std::path::{Path, PathBuf};

pub fn list() -> Vec<ExtensionItem> {
    let mut out = Vec::new();
    out.extend(installers());
    out.extend(browser_extensions());
    out
}

/// Installers left in Downloads. The safety layer permits only these shapes,
/// so anything it would refuse is not listed either.
fn installers() -> Vec<ExtensionItem> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    fsutil::children(&home.join("Downloads"))
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
                    "msi" => "Installer package in Downloads".to_string(),
                    "exe" => "Installer in Downloads".to_string(),
                    "iso" => "Disc image in Downloads".to_string(),
                    other => format!("{} file in Downloads", other.to_uppercase()),
                }),
                path,
            })
        })
        .collect()
}

fn browser_extensions() -> Vec<ExtensionItem> {
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from);
    let mut out = Vec::new();

    let chromium: [(&str, PathBuf); 3] = [
        ("Google Chrome", local.join(r"Google\Chrome\User Data")),
        ("Microsoft Edge", local.join(r"Microsoft\Edge\User Data")),
        (
            "Brave",
            local.join(r"BraveSoftware\Brave-Browser\User Data"),
        ),
    ];

    for (browser, root) in chromium {
        for profile in profile_dirs(&root) {
            for ext_dir in fsutil::children(&profile.join("Extensions")) {
                let Some(id) = ext_dir.file_name().map(|n| n.to_string_lossy().to_string()) else {
                    continue;
                };
                if id.starts_with('.') || safety::check_removable(&ext_dir).is_err() {
                    continue;
                }
                let (size, size_unknown) = fsutil::size_with_confidence(&ext_dir);
                out.push(ExtensionItem {
                    id: ext_dir.to_string_lossy().to_string(),
                    name: super::chromium_extension_name(&ext_dir).unwrap_or_else(|| id.clone()),
                    category: ExtensionCategory::BrowserExtension,
                    size_bytes: size,
                    size_unknown,
                    is_directory: true,
                    requires_admin: false,
                    detail: Some(format!("{browser} extension")),
                    path: ext_dir,
                });
            }
        }
    }

    if let Some(roaming) = roaming {
        for profile in fsutil::children(&roaming.join(r"Mozilla\Firefox\Profiles")) {
            for xpi in fsutil::children(&profile.join("extensions")) {
                if xpi.extension().and_then(|e| e.to_str()) != Some("xpi") {
                    continue;
                }
                if safety::check_removable(&xpi).is_err() {
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
    }

    out
}

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
