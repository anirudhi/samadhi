//! Resizes source photographs to 2560 px wide JPEGs for bundling (this also
//! strips camera metadata). Usage:
//!
//! ```sh
//! cargo run --release --example prepare_photos -- <folder of .jpg files>
//! ```
//!
//! Output goes to `assets/photos/`. Sources and licences: `assets/photos/CREDITS.md`.

use std::path::Path;

const WIDTH: u32 = 2560;
const QUALITY: u8 = 84;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("usage: prepare_photos <folder>")?;
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/photos");
    std::fs::create_dir_all(&out_dir)?;
    let mut entries: Vec<_> = std::fs::read_dir(&input)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg")))
        .collect();
    entries.sort();
    for path in entries {
        let img = image::open(&path)?;
        let height = (u64::from(img.height()) * u64::from(WIDTH) / u64::from(img.width())) as u32;
        let resized = img
            .resize_exact(WIDTH, height, image::imageops::FilterType::Lanczos3)
            .into_rgb8();
        let out = out_dir.join(path.file_name().ok_or("bad file name")?);
        let mut file = std::io::BufWriter::new(std::fs::File::create(&out)?);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, QUALITY)
            .encode_image(&resized)?;
        drop(file);
        println!(
            "{} -> {}x{} ({} KB)",
            out.display(),
            WIDTH,
            height,
            std::fs::metadata(&out)?.len() / 1024
        );
    }
    Ok(())
}
