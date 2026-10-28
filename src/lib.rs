//! Samādhi: a calm, distraction-free, full-screen writing app with
//! ambient music, key-click sounds and soothing backgrounds.
//!
//! Samādhi (Pali: concentration, one-pointed attention) gives the page your
//! whole focus. Documents are plain text, saved as Markdown by default.

pub mod app;
pub mod audio;
pub mod background;
pub mod catalog;
#[cfg(target_os = "linux")]
pub mod desktop;
pub mod document;
pub mod fonts;
pub mod frost;
pub mod icons;
#[cfg(unix)]
pub mod instance;
#[cfg(any(target_os = "macos", test))]
pub mod macos;
pub mod markdown;
pub mod page;
pub mod recovery;
pub mod scene;
pub mod settings;
pub mod spell;
pub mod stage;
pub mod theme;
pub mod timer;
pub mod watchdog;
pub mod widgets;

use std::path::PathBuf;

pub const APP_NAME: &str = "Samādhi";
/// ASCII identifier used for the binary, app id and config folders.
pub const APP_ID: &str = "samadhi";

/// XDG directories: settings live in `~/.config/samadhi`, the user's
/// own backgrounds, music and clicks in `~/.local/share/samadhi`.
pub fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", APP_ID)
}

/// Folders scanned for user-supplied assets of the given kind
/// (`backgrounds`, `music` or `clicks`).
pub fn asset_dirs(kind: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = project_dirs() {
        dirs.push(d.data_dir().join(kind));
    }
    // Assets shipped next to the binary, e.g. in a portable install.
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        dirs.push(exe_dir.join("assets").join(kind));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_dirs_end_in_the_requested_kind() {
        for kind in ["backgrounds", "music", "clicks"] {
            let dirs = asset_dirs(kind);
            assert!(!dirs.is_empty(), "{kind}");
            for dir in &dirs {
                assert!(dir.ends_with(kind), "{}", dir.display());
            }
        }
        // The portable folder sits next to the running binary.
        let exe_dir = std::env::current_exe().unwrap();
        let portable = exe_dir.parent().unwrap().join("assets").join("music");
        assert!(asset_dirs("music").contains(&portable));
    }

    #[test]
    fn project_dirs_use_the_ascii_app_id() {
        if let Some(dirs) = project_dirs() {
            let config = dirs.config_dir().to_string_lossy().to_lowercase();
            assert!(config.contains(APP_ID), "{config}");
            assert!(APP_ID.is_ascii());
            assert!(asset_dirs("music").contains(&dirs.data_dir().join("music")));
        }
    }
}
