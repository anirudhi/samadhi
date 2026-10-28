//! Interface icons from [Lucide](https://lucide.dev) (ISC licence, see
//! `assets/icons/lucide/LICENSE`), rasterised from SVG at the exact pixel
//! size needed so they stay crisp at any display scale, then tinted.

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, ColorImage, Id, Painter, Rect, TextureHandle, TextureOptions, pos2,
};

/// Stroke width, a little finer than Lucide's default of 2 for a lighter look.
const STROKE_WIDTH: &str = "1.5";
/// Lucide icons are drawn on a 24×24 grid.
const GRID: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Font,
    Size,
    Picture,
    Music,
    Keyboard,
    Document,
    Previous,
    Next,
    SpeakerOn,
    SpeakerOff,
    New,
    Open,
    Save,
    SaveAs,
    Smaller,
    DefaultSize,
    Larger,
    Motion,
    Markdown,
    SpellCheck,
    Timer,
    Play,
    Pause,
}

impl Icon {
    pub const ALL: [Self; 23] = [
        Self::Font,
        Self::Size,
        Self::Picture,
        Self::Music,
        Self::Keyboard,
        Self::Document,
        Self::Previous,
        Self::Next,
        Self::SpeakerOn,
        Self::SpeakerOff,
        Self::New,
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Smaller,
        Self::DefaultSize,
        Self::Larger,
        Self::Motion,
        Self::Markdown,
        Self::SpellCheck,
        Self::Timer,
        Self::Play,
        Self::Pause,
    ];

    fn svg(self) -> &'static str {
        macro_rules! lucide {
            ($name:literal) => {
                include_str!(concat!("../assets/icons/lucide/", $name, ".svg"))
            };
        }
        match self {
            Self::Font => lucide!("type"),
            Self::Size => lucide!("a-large-small"),
            Self::Picture => lucide!("image"),
            Self::Music => lucide!("headphones"),
            Self::Keyboard => lucide!("keyboard"),
            Self::Document => lucide!("file-text"),
            Self::Previous => lucide!("chevron-left"),
            Self::Next => lucide!("chevron-right"),
            Self::SpeakerOn => lucide!("volume-2"),
            Self::SpeakerOff => lucide!("volume-x"),
            Self::New => lucide!("file-plus"),
            Self::Open => lucide!("folder-open"),
            Self::Save => lucide!("save"),
            Self::SaveAs => lucide!("file-pen"),
            Self::Smaller => lucide!("a-arrow-down"),
            Self::DefaultSize => lucide!("rotate-ccw"),
            Self::Larger => lucide!("a-arrow-up"),
            Self::Motion => lucide!("wind"),
            Self::Markdown => lucide!("heading"),
            Self::SpellCheck => lucide!("spell-check"),
            Self::Timer => lucide!("timer"),
            Self::Play => lucide!("play"),
            Self::Pause => lucide!("pause"),
        }
    }
}

/// Renders `icon` as a white-on-transparent mask `px` pixels square, so it
/// can be tinted any colour when drawn.
fn rasterize(icon: Icon, px: u32) -> Option<ColorImage> {
    let svg = icon.svg().replace("currentColor", "#ffffff").replace(
        "stroke-width=\"2\"",
        &format!("stroke-width=\"{STROKE_WIDTH}\""),
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / GRID;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA, exactly what egui wants.
    let pixels = pixmap
        .pixels()
        .iter()
        .map(|p| Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
        .collect();
    Some(ColorImage::new([px as usize, px as usize], pixels))
}

/// Rasterised icons, kept in egui's memory per pixel size.
#[derive(Clone, Default)]
struct Cache(HashMap<(Icon, u32), TextureHandle>);

fn texture(ctx: &egui::Context, icon: Icon, px: u32) -> Option<TextureHandle> {
    let id = Id::unique("lucide-icon-cache");
    if let Some(tex) = ctx.data(|d| {
        d.get_temp::<Cache>(id)
            .and_then(|c| c.0.get(&(icon, px)).cloned())
    }) {
        return Some(tex);
    }
    let image = rasterize(icon, px)?;
    let tex = ctx.load_texture(format!("icon-{icon:?}-{px}"), image, TextureOptions::LINEAR);
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(id)
            .0
            .insert((icon, px), tex.clone());
    });
    Some(tex)
}

/// Paints `icon` centred in `rect` (using its shorter side), tinted `color`.
pub fn paint(icon: Icon, painter: &Painter, rect: Rect, color: Color32) {
    let side = rect.width().min(rect.height());
    let ctx = painter.ctx();
    let px = (side * ctx.pixels_per_point()).round().max(1.0) as u32;
    if let Some(tex) = texture(ctx, icon, px) {
        // Land exactly on the pixel grid so the texture is drawn 1:1, with
        // no blending between neighbouring pixels.
        let ppp = ctx.pixels_per_point();
        let min = rect.center() - egui::Vec2::splat(side / 2.0);
        let snapped = pos2((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp);
        let square = Rect::from_min_size(snapped, egui::Vec2::splat(px as f32 / ppp));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(tex.id(), square, uv, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_renders_visible_strokes() {
        for icon in Icon::ALL {
            let image = rasterize(icon, 36).unwrap_or_else(|| panic!("{icon:?} failed"));
            let ink = image.pixels.iter().filter(|p| p.a() > 64).count();
            assert!(ink > 20, "{icon:?} is nearly empty");
            assert!(ink < 36 * 36 / 2, "{icon:?} looks filled, not stroked");
        }
    }

    #[test]
    fn icons_render_at_extreme_sizes() {
        for icon in Icon::ALL {
            let tiny = rasterize(icon, 1).unwrap();
            assert_eq!(tiny.size, [1, 1]);
            let big = rasterize(icon, 256).unwrap();
            assert_eq!(big.size, [256, 256]);
            // Premultiplied white: colour never exceeds coverage.
            assert!(big.pixels.iter().all(|p| p.r() <= p.a()));
        }
        assert!(rasterize(Icon::Font, 0).is_none(), "no zero-sized pixmaps");
    }

    #[test]
    fn every_icon_has_its_own_artwork() {
        let svgs: std::collections::HashSet<_> = Icon::ALL.iter().map(|i| i.svg()).collect();
        assert_eq!(svgs.len(), Icon::ALL.len());
        for icon in Icon::ALL {
            assert!(icon.svg().contains("stroke-width=\"2\""), "{icon:?}");
            assert!(icon.svg().contains("currentColor"), "{icon:?}");
        }
    }

    #[test]
    fn textures_are_cached_per_icon_and_size() {
        let ctx = egui::Context::default();
        let a = texture(&ctx, Icon::Play, 16).unwrap();
        let b = texture(&ctx, Icon::Play, 16).unwrap();
        assert_eq!(a.id(), b.id());
        let bigger = texture(&ctx, Icon::Play, 32).unwrap();
        assert_ne!(a.id(), bigger.id());
        assert_eq!(bigger.size(), [32, 32]);
        assert_ne!(a.id(), texture(&ctx, Icon::Pause, 16).unwrap().id());
    }

    #[test]
    fn painting_draws_one_pixel_aligned_image() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = Rect::from_min_size(pos2(10.3, 20.7), egui::vec2(16.0, 24.0));
            paint(Icon::Timer, ui.painter(), rect, Color32::RED);
            // An empty rectangle still gets a one-pixel icon.
            let empty = Rect::from_min_size(pos2(5.0, 5.0), egui::Vec2::ZERO);
            paint(Icon::Timer, ui.painter(), empty, Color32::RED);
        });
        output.textures_delta.clear();
        let images: Vec<Rect> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .collect();
        assert_eq!(images.len(), 2);
        let first = images[0];
        assert!((images[1].width() - 1.0).abs() < 1e-3, "{:?}", images[1]);
        assert!((first.width() - 16.0).abs() < 1e-3, "{first:?}");
        assert!((first.min.x - first.min.x.round()).abs() < 1e-3);
        assert!((first.min.y - first.min.y.round()).abs() < 1e-3);
    }
}
