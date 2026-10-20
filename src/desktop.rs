//! Desktop integration on Linux: the app adds itself to the application
//! menu (launcher entry and icons) on startup, so installs that only place
//! the program (the release installer, Homebrew, `cargo install`) still
//! show up like any other app. Files are only written when they change.

use std::fs;
use std::path::{Path, PathBuf};

const DESKTOP_TEMPLATE: &str = include_str!("../packaging/samadhi.desktop");
const ICONS: [(u32, &[u8]); 4] = [
    (
        48,
        include_bytes!("../assets/icon/hicolor/48x48/apps/samadhi.png"),
    ),
    (
        128,
        include_bytes!("../assets/icon/hicolor/128x128/apps/samadhi.png"),
    ),
    (
        256,
        include_bytes!("../assets/icon/hicolor/256x256/apps/samadhi.png"),
    ),
    (
        512,
        include_bytes!("../assets/icon/hicolor/512x512/apps/samadhi.png"),
    ),
];

/// Registers the running program in the application menu, unless it is a
/// development build. Errors are logged, never fatal.
pub fn ensure_launcher() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if exe.components().any(|c| c.as_os_str() == "target") {
        return; // `cargo run` and tests: don't touch the real menu.
    }
    let Some(data) = data_home() else {
        return;
    };
    if let Err(err) = write_launcher(&data, &exe) {
        log::warn!("could not add Samādhi to the application menu: {err:#}");
    }
}

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".local/share")))
}

/// Writes the icons and a launcher entry for `exe` under `data`.
pub fn write_launcher(data: &Path, exe: &Path) -> anyhow::Result<()> {
    let mut icon_256 = PathBuf::new();
    for (size, bytes) in ICONS {
        let path = data.join(format!("icons/hicolor/{size}x{size}/apps/samadhi.png"));
        write_if_changed(&path, bytes)?;
        if size == 256 {
            icon_256 = path;
        }
    }
    let entry: String = DESKTOP_TEMPLATE
        .lines()
        .map(|line| {
            if line.starts_with("Exec=") {
                format!("Exec=\"{}\" %f", exe.display())
            } else if line.starts_with("Icon=") {
                // A full path works in every launcher, even ones that don't
                // search the per-user icon theme.
                format!("Icon={}", icon_256.display())
            } else {
                line.to_owned()
            }
        })
        .map(|line| line + "\n")
        .collect();
    write_if_changed(&data.join("applications/samadhi.desktop"), entry.as_bytes())
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    crate::document::write_atomically(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_launcher_pointing_at_the_running_program() {
        let dir = tempfile::tempdir().unwrap();
        let exe = Path::new("/opt/some place/samadhi");
        write_launcher(dir.path(), exe).unwrap();

        let entry = fs::read_to_string(dir.path().join("applications/samadhi.desktop")).unwrap();
        assert!(entry.contains("Exec=\"/opt/some place/samadhi\" %f"));
        assert!(entry.contains("Name=Samādhi"));
        let icon = dir.path().join("icons/hicolor/256x256/apps/samadhi.png");
        assert!(entry.contains(&format!("Icon={}", icon.display())));
        assert!(icon.is_file());

        // A second run changes nothing.
        let before = fs::metadata(dir.path().join("applications/samadhi.desktop"))
            .unwrap()
            .modified()
            .unwrap();
        write_launcher(dir.path(), exe).unwrap();
        let after = fs::metadata(dir.path().join("applications/samadhi.desktop"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after);
    }
}
