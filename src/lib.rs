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
pub mod icons;
pub mod markdown;
pub mod page;
pub mod recovery;
pub mod scene;
pub mod settings;
pub mod spell;
pub mod stage;
pub mod theme;
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
