#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, OnceLock, mpsc};

use eframe::{egui, egui_wgpu, wgpu};
use samadhi::app::{Options, SamadhiApp};
use samadhi::instance::{self, Claim, Message};
use samadhi::{APP_ID, APP_NAME};

/// Set when Samādhi restarts itself on OpenGL because Vulkan failed.
const GL_FALLBACK: &str = "SAMADHI_GL_FALLBACK";

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
    let log_file = init_logging();

    let mut file: Option<PathBuf> = None;
    for arg in std::env::args_os().skip(1) {
        match arg.to_str() {
            Some("-h" | "--help") => {
                println!("{USAGE}");
                if let Some(path) = log_file {
                    println!("\nLog: {}", path.display());
                }
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

    // Only one copy runs: a later launch hands over its file and exits.
    let request = match &file {
        Some(path) => Message::Open(std::path::absolute(path).unwrap_or_else(|_| path.clone())),
        None => Message::Show,
    };
    let instance = match instance::claim(&instance::default_socket(), &request) {
        Ok(Claim::HandedOver) => return ExitCode::SUCCESS,
        Ok(Claim::First(instance)) => Some(instance),
        Err(err) => {
            log::warn!("running without the single-instance check: {err}");
            None
        }
    };
    // Requests from later launches, and the app to wake when one arrives.
    let (messages_tx, messages) = mpsc::channel();
    let app_ctx: Arc<OnceLock<egui::Context>> = Arc::default();
    let guard = instance.map(|instance| {
        let app_ctx = Arc::clone(&app_ctx);
        instance.serve(move |message| {
            let _ = messages_tx.send(message);
            if let Some(ctx) = app_ctx.get() {
                ctx.request_repaint();
            }
        })
    });

    #[cfg(target_os = "linux")]
    samadhi::desktop::ensure_launcher();
    #[cfg(target_os = "macos")]
    samadhi::macos::ensure_app_bundle();

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
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let fallback = std::env::var_os(GL_FALLBACK).is_some();
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup
        && wgpu::Backends::from_env().is_none()
    {
        // Vulkan (Metal on macOS) directly: probing OpenGL as well costs a
        // noticeable part of startup. If that fails we restart on OpenGL.
        setup.instance_descriptor.backends = if fallback {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::PRIMARY
        };
    }

    let result = eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let _ = app_ctx.set(cc.egui_ctx.clone());
            let options = Options {
                file,
                messages: Some(messages),
                watchdog: samadhi::watchdog::Watchdog::spawn(),
                ..Options::default()
            };
            Ok(Box::new(SamadhiApp::new(cc, options)))
        }),
    );
    drop(guard);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if !fallback => {
            log::warn!("starting with Vulkan/Metal failed ({err}); trying OpenGL");
            restart_on_gl()
        }
        Err(err) => {
            eprintln!("{APP_NAME} failed to start: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Runs this program again on the OpenGL renderer (the event loop can only
/// be created once per process).
fn restart_on_gl() -> ExitCode {
    let status = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env(GL_FALLBACK, "1")
            .status()
    });
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
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

/// Logs warnings (and Samādhi's own notes) to the terminal when there is
/// one, and always to a log file, so a freeze or crash in normal use leaves
/// a trace. The file starts afresh once it passes a megabyte. Returns its
/// path.
fn init_logging() -> Option<PathBuf> {
    use std::io::{IsTerminal as _, Write};

    /// Writes to the file and, when attached to a terminal, stderr too.
    struct Tee {
        file: Option<std::fs::File>,
        terminal: bool,
    }
    impl Write for Tee {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Some(file) = &mut self.file {
                let _ = file.write_all(buf);
            }
            if self.terminal {
                let _ = std::io::stderr().write_all(buf);
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if let Some(file) = &mut self.file {
                file.flush()?;
            }
            Ok(())
        }
    }

    let path = samadhi::project_dirs().map(|d| {
        d.state_dir()
            .unwrap_or_else(|| d.data_local_dir())
            .join("samadhi.log")
    });
    let file = path.as_ref().and_then(|path| {
        std::fs::create_dir_all(path.parent()?).ok()?;
        let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > 1 << 20);
        std::fs::OpenOptions::new()
            .create(true)
            .append(!too_big)
            .write(true)
            .truncate(too_big)
            .open(path)
            .ok()
    });
    let tee = Tee {
        file,
        terminal: std::io::stderr().is_terminal(),
    };
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,samadhi=info"),
    )
    .target(env_logger::Target::Pipe(Box::new(tee)))
    .init();
    log::info!("Samādhi {} starting", env!("CARGO_PKG_VERSION"));
    path
}
