//! Persistent user preferences, stored as human-editable TOML in the XDG
//! config directory (`~/.config/samadhi/settings.toml`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

pub const DEFAULT_FONT_SIZE: f32 = 16.0;
pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 96.0;

/// The writing area, in logical points relative to the window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // independent on/off preferences
pub struct Settings {
    /// `None` until the user moves or resizes the page; a centred default is
    /// then computed from the screen size.
    pub page: Option<PageRect>,
    /// System font family; `None` uses the built-in font.
    pub font_family: Option<String>,
    pub font_size: f32,
    /// Opacity of the sheet behind the text (0 = invisible).
    pub page_opacity: f32,
    /// Name of the theme (a built-in landscape or one of the user's images).
    pub theme: Option<String>,
    /// Parallax and ambient animation; turn off to reduce motion.
    pub motion: bool,
    pub music: Option<String>,
    pub music_muted: bool,
    pub music_volume: f32,
    pub click: Option<String>,
    pub click_muted: bool,
    pub click_volume: f32,
    /// Style markdown live in the editor.
    pub markdown: bool,
    /// Underline misspelled words.
    pub spell_check: bool,
    /// The document open when the app was last closed; reopened at start.
    pub last_file: Option<PathBuf>,
    /// Where new documents are created (default: a `Samadhi` folder in
    /// Google Drive when it is installed, else `~/Documents/Samādhi`).
    pub documents_dir: Option<PathBuf>,
    /// Extension for new documents (`md` by default).
    pub default_extension: String,
    /// Length of the focus timer, in minutes.
    pub timer_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            page: None,
            font_family: None,
            font_size: DEFAULT_FONT_SIZE,
            page_opacity: 0.0,
            theme: None,
            motion: true,
            music: None,
            music_muted: false,
            music_volume: 0.6,
            click: None,
            click_muted: false,
            click_volume: 0.7,
            markdown: true,
            spell_check: false,
            last_file: None,
            documents_dir: None,
            default_extension: "md".to_owned(),
            timer_minutes: crate::timer::DEFAULT_MINUTES,
        }
    }
}

impl Settings {
    /// The folder new documents go in.
    pub fn documents_dir(&self) -> PathBuf {
        if let Some(dir) = &self.documents_dir {
            return dir.clone();
        }
        let Some(user) = directories::UserDirs::new() else {
            return PathBuf::from(".");
        };
        if let Some(drive) = google_drive_folder(user.home_dir()) {
            return drive;
        }
        user.document_dir()
            .unwrap_or_else(|| user.home_dir())
            .join(crate::APP_NAME)
    }

    /// Location of the settings file, if the platform has a config dir.
    pub fn default_path() -> Option<PathBuf> {
        crate::project_dirs().map(|d| d.config_dir().join("settings.toml"))
    }

    /// Loads settings, falling back to defaults (with a warning) if the file
    /// is missing or unreadable so a bad config never prevents writing.
    pub fn load_or_default(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        match fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(settings) => settings.sanitized(),
                Err(err) => {
                    log::warn!("ignoring malformed settings at {}: {err}", path.display());
                    Self::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                log::warn!("cannot read settings at {}: {err}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = toml::to_string_pretty(self).context("serialising settings")?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating config dir {}", dir.display()))?;
        }
        crate::document::write_atomically(path, text.as_bytes())
    }

    /// Clamps values that a hand-edited file could have made nonsensical.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        let finite_or = |v: f32, d: f32| if v.is_finite() { v } else { d };
        self.font_size =
            finite_or(self.font_size, DEFAULT_FONT_SIZE).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.page_opacity = finite_or(self.page_opacity, 0.0).clamp(0.0, 1.0);
        self.music_volume = finite_or(self.music_volume, 0.6).clamp(0.0, 1.0);
        self.click_volume = finite_or(self.click_volume, 0.7).clamp(0.0, 1.0);
        self.default_extension = self
            .default_extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        self.timer_minutes = self
            .timer_minutes
            .clamp(crate::timer::MIN_MINUTES, crate::timer::MAX_MINUTES);
        if self.default_extension.is_empty() {
            "md".clone_into(&mut self.default_extension);
        }
        if let Some(p) = self.page {
            let ok = [p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite());
            if !ok || p.width <= 0.0 || p.height <= 0.0 {
                self.page = None;
            }
        }
        self
    }
}

/// A `Samadhi` folder inside Google Drive, if Drive is synced to this
/// computer, so writing is backed up and shared between devices with no
/// setup. It goes in a `Writing` folder when the Drive has one.
///
/// Looks where Google Drive for desktop keeps it on macOS
/// (`~/Library/CloudStorage/GoogleDrive-<account>/My Drive`), its older
/// locations, and the usual places for a Drive mounted on Linux.
fn google_drive_folder(home: &Path) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = fs::read_dir(home.join("Library/CloudStorage"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("GoogleDrive-"))
        .map(|e| e.path().join("My Drive"))
        .collect();
    roots.sort();
    roots.extend(
        ["Google Drive/My Drive", "Google Drive", "GoogleDrive"]
            .iter()
            .map(|p| home.join(p)),
    );
    let root = roots.into_iter().find(|r| r.is_dir())?;
    let writing = root.join("Writing");
    // ASCII, unlike the app's name: macOS and Linux store accented file
    // names differently, which can make sync tools see two folders.
    Some(if writing.is_dir() { writing } else { root }.join("Samadhi"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.toml");
        let settings = Settings {
            page: Some(PageRect {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 400.0,
            }),
            font_family: Some("Serif".into()),
            theme: Some("Night Sky".into()),
            motion: false,
            music_muted: true,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load_or_default(Some(&path)), settings);
    }

    #[test]
    fn partial_and_malformed_files_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");

        fs::write(&path, "font_size = 30.0\n").unwrap();
        let s = Settings::load_or_default(Some(&path));
        assert!((s.font_size - 30.0).abs() < f32::EPSILON);
        assert_eq!(s.default_extension, "md");

        fs::write(&path, "this is = = not toml").unwrap();
        assert_eq!(Settings::load_or_default(Some(&path)), Settings::default());

        let missing = dir.path().join("missing.toml");
        assert_eq!(
            Settings::load_or_default(Some(&missing)),
            Settings::default()
        );
    }

    #[test]
    fn documents_go_to_google_drive_when_it_is_installed() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        assert_eq!(google_drive_folder(home), None);

        let linux = home.join("GoogleDrive");
        fs::create_dir_all(&linux).unwrap();
        assert_eq!(google_drive_folder(home), Some(linux.join("Samadhi")));

        // Google Drive for desktop on a Mac wins, and a Writing folder is used.
        let mac = home.join("Library/CloudStorage/GoogleDrive-me@example.com/My Drive");
        fs::create_dir_all(mac.join("Writing")).unwrap();
        assert_eq!(google_drive_folder(home), Some(mac.join("Writing/Samadhi")));
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let s = Settings {
            font_size: 1000.0,
            page_opacity: -3.0,
            music_volume: f32::NAN,
            default_extension: " .TXT ".into(),
            page: Some(PageRect {
                x: 0.0,
                y: 0.0,
                width: -5.0,
                height: 10.0,
            }),
            ..Settings::default()
        }
        .sanitized();
        assert!((s.font_size - MAX_FONT_SIZE).abs() < f32::EPSILON);
        assert!(s.page_opacity.abs() < f32::EPSILON);
        assert!((s.music_volume - 0.6).abs() < f32::EPSILON);
        assert_eq!(s.default_extension, "txt");
        assert_eq!(s.page, None);
    }
}
