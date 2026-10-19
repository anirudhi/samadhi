//! Renders `assets/icon/samadhi.svg` to the PNG sizes desktop environments
//! expect. Run after editing the SVG:
//!
//! ```sh
//! cargo run --example render_icons
//! ```

use std::path::Path;

const SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon");
    let svg = std::fs::read(root.join("samadhi.svg"))?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())?;
    for size in SIZES {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("bad icon size")?;
        let scale = size as f32 / tree.size().width();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let dir = root.join(format!("hicolor/{size}x{size}/apps"));
        std::fs::create_dir_all(&dir)?;
        pixmap.save_png(dir.join("samadhi.png"))?;
        println!("wrote {size}x{size}");
    }
    Ok(())
}
