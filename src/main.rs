#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use samadhi::app::{Options, SamadhiApp};
use samadhi::{APP_ID, APP_NAME};

const USAGE: &str = "\
Usage: samadhi [OPTIONS] [FILE]

A calm, distraction-free, full-screen writing app. Opens FILE (any
plain-text or Markdown document), or else any unsaved draft left by a crash,
or else the last document you saved.

Options:
  -h, --help     Show this help
  -V, --version  Show the version

Shortcuts:
  Esc            Quit (asks about unsaved drafts)
  Ctrl+S         Save (names a new draft; saved files then autosave)
  Ctrl+Shift+S   Save as
  Ctrl+N         New document    Ctrl+O        Open
  Ctrl+M         Minimise
  Ctrl+= / Ctrl+-  Larger / smaller text";

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let mut file: Option<PathBuf> = None;
    for arg in std::env::args_os().skip(1) {
        match arg.to_str() {
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("-V" | "--version") => {
                println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some(flag) if flag.starts_with('-') && flag.len() > 1 => {
                eprintln!("unknown option {flag}\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("only one file can be opened at a time\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        // On Wayland the compositor finds the icon through the app id and
        // the installed `samadhi.desktop` file.
        .with_app_id(APP_ID)
        // Always borderless full screen: no title bar or window chrome.
        .with_fullscreen(true)
        .with_decorations(false);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let options = Options {
                file,
                ..Options::default()
            };
            Ok(Box::new(SamadhiApp::new(cc, options)))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{APP_NAME} failed to start: {err}");
            ExitCode::FAILURE
        }
    }
}

/// The app icon for window managers that take it from the window (X11).
fn window_icon() -> Option<egui::IconData> {
    let png = include_bytes!("../assets/icon/hicolor/256x256/apps/samadhi.png");
    let image = image::load_from_memory(png).ok()?.into_rgba8();
    Some(egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
