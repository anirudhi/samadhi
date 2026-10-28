//! Drawing the current theme: depth layers with parallax, ambient particles,
//! drifting fog, a crossfade between themes and a vignette that deepens
//! while you write.

use std::f32::consts::TAU;
use std::time::Instant;

use eframe::egui::{
    self, Color32, ColorImage, Mesh, Painter, Pos2, Rect, TextureHandle, TextureOptions,
    TextureWrapMode, Vec2, pos2, vec2,
};

use crate::background::{Prepared, cover_uv};
use crate::scene::Particles;

/// How far (in points) the nearest layer moves at the edge of the screen.
const MAX_SHIFT: f32 = 34.0;
const CROSSFADE_SECS: f32 = 0.9;
/// How quickly the parallax catches up with the mouse (per second).
const PARALLAX_EASE: f32 = 2.2;

#[derive(Debug, Clone, Copy)]
struct Mote {
    x: f32,
    y: f32,
    depth: f32,
    phase: f32,
    speed: f32,
}

struct LiveLayer {
    texture: TextureHandle,
    /// Blurred copy, drawn behind the frosted sheet.
    frost: TextureHandle,
    depth: f32,
    drift: f32,
}

struct Live {
    layers: Vec<LiveLayer>,
    ink: Color32,
    dark: bool,
    particles: Option<Particles>,
    motes: Vec<Mote>,
}

/// Everything behind the page.
pub struct Stage {
    current: Option<Live>,
    previous: Option<Live>,
    switched_at: Instant,
    started: Instant,
    last_frame: Instant,
    parallax: Vec2,
    /// Animation clock and crossfade of the last frame, so the frosted
    /// sheet lines up exactly with the scene painted under it.
    clock: f32,
    crossfade: f32,
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("loaded", &self.current.is_some())
            .field("dark", &self.dark())
            .finish_non_exhaustive()
    }
}

impl Default for Stage {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            current: None,
            previous: None,
            switched_at: now,
            started: now,
            last_frame: now,
            parallax: Vec2::ZERO,
            clock: 0.0,
            crossfade: 1.0,
        }
    }
}

impl Stage {
    /// Uploads a newly built scene and starts crossfading to it.
    pub fn show(&mut self, ctx: &egui::Context, prepared: Prepared) {
        let Prepared { scene, frost } = prepared;
        let max_side = ctx.input(|i| i.max_texture_side);
        let layers = scene
            .layers
            .into_iter()
            .zip(frost)
            .enumerate()
            .map(|(i, (layer, frost))| {
                let options = TextureOptions {
                    wrap_mode: if layer.drift == 0.0 {
                        TextureWrapMode::ClampToEdge
                    } else {
                        TextureWrapMode::Repeat
                    },
                    ..TextureOptions::LINEAR
                };
                LiveLayer {
                    texture: ctx.load_texture(
                        format!("scene-layer-{i}"),
                        fit_texture(layer.image, max_side),
                        options,
                    ),
                    frost: ctx.load_texture(format!("scene-frost-{i}"), frost, options),
                    depth: layer.depth,
                    drift: layer.drift,
                }
            })
            .collect();
        let live = Live {
            layers,
            ink: scene.ink,
            dark: scene.dark,
            particles: scene.particles,
            motes: scene.particles.map(motes).unwrap_or_default(),
        };
        self.previous = self.current.replace(live);
        self.switched_at = Instant::now();
    }

    pub fn is_loaded(&self) -> bool {
        self.current.is_some()
    }

    /// Text colour chosen by the theme.
    pub fn ink(&self) -> Color32 {
        self.current
            .as_ref()
            .map_or(Color32::from_rgb(0x2b, 0x2b, 0x2b), |l| l.ink)
    }

    pub fn dark(&self) -> bool {
        self.current.as_ref().is_some_and(|l| l.dark)
    }

    /// Paints the stage. `writing` goes from 0 (interface visible) to 1
    /// (deep in writing) and deepens the vignette. Returns `true` while
    /// something is moving and another frame is needed.
    pub fn paint(
        &mut self,
        painter: &Painter,
        screen: Rect,
        mouse: Option<Pos2>,
        motion: bool,
        writing: f32,
    ) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let t = if motion {
            now.duration_since(self.started).as_secs_f32()
        } else {
            0.0
        };

        // Parallax follows the mouse, with a very slow "breathing" sway.
        let target = if motion {
            let towards_mouse = mouse.map_or(Vec2::ZERO, |p| {
                ((p - screen.center()) / (screen.size() * 0.5))
                    .clamp(Vec2::splat(-1.0), Vec2::splat(1.0))
            });
            towards_mouse + vec2((t * TAU / 23.0).sin() * 0.12, (t * TAU / 31.0).sin() * 0.06)
        } else {
            Vec2::ZERO
        };
        self.parallax += (target - self.parallax) * (1.0 - (-dt * PARALLAX_EASE).exp());
        let settling = (target - self.parallax).length() > 0.002;

        let fade = (now.duration_since(self.switched_at).as_secs_f32() / CROSSFADE_SECS).min(1.0);
        self.clock = t;
        self.crossfade = fade;
        if fade >= 1.0 {
            self.previous = None;
        }
        if self.current.is_none() {
            painter.rect_filled(screen, 0.0, Color32::from_rgb(0xed, 0xea, 0xe4));
        }
        if let Some(previous) = &self.previous {
            paint_live(painter, screen, previous, self.parallax, t, 1.0);
        }
        let mut animated = settling || fade < 1.0;
        if let Some(current) = &self.current {
            paint_live(painter, screen, current, self.parallax, t, smooth(fade));
            paint_vignette(painter, screen, current.dark, writing);
            animated |= motion
                && (current.particles.is_some() || current.layers.iter().any(|l| l.drift != 0.0));
        }
        animated
    }
}

impl Stage {
    /// Paints the scene behind `page` again, blurred, like frosted glass.
    /// `amount` goes from 0 (clear) to 1 (fully frosted). Call after
    /// [`Self::paint`] in the same frame.
    pub fn paint_frost(&self, painter: &Painter, screen: Rect, page: Rect, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let glass = painter.with_clip_rect(page.intersect(painter.clip_rect()));
        if let Some(previous) = &self.previous {
            paint_frosted(&glass, screen, previous, self.parallax, self.clock, amount);
        }
        if let Some(current) = &self.current {
            let alpha = amount * smooth(self.crossfade);
            paint_frosted(&glass, screen, current, self.parallax, self.clock, alpha);
        }
    }
}

/// The blurred layers, placed exactly like the sharp ones, then the
/// particles as soft, out-of-focus blurs.
fn paint_frosted(painter: &Painter, screen: Rect, live: &Live, parallax: Vec2, t: f32, alpha: f32) {
    let tint = Color32::WHITE.gamma_multiply(alpha);
    for layer in &live.layers {
        let (rect, uv) = placement(screen, layer, parallax, t);
        painter.image(layer.frost.id(), rect, uv, tint);
    }
    paint_particles(painter, screen, live, parallax, t, alpha, true);
}

/// Where a layer is drawn this frame, and which part of its image.
fn placement(screen: Rect, layer: &LiveLayer, parallax: Vec2, t: f32) -> (Rect, Rect) {
    let shift = -parallax * layer.depth * MAX_SHIFT;
    let margin = MAX_SHIFT * layer.depth + 2.0;
    let rect = screen.expand(margin).translate(shift);
    let mut uv = cover_uv(layer.texture.size_vec2(), rect.size());
    if layer.drift != 0.0 {
        uv = uv.translate(vec2(t * layer.drift, 0.0));
    }
    (rect, uv)
}

/// Downscales `image` if a side exceeds what the GPU supports (often 2048
/// on older or integrated graphics).
fn fit_texture(image: ColorImage, max_side: usize) -> ColorImage {
    let [w, h] = image.size;
    if w <= max_side && h <= max_side {
        return image;
    }
    let scale = max_side as f32 / w.max(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale) as u32).max(1),
        ((h as f32 * scale) as u32).max(1),
    );
    let raw: Vec<u8> = image.pixels.iter().flat_map(Color32::to_array).collect();
    let Some(buffer) = image::RgbaImage::from_raw(w as u32, h as u32, raw) else {
        return image;
    };
    let resized = image::imageops::resize(&buffer, nw, nh, image::imageops::FilterType::Triangle);
    // The pixels are premultiplied already, so copy them across as-is.
    let pixels = resized
        .pixels()
        .map(|p| Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    ColorImage::new([nw as usize, nh as usize], pixels)
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn paint_live(painter: &Painter, screen: Rect, live: &Live, parallax: Vec2, t: f32, alpha: f32) {
    let tint = Color32::WHITE.gamma_multiply(alpha);
    for layer in &live.layers {
        let (rect, uv) = placement(screen, layer, parallax, t);
        painter.image(layer.texture.id(), rect, uv, tint);
    }
    paint_particles(painter, screen, live, parallax, t, alpha, false);
}

/// Deterministic pseudo-random motes for a particle kind.
fn motes(kind: Particles) -> Vec<Mote> {
    let count = match kind {
        Particles::Snow => 170,
        Particles::Stars => 150,
        Particles::Fireflies => 28,
        Particles::Dust => 45,
    };
    let mut seed = 0x2545_F491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    (0..count)
        .map(|_| {
            let (x, y) = (next(), next());
            let y = match kind {
                Particles::Stars => y * y * 0.6,
                Particles::Fireflies => 0.55 + y * 0.4,
                _ => y,
            };
            Mote {
                x,
                y,
                depth: next(),
                phase: next() * TAU,
                speed: 0.5 + next(),
            }
        })
        .collect()
}

fn paint_particles(
    painter: &Painter,
    screen: Rect,
    live: &Live,
    parallax: Vec2,
    t: f32,
    alpha: f32,
    soft: bool,
) {
    let Some(kind) = live.particles else {
        return;
    };
    // Behind the frosted sheet, each mote is a wide, faint, out-of-focus
    // glow instead of a sharp dot.
    let dot = |p: Pos2, r: f32, color: Color32| {
        if soft {
            soft_dot(painter, p, (r + 1.5) * 3.0, color.gamma_multiply(0.35));
        } else {
            painter.circle_filled(p, r, color);
        }
    };
    let at = |x: f32, y: f32, depth: f32| {
        let shift = -parallax * (0.3 + depth * 0.7) * MAX_SHIFT;
        pos2(
            screen.left() + x * screen.width(),
            screen.top() + y * screen.height(),
        ) + shift
    };
    for m in &live.motes {
        match kind {
            Particles::Snow => {
                let fall = 0.012 + 0.03 * m.depth;
                let y = (m.y + t * fall * m.speed).fract();
                let x = (m.x + 0.012 * (t * 0.35 * m.speed + m.phase).sin() + t * 0.003).fract();
                let r = 0.8 + 2.4 * m.depth;
                let a = (0.35 + 0.5 * m.depth) * alpha;
                dot(at(x, y, m.depth), r, Color32::WHITE.gamma_multiply(a));
            }
            Particles::Stars => {
                let twinkle = 0.5 + 0.5 * (t * 1.7 * m.speed + m.phase).sin();
                let a = (0.25 + 0.6 * twinkle * (0.4 + 0.6 * m.depth)) * alpha;
                let r = 0.5 + 1.1 * m.depth * m.depth;
                dot(
                    at(m.x, m.y, 0.0),
                    r,
                    Color32::from_rgb(0xf4, 0xf1, 0xe6).gamma_multiply(a),
                );
            }
            Particles::Fireflies => {
                let x = m.x + 0.025 * (t * 0.21 * m.speed + m.phase).sin();
                let y = m.y + 0.018 * (t * 0.17 * m.speed + m.phase * 1.7).cos();
                let pulse = (0.5 + 0.5 * (t * 0.9 * m.speed + m.phase).sin()).powi(2);
                let p = at(x, y, m.depth);
                let glow = Color32::from_rgb(0xe9, 0xf2, 0xa2);
                dot(
                    p,
                    7.0 + 5.0 * m.depth,
                    glow.gamma_multiply(0.08 * pulse * alpha),
                );
                dot(
                    p,
                    1.2 + 1.0 * m.depth,
                    glow.gamma_multiply((0.25 + 0.7 * pulse) * alpha),
                );
            }
            Particles::Dust => {
                let x = (m.x + t * 0.004 * m.speed).fract();
                let y = m.y + 0.012 * (t * 0.2 * m.speed + m.phase).sin();
                let a = (0.15 + 0.25 * m.depth) * alpha;
                dot(
                    at(x, y, m.depth),
                    0.8 + 1.4 * m.depth,
                    Color32::WHITE.gamma_multiply(a),
                );
            }
        }
    }
}

/// A round glow fading from `color` at the centre to nothing at `radius`.
fn soft_dot(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    const SEGMENTS: u32 = 16;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..SEGMENTS {
        let angle = TAU * i as f32 / SEGMENTS as f32;
        mesh.colored_vertex(
            center + radius * vec2(angle.cos(), angle.sin()),
            Color32::TRANSPARENT,
        );
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % SEGMENTS);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Darkens the edges of the screen, more so while writing, drawing the eye
/// to the page like a shallow depth of field.
fn paint_vignette(painter: &Painter, screen: Rect, dark: bool, writing: f32) {
    const COLS: usize = 16;
    const ROWS: usize = 10;
    let strength = (if dark { 0.42 } else { 0.2 }) * (0.45 + 0.55 * writing);
    let mut mesh = Mesh::default();
    for row in 0..=ROWS {
        for col in 0..=COLS {
            let (u, v) = (col as f32 / COLS as f32, row as f32 / ROWS as f32);
            let d = (vec2(u - 0.5, v - 0.5) * 2.0).length() / std::f32::consts::SQRT_2;
            let a = strength * smooth(((d - 0.35) / 0.65).clamp(0.0, 1.0));
            let pos = pos2(
                screen.left() + u * screen.width(),
                screen.top() + v * screen.height(),
            );
            mesh.colored_vertex(pos, Color32::from_black_alpha((a * 255.0) as u8));
        }
    }
    let stride = (COLS + 1) as u32;
    for row in 0..ROWS as u32 {
        for col in 0..COLS as u32 {
            let i = row * stride + col;
            mesh.add_triangle(i, i + 1, i + stride);
            mesh.add_triangle(i + 1, i + stride + 1, i + stride);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_textures_are_scaled_to_fit_the_gpu() {
        let image = ColorImage::new([4000, 1000], vec![Color32::WHITE; 4000 * 1000]);
        let fitted = fit_texture(image, 2048);
        assert_eq!(fitted.size, [2048, 512]);
        let small = ColorImage::new([10, 10], vec![Color32::WHITE; 100]);
        assert_eq!(fit_texture(small, 2048).size, [10, 10]);
    }

    fn prepared(gray: u8, particles: Option<Particles>) -> Prepared {
        let mut scene =
            crate::scene::from_image(ColorImage::new([8, 4], vec![Color32::from_gray(gray); 32]));
        scene.particles = particles;
        let frost = scene
            .layers
            .iter()
            .map(|l| crate::frost::frost(&l.image, l.drift != 0.0))
            .collect();
        Prepared { scene, frost }
    }

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))
    }

    /// Runs one frame, painting the stage, and returns its result and the
    /// number of shapes drawn.
    fn frame(
        ctx: &egui::Context,
        stage: &mut Stage,
        draw: impl Fn(&mut Stage, &Painter) -> bool,
    ) -> (bool, usize) {
        let mut animated = false;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            animated = draw(stage, ui.painter());
        });
        output.textures_delta.clear();
        (animated, output.shapes.len())
    }

    #[test]
    fn an_empty_stage_paints_a_plain_backdrop() {
        let ctx = egui::Context::default();
        let mut stage = Stage::default();
        assert!(!stage.is_loaded());
        assert!(!stage.dark());
        assert_eq!(stage.ink(), Color32::from_rgb(0x2b, 0x2b, 0x2b));
        assert!(format!("{stage:?}").contains("loaded: false"));
        let (_, baseline) = frame(&ctx, &mut stage, |_, _| false);
        stage.switched_at = Instant::now()
            .checked_sub(std::time::Duration::from_secs(2))
            .unwrap_or(stage.switched_at);
        let (animated, shapes) = frame(&ctx, &mut stage, |s, p| {
            s.paint(p, screen(), None, false, 0.0)
        });
        assert!(!animated, "nothing moves without a scene or motion");
        assert_eq!(shapes, baseline + 1);
    }

    #[test]
    fn showing_a_scene_crossfades_to_it() {
        let ctx = egui::Context::default();
        let mut stage = Stage::default();
        stage.show(&ctx, prepared(10, None));
        assert!(stage.is_loaded());
        assert!(stage.dark(), "a dark image gets light ink");
        assert_ne!(stage.ink(), Color32::from_rgb(0x2b, 0x2b, 0x2b));
        let (animated, _) = frame(&ctx, &mut stage, |s, p| {
            s.paint(p, screen(), Some(pos2(5.0, 5.0)), true, 1.0)
        });
        assert!(animated, "the crossfade has only just started");

        stage.show(&ctx, prepared(240, Some(Particles::Snow)));
        assert!(!stage.dark());
        assert!(stage.previous.is_some(), "the old scene fades out");
    }

    #[test]
    fn frost_is_drawn_only_when_asked_for() {
        let ctx = egui::Context::default();
        let mut stage = Stage::default();
        stage.show(&ctx, prepared(120, Some(Particles::Fireflies)));
        let page = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0));
        let (_, baseline) = frame(&ctx, &mut stage, |_, _| false);
        let (_, none) = frame(&ctx, &mut stage, |s, p| {
            s.paint_frost(p, screen(), page, 0.0);
            false
        });
        assert_eq!(none, baseline);
        let (_, some) = frame(&ctx, &mut stage, |s, p| {
            s.paint(p, screen(), None, true, 0.5);
            s.paint_frost(p, screen(), page, 1.0);
            false
        });
        assert!(some > baseline + 1);
    }

    #[test]
    fn every_particle_kind_paints_without_trouble() {
        let ctx = egui::Context::default();
        for kind in [
            Particles::Snow,
            Particles::Stars,
            Particles::Fireflies,
            Particles::Dust,
        ] {
            let mut stage = Stage::default();
            stage.show(&ctx, prepared(60, Some(kind)));
            for motion in [true, false] {
                let (animated, _) = frame(&ctx, &mut stage, |s, p| {
                    let a = s.paint(p, screen(), Some(pos2(-50.0, 900.0)), motion, 0.3);
                    s.paint_frost(p, screen(), screen(), 0.8);
                    a
                });
                assert!(animated, "{kind:?}: the crossfade is still running");
            }
        }
    }

    #[test]
    fn motes_are_deterministic_and_on_screen() {
        for kind in [
            Particles::Snow,
            Particles::Stars,
            Particles::Fireflies,
            Particles::Dust,
        ] {
            let a = motes(kind);
            let b = motes(kind);
            assert!(!a.is_empty());
            assert_eq!(a.len(), b.len());
            for (m, n) in a.iter().zip(&b) {
                assert_eq!((m.x, m.y, m.depth), (n.x, n.y, n.depth));
                assert!(
                    (0.0..=1.0).contains(&m.x) && (0.0..=1.0).contains(&m.y),
                    "{kind:?}"
                );
                assert!((0.0..=1.0).contains(&m.depth));
                assert!(m.speed >= 0.5 && m.speed <= 1.5);
            }
        }
        // Stars stay in the sky, fireflies near the ground.
        assert!(motes(Particles::Stars).iter().all(|m| m.y <= 0.6));
        assert!(motes(Particles::Fireflies).iter().all(|m| m.y >= 0.55));
    }

    #[test]
    fn smoothstep_eases_from_zero_to_one() {
        assert!(smooth(0.0).abs() < f32::EPSILON);
        assert!((smooth(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((smooth(0.5) - 0.5).abs() < f32::EPSILON);
        let mut last = 0.0;
        for i in 1..=100 {
            let v = smooth(i as f32 / 100.0);
            assert!(v >= last);
            last = v;
        }
    }

    #[test]
    fn layers_cover_the_screen_with_room_to_move() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "t",
            ColorImage::new([200, 100], vec![Color32::WHITE; 20_000]),
            TextureOptions::LINEAR,
        );
        let layer = LiveLayer {
            texture: texture.clone(),
            frost: texture,
            depth: 1.0,
            drift: 0.0,
        };
        for parallax in [Vec2::ZERO, vec2(1.0, -1.0), vec2(-1.0, 1.0)] {
            let (rect, uv) = placement(screen(), &layer, parallax, 3.0);
            assert!(rect.contains_rect(screen()), "{parallax:?}: {rect:?}");
            assert!(uv.min.x >= 0.0 && uv.max.x <= 1.0);
        }
    }
}
