//! Renders the app icon at every size desktops expect, from the 1024 px
//! master `assets/icon/samadhi.png`.
//!
//! ```sh
//! cargo run --release --example render_icons                  # from the master
//! cargo run --release --example render_icons -- new-logo.png  # replace the master first
//! ```
//!
//! A new logo of any size or aspect ratio is centred on a square,
//! transparent canvas with a small margin, so it is never stretched.

use std::path::Path;

use image::{RgbaImage, imageops::FilterType};

const SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];
const MASTER: u32 = 1024;
/// Margin on each side, as a fraction of the icon, so the artwork doesn't
/// touch the edges (like most desktop app icons).
const MARGIN: f32 = 0.04;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon");
    let master_path = root.join("samadhi.png");

    if let Some(source) = std::env::args().nth(1) {
        let logo = image::open(&source)?.into_rgba8();
        let inner = (MASTER as f32 * (1.0 - 2.0 * MARGIN)) as u32;
        let scale = inner as f32 / logo.width().max(logo.height()) as f32;
        let (w, h) = (
            (logo.width() as f32 * scale).round() as u32,
            (logo.height() as f32 * scale).round() as u32,
        );
        let resized = image::imageops::resize(&logo, w, h, FilterType::Lanczos3);
        let mut master = RgbaImage::new(MASTER, MASTER);
        image::imageops::overlay(
            &mut master,
            &resized,
            i64::from((MASTER - w) / 2),
            i64::from((MASTER - h) / 2),
        );
        master.save(&master_path)?;
        println!("wrote master {MASTER}x{MASTER} from {source}");
    }

    let master = image::open(&master_path)?.into_rgba8();
    for size in SIZES {
        let icon = image::imageops::resize(&master, size, size, FilterType::Lanczos3);
        let dir = root.join(format!("hicolor/{size}x{size}/apps"));
        std::fs::create_dir_all(&dir)?;
        icon.save(dir.join("samadhi.png"))?;
        println!("wrote {size}x{size}");
    }
    Ok(())
}
