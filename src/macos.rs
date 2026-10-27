//! Desktop integration on macOS. Homebrew and the install script only
//! place a command-line program, so on startup the app adds
//! `~/Applications/Samādhi.app`, a small bundle that launches it. That
//! makes it show up in Launchpad, Spotlight and Finder, with its icon, and
//! lets it be kept in the Dock. Files are only written when they change.

use std::fs;
use std::path::{Path, PathBuf};

const BUNDLE_ID: &str = "io.github.anirudhi.samadhi";
/// Icon sizes for the `.icns` file, by their four-letter type.
const ICONS: [(&[u8; 4], &[u8]); 4] = [
    (
        b"ic07",
        include_bytes!("../assets/icon/hicolor/128x128/apps/samadhi.png"),
    ),
    (
        b"ic08",
        include_bytes!("../assets/icon/hicolor/256x256/apps/samadhi.png"),
    ),
    (
        b"ic09",
        include_bytes!("../assets/icon/hicolor/512x512/apps/samadhi.png"),
    ),
    (b"ic10", include_bytes!("../assets/icon/samadhi.png")),
];

/// Adds the app bundle for the running program, unless it is a development
/// build. Errors are logged, never fatal.
pub fn ensure_app_bundle() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if exe.components().any(|c| c.as_os_str() == "target") {
        return; // `cargo run` and tests: don't touch the real Applications.
    }
    let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()) else {
        return;
    };
    let bundle = home
        .join("Applications")
        .join(format!("{}.app", crate::APP_NAME));
    match write_bundle(&bundle, &exe) {
        Ok(true) => register(&bundle),
        Ok(false) => {}
        Err(err) => log::warn!("could not add Samādhi to Applications: {err:#}"),
    }
}

/// Tells Launch Services about a new or changed bundle, so Launchpad and
/// Finder show it (and its icon) straight away.
fn register(bundle: &Path) {
    const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = std::process::Command::new(LSREGISTER)
        .arg("-f")
        .arg(bundle)
        .status();
}

/// Writes the bundle at `bundle` to launch `exe`. Returns whether anything
/// changed.
pub fn write_bundle(bundle: &Path, exe: &Path) -> anyhow::Result<bool> {
    let contents = bundle.join("Contents");
    let launcher = format!(
        "#!/bin/sh\nexec \"{}\" \"$@\"\n",
        launch_target(exe).display()
    );
    let mut changed = false;
    changed |= write_if_changed(&contents.join("Info.plist"), info_plist().as_bytes())?;
    changed |= write_if_changed(&contents.join("Resources/samadhi.icns"), &icns())?;
    let script = contents.join("MacOS/samadhi");
    changed |= write_if_changed(&script, launcher.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    }
    Ok(changed)
}

/// The program to launch: Homebrew's stable `bin` link rather than the
/// versioned copy in its Cellar, so the bundle keeps working after
/// `brew upgrade`.
fn launch_target(exe: &Path) -> PathBuf {
    let parts: Vec<_> = exe.components().collect();
    match parts.iter().position(|c| c.as_os_str() == "Cellar") {
        Some(i) => parts[..i]
            .iter()
            .collect::<PathBuf>()
            .join("bin")
            .join(crate::APP_ID),
        None => exe.to_path_buf(),
    }
}

fn info_plist() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let name = crate::APP_NAME;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>{name}</string>
	<key>CFBundleDisplayName</key>
	<string>{name}</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleExecutable</key>
	<string>samadhi</string>
	<key>CFBundleIconFile</key>
	<string>samadhi</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.productivity</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// An Apple icon file: a header, then each image as a typed chunk. Modern
/// macOS reads PNG data directly from these chunk types.
fn icns() -> Vec<u8> {
    let chunk_len = |png: &[u8]| u32::try_from(png.len() + 8).expect("icon fits");
    let total = 8 + ICONS.iter().map(|(_, png)| chunk_len(png)).sum::<u32>();
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&total.to_be_bytes());
    for (kind, png) in ICONS {
        out.extend_from_slice(kind);
        out.extend_from_slice(&chunk_len(png).to_be_bytes());
        out.extend_from_slice(png);
    }
    out
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> anyhow::Result<bool> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    crate::document::write_atomically(path, bytes)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homebrew_installs_launch_through_the_stable_link() {
        assert_eq!(
            launch_target(Path::new("/opt/homebrew/Cellar/samadhi/0.2.0/bin/samadhi")),
            Path::new("/opt/homebrew/bin/samadhi")
        );
        let plain = Path::new("/Users/me/.local/bin/samadhi");
        assert_eq!(launch_target(plain), plain);
    }

    #[test]
    fn writes_a_bundle_that_launches_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Samādhi.app");
        let exe = Path::new("/usr/local/Cellar/samadhi/0.2.0/bin/samadhi");
        assert!(write_bundle(&bundle, exe).unwrap());

        let script = fs::read_to_string(bundle.join("Contents/MacOS/samadhi")).unwrap();
        assert_eq!(
            script,
            "#!/bin/sh\nexec \"/usr/local/bin/samadhi\" \"$@\"\n"
        );
        let plist = fs::read_to_string(bundle.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains("<string>Samādhi</string>"));
        assert!(plist.contains(BUNDLE_ID));

        let icns = fs::read(bundle.join("Contents/Resources/samadhi.icns")).unwrap();
        assert_eq!(&icns[..4], b"icns");
        let len = u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize;
        assert_eq!(len, icns.len());
        assert_eq!(&icns[8..12], b"ic07");
        assert_eq!(&icns[16..20], b"\x89PNG");

        assert!(
            !write_bundle(&bundle, exe).unwrap(),
            "second run is a no-op"
        );
    }
}
