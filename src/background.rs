//! Loading theme scenes (built-in landscapes or the user's own images) off
//! the UI thread.

use std::path::Path;
use std::sync::mpsc;

use anyhow::Context as _;
use eframe::egui::{self, ColorImage, Rect, Vec2, pos2};

use crate::catalog;
use crate::scene::{self, Scene, ThemeKind};

pub type ThemeSource = catalog::Source<ThemeKind>;

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "bmp", "gif", "webp", "tif", "tiff", "pbm", "pgm", "ppm",
];

/// Images larger than this (on their longest side) are downscaled on load to
/// keep GPU memory reasonable; it is still sharp on a 4K screen.
const MAX_IMAGE_SIDE: u32 = 3840;

/// Decodes an image file, downscaling very large images.
pub fn load_image(path: &Path) -> anyhow::Result<ColorImage> {
    let img = image::ImageReader::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("reading {}", path.display()))?
        .decode()
        .with_context(|| format!("decoding {}", path.display()))?;
    let img = if img.width().max(img.height()) > MAX_IMAGE_SIDE {
        img.resize(
            MAX_IMAGE_SIDE,
            MAX_IMAGE_SIDE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

/// The part of an image (in UV space) to show so it covers `target` without
/// distortion, cropping the overflow equally on both sides. The original app
/// stretched images, which distorted them on non-matching screens.
pub fn cover_uv(image: Vec2, target: Vec2) -> Rect {
    if image.x <= 0.0 || image.y <= 0.0 || target.x <= 0.0 || target.y <= 0.0 {
        return Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    }
    let image_aspect = image.x / image.y;
    let target_aspect = target.x / target.y;
    if image_aspect > target_aspect {
        let visible = target_aspect / image_aspect;
        let margin = (1.0 - visible) / 2.0;
        Rect::from_min_max(pos2(margin, 0.0), pos2(1.0 - margin, 1.0))
    } else {
        let visible = image_aspect / target_aspect;
        let margin = (1.0 - visible) / 2.0;
        Rect::from_min_max(pos2(0.0, margin), pos2(1.0, 1.0 - margin))
    }
}

/// Thumbnail size in pixels (drawn at half this in points, so crisp on
/// high-DPI screens).
pub const THUMB_PX: [usize; 2] = [92, 52];

/// Builds a preview of every theme on a worker thread, sending each as
/// `(index, image)` as soon as it is ready.
pub fn thumbnails(
    sources: Vec<ThemeSource>,
    ctx: &egui::Context,
) -> mpsc::Receiver<(usize, ColorImage)> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("theme-thumbnails".into())
        .spawn(move || {
            for (i, source) in sources.into_iter().enumerate() {
                let scene = match source {
                    catalog::Source::Builtin(kind) => kind.build(),
                    catalog::Source::File(path) => match load_image(&path) {
                        Ok(image) => scene::from_image(image),
                        Err(_) => continue,
                    },
                };
                if tx
                    .send((i, scene.thumbnail(THUMB_PX[0], THUMB_PX[1])))
                    .is_err()
                {
                    return;
                }
                ctx.request_repaint();
            }
        })
        .expect("spawning a thread should not fail");
    rx
}

/// A scene ready to show, with a frosted copy of each layer for the sheet.
#[derive(Debug)]
pub struct Prepared {
    pub scene: Scene,
    pub frost: Vec<ColorImage>,
}

impl Prepared {
    fn new(scene: Scene) -> Self {
        let frost = scene
            .layers
            .iter()
            .map(|l| crate::frost::frost(&l.image, l.drift != 0.0))
            .collect();
        Self { scene, frost }
    }
}

type LoadResult = (u64, String, anyhow::Result<Prepared>);

/// Builds scenes on worker threads; only the most recent request wins, so
/// flicking quickly through themes never shows a stale one.
#[derive(Debug)]
pub struct SceneLoader {
    tx: mpsc::Sender<LoadResult>,
    rx: mpsc::Receiver<LoadResult>,
    generation: u64,
    pending: bool,
}

impl Default for SceneLoader {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            generation: 0,
            pending: false,
        }
    }
}

impl SceneLoader {
    pub fn request(&mut self, name: String, source: ThemeSource, ctx: &egui::Context) {
        self.generation += 1;
        self.pending = true;
        let generation = self.generation;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("scene-builder".into())
            .spawn(move || {
                let result = match source {
                    catalog::Source::Builtin(kind) => Ok(kind.build()),
                    catalog::Source::File(path) => load_image(&path).map(scene::from_image),
                }
                .map(Prepared::new);
                // The receiver only disappears when the app is closing.
                let _ = tx.send((generation, name, result));
                ctx.request_repaint();
            })
            .expect("spawning a thread should not fail");
    }

    /// Returns the result of the latest request once it is ready.
    pub fn poll(&mut self) -> Option<(String, anyhow::Result<Prepared>)> {
        let mut latest = None;
        while let Ok((generation, name, result)) = self.rx.try_recv() {
            if generation == self.generation {
                latest = Some((name, result));
                self.pending = false;
            }
        }
        latest
    }

    /// Whether the latest request has not been collected by [`Self::poll`].
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Rect, b: Rect) -> bool {
        (a.min - b.min).length() < 1e-5 && (a.max - b.max).length() < 1e-5
    }

    #[test]
    fn cover_crops_wide_images_horizontally() {
        let uv = cover_uv(Vec2::new(2000.0, 1000.0), Vec2::new(1000.0, 1000.0));
        assert!(approx(
            uv,
            Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0))
        ));
    }

    #[test]
    fn cover_crops_tall_images_vertically() {
        let uv = cover_uv(Vec2::new(1000.0, 2000.0), Vec2::new(1000.0, 1000.0));
        assert!(approx(
            uv,
            Rect::from_min_max(pos2(0.0, 0.25), pos2(1.0, 0.75))
        ));
    }

    #[test]
    fn cover_with_matching_aspect_uses_whole_image() {
        let uv = cover_uv(Vec2::new(1920.0, 1080.0), Vec2::new(1280.0, 720.0));
        assert!(approx(
            uv,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
        ));
    }

    #[test]
    fn loads_and_downscales_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.png");
        image::RgbaImage::from_pixel(MAX_IMAGE_SIDE * 2, 10, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        let img = load_image(&path).unwrap();
        assert_eq!(img.size[0], MAX_IMAGE_SIDE as usize);
        assert!(load_image(&dir.path().join("missing.png")).is_err());
    }
}
