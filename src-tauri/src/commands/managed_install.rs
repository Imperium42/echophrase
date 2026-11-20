// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Detection of "managed" installs - installs owned by an external package
//! manager (Scoop, winget, Homebrew, Flathub, ...) rather than by our own
//! NSIS/DMG installer.
//!
//! A managed install must not self-update: the package manager tracks the
//! installed version, and an in-app update would silently desync it (and on
//! Scoop would be clobbered by the next `scoop update`). Rather than shipping
//! a second build per manager, the packaging step drops a marker file next to
//! the executable naming the manager; we read it at runtime.
//!
//! Marker: `.managed-install` in the executable's directory. Its (trimmed)
//! contents are the manager id, e.g. `scoop`. An empty file is treated as an
//! unknown-but-managed install.

use std::fs;
use std::path::PathBuf;

/// File name of the marker written by the packaging step.
const MARKER_FILE: &str = ".managed-install";

/// Describes how this build was installed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagedInstall {
    /// True when a package manager owns this install and self-update must be suppressed.
    pub managed: bool,
    /// Manager id from the marker file (`scoop`, `winget`, ...), if known.
    pub manager: Option<String>,
    /// Shell command the user should run to upgrade, if we know one for `manager`.
    pub upgrade_command: Option<String>,
}

impl Default for ManagedInstall {
    /// Unmanaged install: the in-app updater owns upgrades.
    fn default() -> Self {
        Self {
            managed: false,
            manager: None,
            upgrade_command: None,
        }
    }
}

/// Resolves the directory containing the running executable.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

/// Maps a manager id to the command that upgrades Echophrase through it.
fn upgrade_command_for(manager: &str) -> Option<String> {
    match manager {
        "scoop" => Some("scoop update echophrase".to_string()),
        "winget" => Some("winget upgrade Imperium42.Echophrase".to_string()),
        "homebrew" | "brew" => Some("brew upgrade --cask echophrase".to_string()),
        _ => None,
    }
}

/// Homebrew prefixes whose `Caskroom/echophrase` dir marks a cask-owned install
/// (Apple Silicon, then Intel). A marker file is not an option on macOS: the cask's
/// `app` stanza moves the signed bundle into /Applications, and writing inside it
/// would break the code signature. `brew uninstall` removes the Caskroom dir.
#[cfg(target_os = "macos")]
const HOMEBREW_CASKROOMS: &[&str] = &["/opt/homebrew/Caskroom", "/usr/local/Caskroom"];

/// True when any of `caskrooms` contains an `echophrase` cask dir.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn is_cask_installed(caskrooms: &[&str]) -> bool {
    caskrooms
        .iter()
        .any(|root| std::path::Path::new(root).join("echophrase").is_dir())
}

/// Builds the report for a known manager id.
fn managed_by(manager: &str) -> ManagedInstall {
    ManagedInstall {
        managed: true,
        manager: Some(manager.to_string()),
        upgrade_command: upgrade_command_for(manager),
    }
}

/// Reads the marker file (if any) and reports how this build was installed.
///
/// Any I/O error is treated as "not managed" - failing open keeps the normal
/// updater working rather than stranding regular users with no upgrade path.
pub fn detect() -> ManagedInstall {
    #[cfg(target_os = "macos")]
    if is_cask_installed(HOMEBREW_CASKROOMS) {
        return managed_by("homebrew");
    }

    let Some(dir) = exe_dir() else {
        return ManagedInstall::default();
    };
    let marker = dir.join(MARKER_FILE);
    let Ok(contents) = fs::read_to_string(&marker) else {
        return ManagedInstall::default();
    };

    let manager = contents.trim().to_lowercase();
    if manager.is_empty() {
        return ManagedInstall {
            managed: true,
            manager: None,
            upgrade_command: None,
        };
    }
    managed_by(&manager)
}

/// Tauri command: reports whether a package manager owns this install.
#[tauri::command]
pub fn get_managed_install() -> ManagedInstall {
    detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmanaged_by_default_in_test_binary() {
        // The test binary's dir has no marker, so detection must fail open.
        assert!(!detect().managed);
    }

    #[test]
    fn known_managers_map_to_upgrade_commands() {
        assert_eq!(
            upgrade_command_for("scoop").as_deref(),
            Some("scoop update echophrase")
        );
        assert!(upgrade_command_for("apt").is_none());
    }

    #[test]
    fn cask_detected_only_when_caskroom_dir_exists() {
        let root = std::env::temp_dir().join(format!("ep-caskroom-{}", std::process::id()));
        let root_str = root.to_string_lossy().to_string();
        assert!(!is_cask_installed(&[root_str.as_str()]));

        fs::create_dir_all(root.join("echophrase")).unwrap();
        assert!(is_cask_installed(&[root_str.as_str()]));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn homebrew_report_carries_upgrade_command() {
        let report = managed_by("homebrew");
        assert!(report.managed);
        assert_eq!(
            report.upgrade_command.as_deref(),
            Some("brew upgrade --cask echophrase")
        );
    }
}
