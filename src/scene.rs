//! Themes: painted landscapes generated in code, made of depth layers so
//! they can move with a gentle parallax.
//!
//! Every theme is original artwork created procedurally at start-up, so no
//! image files or licences are involved. Each one sets its own text colour
//! and whether the interface should be light or dark, the way OmmWriter's
//! themes do.

// Colours are written as familiar `0xRRGGBB` hex codes.
#![allow(clippy::unreadable_literal)]

use std::f32::consts::TAU;

use eframe::egui::{Color32, ColorImage};

/// Built-in themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    // Photographs (public domain; see assets/photos/CREDITS.md).
    HazyHills,
    WhiteForest,
    DesertHaze,
    SeaMist,
    FogForest,
    DuskRidges,
    MilkyWay,
    IslandInFog,
    // Painted in code.
    DawnMist,
    Snowfield,
    Dunes,
    SeaGlass,
    Paper,
    Dusk,
    ForestNight,
    NightSky,
}

/// Small moving things that bring a scene to life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Particles {
    Snow,
    Stars,
    Fireflies,
    Dust,
}

/// One depth layer. `depth` runs from 0 (infinitely far, never moves) to 1
/// (closest to the viewer, moves the most).
#[derive(Debug, Clone)]
pub struct Layer {
    pub image: ColorImage,
    pub depth: f32,
    /// Horizontal drift in screen widths per second (fog banks); the image
    /// tiles seamlessly in x when this is non-zero.
    pub drift: f32,
}

/// A fully generated theme, ready to upload to the GPU.
#[derive(Debug, Clone)]
pub struct Scene {
    pub layers: Vec<Layer>,
    pub ink: Color32,
    pub dark: bool,
    pub particles: Option<Particles>,
}

impl ThemeKind {
    pub const ALL: [(&'static str, Self); 16] = [
        ("Hazy Hills", Self::HazyHills),
        ("White Forest", Self::WhiteForest),
        ("Desert Haze", Self::DesertHaze),
        ("Sea Mist", Self::SeaMist),
        ("Fog Forest", Self::FogForest),
        ("Dusk Ridges", Self::DuskRidges),
        ("Milky Way", Self::MilkyWay),
        ("Island in Fog", Self::IslandInFog),
        ("Dawn Mist", Self::DawnMist),
        ("Snowfield", Self::Snowfield),
        ("Dunes", Self::Dunes),
        ("Sea Glass", Self::SeaGlass),
        ("Paper", Self::Paper),
        ("Dusk", Self::Dusk),
        ("Forest Night", Self::ForestNight),
        ("Night Sky", Self::NightSky),
    ];

    pub fn is_dark(self) -> bool {
        match self.photo() {
            Some(photo) => photo.dark,
            None => matches!(self, Self::Dusk | Self::ForestNight | Self::NightSky),
        }
    }

    /// How each photograph is presented.
    fn photo(self) -> Option<Photo> {
        let p = |bytes, dark, particles, veil, fog| Photo {
            bytes,
            dark,
            particles,
            veil,
            fog,
        };
        let white = hex(0xF4F2EE);
        let black = hex(0x0B0C10);
        Some(match self {
            Self::HazyHills => p(
                include_bytes!("../assets/photos/hazy-hills.jpg"),
                false,
                Some(Particles::Dust),
                Some((white, 0.0, 0.45)),
                false,
            ),
            Self::WhiteForest => p(
                include_bytes!("../assets/photos/white-forest.jpg"),
                false,
                Some(Particles::Snow),
                None,
                false,
            ),
            Self::DesertHaze => p(
                include_bytes!("../assets/photos/desert-haze.jpg"),
                true,
                Some(Particles::Dust),
                Some((black, 0.45, 0.1)),
                false,
            ),
            Self::SeaMist => p(
                include_bytes!("../assets/photos/sea-mist.jpg"),
                false,
                None,
                Some((white, 0.0, 0.35)),
                false,
            ),
            Self::FogForest => p(
                include_bytes!("../assets/photos/fog-forest.jpg"),
                false,
                None,
                Some((white, 0.0, 0.75)),
                true,
            ),
            Self::DuskRidges => p(
                include_bytes!("../assets/photos/dusk-ridges.jpg"),
                true,
                None,
                Some((black, 0.4, 0.05)),
                false,
            ),
            Self::MilkyWay => p(
                include_bytes!("../assets/photos/milky-way.jpg"),
                true,
                Some(Particles::Stars),
                None,
                false,
            ),
            Self::IslandInFog => p(
                include_bytes!("../assets/photos/island-in-fog.jpg"),
                false,
                None,
                None,
                true,
            ),
            _ => return None,
        })
    }

    /// Paints the theme. Takes a few hundred milliseconds, so the app runs
    /// it on a worker thread.
    pub fn build(self) -> Scene {
        if let Some(photo) = self.photo() {
            return photo.build();
        }
        let c = hex;
        match self {
            Self::DawnMist => Scene {
                layers: vec![
                    sky(
                        c(0xE6D9EA),
                        c(0xF8E5D4),
                        Some(((0.68, 0.46), c(0xFFF4E2), 0.32)),
                    ),
                    ridge(0.55, 0.17, 1, c(0xCBC3DC), c(0xEEDFE6), 0.15, 6.0, 0.18),
                    fog(0.62, 0.05, c(0xF6E8EC), 0.55, 21, 0.002),
                    ridge(0.64, 0.12, 2, c(0xB2A9CA), c(0xE3D3E0), 0.35, 3.0, 0.4),
                    ridge(0.75, 0.06, 3, c(0x9A90B4), c(0xCDBFD3), 0.6, 1.2, 0.7),
                    fog(0.84, 0.05, c(0xF2E2E8), 0.35, 22, 0.004),
                    ridge(0.88, 0.035, 4, c(0x80779B), c(0xA89DB8), 0.85, 0.0, 1.0),
                ],
                ink: c(0x2F2A3A),
                dark: false,
                particles: Some(Particles::Dust),
            },
            Self::Snowfield => Scene {
                layers: vec![
                    sky(
                        c(0xD5E1EC),
                        c(0xF3F6F9),
                        Some(((0.3, 0.3), c(0xFFFFFF), 0.4)),
                    ),
                    ridge(0.56, 0.16, 11, c(0xC4D0DD), c(0xE7EDF3), 0.2, 6.0, 0.18),
                    trees(0.66, 0.06, 12, c(0x9AA8B7), c(0xDDE4EB), 0.3, 2.5, 0.42),
                    fog(0.7, 0.05, c(0xF4F7FA), 0.5, 13, 0.003),
                    ridge(0.8, 0.04, 14, c(0xF2F5F8), c(0xDCE4EC), 0.5, 1.0, 0.75),
                    ridge(0.9, 0.025, 15, c(0xFBFCFD), c(0xE9EEF3), 0.8, 0.0, 1.0),
                ],
                ink: c(0x2C3440),
                dark: false,
                particles: Some(Particles::Snow),
            },
            Self::Dunes => Scene {
                layers: vec![
                    sky(
                        c(0xF1D7BD),
                        c(0xF9ECDD),
                        Some(((0.72, 0.38), c(0xFFF6E6), 0.3)),
                    ),
                    dune(0.6, 0.07, 21, c(0xE6C6A2), c(0xF2DFC9), 6.0, 0.2),
                    dune(0.7, 0.07, 22, c(0xDDB58C), c(0xEBD0B3), 3.0, 0.45),
                    dune(0.8, 0.06, 23, c(0xCFA176), c(0xE0BD99), 1.0, 0.75),
                    dune(0.9, 0.04, 24, c(0xC08F64), c(0xD3A984), 0.0, 1.0),
                ],
                ink: c(0x3B2E22),
                dark: false,
                particles: Some(Particles::Dust),
            },
            Self::SeaGlass => Scene {
                layers: vec![
                    sky(
                        c(0xD3EBE7),
                        c(0xF2F8F3),
                        Some(((0.5, 0.58), c(0xFFFFFF), 0.35)),
                    ),
                    ridge(0.58, 0.04, 31, c(0xB6CFCB), c(0xD9E9E5), 0.25, 5.0, 0.15),
                    sea(0.62, c(0xA9D0CA), c(0x76AAA4), 0.6),
                    fog(0.6, 0.03, c(0xEEF7F4), 0.5, 32, 0.002),
                ],
                ink: c(0x24403D),
                dark: false,
                particles: None,
            },
            Self::Paper => Scene {
                layers: vec![paper(c(0xF4F0E6))],
                ink: c(0x2B2B2B),
                dark: false,
                particles: None,
            },
            Self::Dusk => Scene {
                layers: vec![
                    sky(
                        c(0x2B2A4C),
                        c(0xD4917F),
                        Some(((0.5, 0.62), c(0xF7C99C), 0.22)),
                    ),
                    sun(0.5, 0.6, 0.05, c(0xF9D7AE)),
                    ridge(0.6, 0.15, 41, c(0x5A4466), c(0x9C6F7C), 0.25, 5.0, 0.2),
                    ridge(0.7, 0.1, 42, c(0x3E3155), c(0x6B5070), 0.5, 2.0, 0.45),
                    fog(0.76, 0.04, c(0x8C6A80), 0.3, 43, 0.003),
                    ridge(0.82, 0.05, 44, c(0x2A2340), c(0x3E3255), 0.75, 0.6, 0.75),
                    ridge(0.92, 0.03, 45, c(0x1B1730), c(0x251F3B), 0.9, 0.0, 1.0),
                ],
                ink: c(0xEDE6F2),
                dark: true,
                particles: Some(Particles::Stars),
            },
            Self::ForestNight => Scene {
                layers: vec![
                    sky(
                        c(0x0D1820),
                        c(0x1E3A3E),
                        Some(((0.72, 0.22), c(0x5D7F80), 0.18)),
                    ),
                    sun(0.72, 0.2, 0.02, c(0xD9E4DC)),
                    trees(0.6, 0.08, 51, c(0x1A3035), c(0x2C4A4D), 0.15, 4.0, 0.2),
                    fog(0.66, 0.05, c(0x4E7473), 0.35, 52, 0.003),
                    trees(0.74, 0.09, 53, c(0x12232A), c(0x1D3439), 0.4, 1.5, 0.5),
                    trees(0.88, 0.1, 54, c(0x0A151A), c(0x0F1E23), 0.75, 0.0, 1.0),
                ],
                ink: c(0xDCE8E3),
                dark: true,
                particles: Some(Particles::Fireflies),
            },
            Self::NightSky => Scene {
                layers: vec![
                    sky(
                        c(0x060A18),
                        c(0x1C2648),
                        Some(((0.25, 0.75), c(0x34406A), 0.35)),
                    ),
                    ridge(0.74, 0.17, 61, c(0x161D38), c(0x1E2747), 0.3, 3.0, 0.3),
                    ridge(0.86, 0.06, 62, c(0x0C1126), c(0x121933), 0.6, 0.0, 1.0),
                ],
                ink: c(0xE6E9F5),
                dark: true,
                particles: Some(Particles::Stars),
            },
            _ => unreachable!("photographs are built above"),
        }
    }
}

impl Scene {
    /// A small preview of the scene, `w`×`h` pixels, cropped to fill like
    /// the real background (layers composited, 3×3 supersampled).
    pub fn thumbnail(&self, w: usize, h: usize) -> ColorImage {
        let mut acc = vec![[0.0_f32; 4]; w * h];
        for layer in &self.layers {
            let [lw, lh] = layer.image.size;
            let uv = crate::background::cover_uv(
                eframe::egui::vec2(lw as f32, lh as f32),
                eframe::egui::vec2(w as f32, h as f32),
            );
            let sample = |u: f32, v: f32| {
                let x = ((uv.min.x + u * uv.width()) * lw as f32).clamp(0.0, (lw - 1) as f32);
                let y = ((uv.min.y + v * uv.height()) * lh as f32).clamp(0.0, (lh - 1) as f32);
                layer.image.pixels[y as usize * lw + x as usize]
            };
            for py in 0..h {
                for px in 0..w {
                    let mut c = [0.0_f32; 4];
                    for sy in 0..3 {
                        for sx in 0..3 {
                            let u = (px as f32 + (sx as f32 + 0.5) / 3.0) / w as f32;
                            let v = (py as f32 + (sy as f32 + 0.5) / 3.0) / h as f32;
                            let p = sample(u, v);
                            for (ch, value) in c.iter_mut().zip(p.to_array()) {
                                *ch += f32::from(value) / 9.0;
                            }
                        }
                    }
                    // Premultiplied "over".
                    let dst = &mut acc[py * w + px];
                    let keep = 1.0 - c[3] / 255.0;
                    for i in 0..4 {
                        dst[i] = c[i] + dst[i] * keep;
                    }
                }
            }
        }
        let pixels = acc
            .iter()
            .map(|c| {
                let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                Color32::from_rgba_premultiplied(q(c[0]), q(c[1]), q(c[2]), q(c[3]))
            })
            .collect();
        ColorImage::new([w, h], pixels)
    }
}

/// A bundled photograph and how to present it.
#[derive(Debug, Clone, Copy)]
struct Photo {
    bytes: &'static [u8],
    dark: bool,
    particles: Option<Particles>,
    /// A soft wash from the top to the bottom of the screen (colour, top
    /// opacity, bottom opacity), so text stays readable over bright skies
    /// or dark foregrounds.
    veil: Option<(Color32, f32, f32)>,
    /// Add a slowly drifting band of mist.
    fog: bool,
}

impl Photo {
    fn build(self) -> Scene {
        let image = image::load_from_memory(self.bytes)
            .expect("bundled photographs are valid JPEGs")
            .into_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let mut layers = vec![Layer {
            image: ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
            depth: 0.3,
            drift: 0.0,
        }];
        if self.fog {
            let mist = if self.dark {
                hex(0x8A98A0)
            } else {
                hex(0xF6F6F4)
            };
            layers.push(fog(0.55, 0.07, mist, 0.35, 91, 0.003));
        }
        if let Some((color, top, bottom)) = self.veil {
            layers.push(veil(color, top, bottom));
        }
        Scene {
            layers,
            ink: if self.dark {
                hex(0xEEEEEA)
            } else {
                hex(0x24262B)
            },
            dark: self.dark,
            particles: self.particles,
        }
    }
}

/// A vertical wash of `color` from `top` to `bottom` opacity.
fn veil(color: Color32, top: f32, bottom: f32) -> Layer {
    let (w, h) = (4, H / 4);
    let mut c = Canvas::new(w, h);
    let rgb = [
        f32::from(color.r()),
        f32::from(color.g()),
        f32::from(color.b()),
    ];
    for y in 0..h {
        let t = smoothstep(y as f32 / (h - 1) as f32);
        for x in 0..w {
            c.set(x, y, rgb, top + (bottom - top) * t);
        }
    }
    Layer {
        image: c.into_image(95),
        depth: 0.0,
        drift: 0.0,
    }
}

/// Builds a single-layer scene from one of the user's own images, picking
/// light or dark text from the image's brightness.
pub fn from_image(image: ColorImage) -> Scene {
    let n = image.pixels.len().max(1) as f32;
    let luminance = image
        .pixels
        .iter()
        .map(|p| 0.2126 * f32::from(p.r()) + 0.7152 * f32::from(p.g()) + 0.0722 * f32::from(p.b()))
        .sum::<f32>()
        / n
        / 255.0;
    let dark = luminance < 0.5;
    Scene {
        layers: vec![Layer {
            image,
            depth: 0.35,
            drift: 0.0,
        }],
        ink: if dark { hex(0xEDEDED) } else { hex(0x262626) },
        dark,
        particles: None,
    }
}

// ----- painting --------------------------------------------------------------

const W: usize = 1600;
const H: usize = 900;

fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        f32::from(a.r()) + (f32::from(b.r()) - f32::from(a.r())) * t,
        f32::from(a.g()) + (f32::from(b.g()) - f32::from(a.g())) * t,
        f32::from(a.b()) + (f32::from(b.b()) - f32::from(a.b())) * t,
    ]
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deterministic hash → [0, 1).
fn hash(seed: u32, i: i64) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(seed).wrapping_mul(0xD1B5_4A32_D192_ED03);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth 1-D fractal noise in [-1, 1], periodic over `period` lattice cells
/// at the base octave so drifting layers tile seamlessly.
fn fractal(seed: u32, x: f32, octaves: u32, roughness: f32, period: i64) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        let p = x * freq;
        let cells = period * freq as i64;
        let i = p.floor() as i64;
        let f = p - p.floor();
        let a = hash(seed + o, i.rem_euclid(cells)) * 2.0 - 1.0;
        let b = hash(seed + o, (i + 1).rem_euclid(cells)) * 2.0 - 1.0;
        sum += (a + (b - a) * smoothstep(f)) * amp;
        norm += amp;
        amp *= roughness;
        freq *= 2.0;
    }
    sum / norm
}

/// A premultiplied RGBA float canvas.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![[0.0; 4]; w * h],
        }
    }

    fn set(&mut self, x: usize, y: usize, rgb: [f32; 3], alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);
        self.px[y * self.w + x] = [rgb[0] * a, rgb[1] * a, rgb[2] * a, a * 255.0];
    }

    /// Three box-blur passes approximate a Gaussian (depth of field).
    fn blur(&mut self, radius: f32) {
        let r = radius.round() as usize;
        if r == 0 {
            return;
        }
        for _ in 0..3 {
            self.box_pass(r, true);
            self.box_pass(r, false);
        }
    }

    fn box_pass(&mut self, r: usize, horizontal: bool) {
        let (len, lines) = if horizontal {
            (self.w, self.h)
        } else {
            (self.h, self.w)
        };
        let idx = |line: usize, i: usize| {
            if horizontal {
                line * self.w + i
            } else {
                i * self.w + line
            }
        };
        let mut out = self.px.clone();
        let window = (2 * r + 1) as f32;
        for line in 0..lines {
            let mut acc = [0.0_f32; 4];
            let at = |i: isize| self.px[idx(line, i.clamp(0, len as isize - 1) as usize)];
            for i in -(r as isize)..=(r as isize) {
                let p = at(i);
                for c in 0..4 {
                    acc[c] += p[c];
                }
            }
            for i in 0..len {
                let o = &mut out[idx(line, i)];
                for c in 0..4 {
                    o[c] = acc[c] / window;
                }
                let add = at(i as isize + r as isize + 1);
                let sub = at(i as isize - r as isize);
                for c in 0..4 {
                    acc[c] += add[c] - sub[c];
                }
            }
        }
        self.px = out;
    }

    /// Converts to an egui image, adding a little dither against banding.
    fn into_image(self, seed: u32) -> ColorImage {
        let pixels = self
            .px
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let d = hash(seed, i as i64) - 0.5;
                let q = |v: f32| (v + d).round().clamp(0.0, 255.0) as u8;
                let a = q(p[3]);
                Color32::from_rgba_premultiplied(q(p[0]).min(a), q(p[1]).min(a), q(p[2]).min(a), a)
            })
            .collect();
        ColorImage::new([self.w, self.h], pixels)
    }
}

/// A full-screen gradient sky with an optional soft glow (sun or moon haze).
fn sky(top: Color32, bottom: Color32, glow: Option<((f32, f32), Color32, f32)>) -> Layer {
    // Skies are smooth, so a small canvas stretched by the GPU is enough.
    let (w, h) = (W / 4, H / 4);
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / (w - 1) as f32, y as f32 / (h - 1) as f32);
            let mut rgb = mix(top, bottom, smoothstep(v));
            if let Some(((gx, gy), color, radius)) = glow {
                let d = (((u - gx) * 16.0 / 9.0).powi(2) + (v - gy).powi(2)).sqrt();
                let k = (-(d / radius).powi(2)).exp() * 0.85;
                let g = [
                    f32::from(color.r()),
                    f32::from(color.g()),
                    f32::from(color.b()),
                ];
                for i in 0..3 {
                    rgb[i] += (g[i] - rgb[i]) * k;
                }
            }
            c.set(x, y, rgb, 1.0);
        }
    }
    Layer {
        image: c.into_image(1),
        depth: 0.0,
        drift: 0.0,
    }
}

/// A sun or moon disc with a soft edge.
fn sun(cx: f32, cy: f32, radius: f32, color: Color32) -> Layer {
    let (w, h) = (W / 2, H / 2);
    let mut c = Canvas::new(w, h);
    let rgb = [
        f32::from(color.r()),
        f32::from(color.g()),
        f32::from(color.b()),
    ];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let d = (((u - cx) * 16.0 / 9.0).powi(2) + (v - cy).powi(2)).sqrt();
            let a = (1.0 - smoothstep((d - radius) / (radius * 0.35) + 0.5)) * 0.92;
            if a > 0.0 {
                c.set(x, y, rgb, a);
            }
        }
    }
    Layer {
        image: c.into_image(2),
        depth: 0.05,
        drift: 0.0,
    }
}

/// A mountain ridge. `base` and `amplitude` are fractions of the height,
/// `haze` the colour it fades to at its foot, `blur` the depth-of-field
/// radius in pixels.
#[allow(clippy::too_many_arguments)]
fn ridge(
    base: f32,
    amplitude: f32,
    seed: u32,
    color: Color32,
    haze: Color32,
    roughness: f32,
    blur: f32,
    depth: f32,
) -> Layer {
    let heights: Vec<f32> = (0..W)
        .map(|x| {
            let u = x as f32 / W as f32;
            let n = fractal(seed, u * 4.0, 6, 0.45 + roughness * 0.2, 4);
            base - amplitude * (0.55 + 0.45 * n)
        })
        .collect();
    fill_below(&heights, seed, color, haze, blur, depth)
}

/// Rolling dunes: smooth, long curves.
fn dune(
    base: f32,
    amplitude: f32,
    seed: u32,
    color: Color32,
    haze: Color32,
    blur: f32,
    depth: f32,
) -> Layer {
    let heights: Vec<f32> = (0..W)
        .map(|x| {
            let u = x as f32 / W as f32;
            let n = fractal(seed, u * 2.0, 3, 0.35, 2);
            base - amplitude * (0.5 + 0.5 * n)
        })
        .collect();
    fill_below(&heights, seed, color, haze, blur, depth)
}

/// A ridge covered with pine silhouettes.
#[allow(clippy::too_many_arguments)]
fn trees(
    base: f32,
    amplitude: f32,
    seed: u32,
    color: Color32,
    haze: Color32,
    density: f32,
    blur: f32,
    depth: f32,
) -> Layer {
    let mut heights: Vec<f32> = (0..W)
        .map(|x| {
            let u = x as f32 / W as f32;
            base - amplitude * 0.4 * (0.5 + 0.5 * fractal(seed, u * 3.0, 4, 0.5, 3))
        })
        .collect();
    // Scatter conifers: tall narrow triangles rising from the ridge.
    let count = (60.0 + 220.0 * density) as i64;
    for i in 0..count {
        let cx = hash(seed + 100, i) * W as f32;
        let tall = amplitude * (0.5 + hash(seed + 200, i)) * (0.6 + depth * 0.6);
        let half = W as f32 * (0.004 + 0.006 * hash(seed + 300, i)) * (0.6 + depth);
        let lo = (cx - half).max(0.0) as usize;
        let hi = ((cx + half) as usize).min(W - 1);
        for (x, h) in heights.iter_mut().enumerate().take(hi + 1).skip(lo) {
            let t = 1.0 - ((x as f32 - cx).abs() / half);
            // A slightly ragged edge reads as foliage.
            let ragged = 1.0 - 0.12 * hash(seed + 400, x as i64);
            let top = base - amplitude * 0.2 - tall * t * ragged;
            *h = h.min(top);
        }
    }
    fill_below(&heights, seed, color, haze, blur, depth)
}

fn fill_below(
    heights: &[f32],
    seed: u32,
    color: Color32,
    haze: Color32,
    blur: f32,
    depth: f32,
) -> Layer {
    // Soft, distant layers are painted at half resolution: they are blurred
    // anyway, so the GPU's upscaling is invisible and painting is 4× faster.
    let scale = if blur >= 2.0 { 2 } else { 1 };
    let (w, h) = (W / scale, H / scale);
    let mut c = Canvas::new(w, h);
    // Haze depends on altitude, measured from the layer's highest point, so
    // tall shapes (trees, peaks) don't drag a streak of colour below them.
    let summit = heights.iter().copied().fold(f32::INFINITY, f32::min) * h as f32;
    for x in 0..w {
        let top = heights[x * scale] * h as f32;
        for y in (top.floor().max(0.0) as usize)..h {
            // Anti-aliased top edge, then fade into haze towards the foot.
            let a = (y as f32 + 1.0 - top).clamp(0.0, 1.0);
            let below = (y as f32 - summit) / (h as f32 * 0.4);
            c.set(x, y, mix(color, haze, below), a);
        }
    }
    let blur = blur / scale as f32;
    c.blur(blur);
    Layer {
        image: c.into_image(seed),
        depth,
        drift: 0.0,
    }
}

/// A soft horizontal band of mist that drifts slowly sideways.
fn fog(center: f32, thickness: f32, color: Color32, opacity: f32, seed: u32, drift: f32) -> Layer {
    let (w, h) = (W / 2, H / 2);
    let mut c = Canvas::new(w, h);
    let rgb = [
        f32::from(color.r()),
        f32::from(color.g()),
        f32::from(color.b()),
    ];
    for x in 0..w {
        let u = x as f32 / w as f32;
        // Periodic noise so the band tiles seamlessly as it drifts.
        let wobble = fractal(seed, u * 3.0, 4, 0.5, 3);
        let density = 0.55 + 0.45 * fractal(seed + 7, u * 5.0, 3, 0.5, 5);
        for y in 0..h {
            let v = y as f32 / h as f32;
            let d = (v - center - wobble * thickness * 0.6) / thickness;
            let a = (-d * d).exp() * opacity * density;
            if a > 0.003 {
                c.set(x, y, rgb, a);
            }
        }
    }
    c.blur(4.0);
    Layer {
        image: c.into_image(seed),
        depth: 0.5,
        drift,
    }
}

/// Calm water below a horizon, with faint horizontal sheen.
fn sea(horizon: f32, near: Color32, far: Color32, depth: f32) -> Layer {
    let mut c = Canvas::new(W, H);
    let top = (horizon * H as f32) as usize;
    for y in top..H {
        let v = (y - top) as f32 / (H - top) as f32;
        for x in 0..W {
            let u = x as f32 / W as f32;
            let mut rgb = mix(near, far, v.powf(0.7));
            let sheen = (TAU * (v * v * 60.0 + fractal(71, u * 6.0, 3, 0.5, 6) * 0.8)).sin();
            let k = (sheen * 0.5 + 0.5).powi(24) * 0.1 * (1.0 - v);
            for ch in &mut rgb {
                *ch += (255.0 - *ch) * k;
            }
            c.set(x, y, rgb, 1.0);
        }
    }
    c.blur(0.8);
    Layer {
        image: c.into_image(70),
        depth,
        drift: 0.0,
    }
}

/// Warm paper with a fine grain.
fn paper(color: Color32) -> Layer {
    let (w, h) = (W, H);
    let mut c = Canvas::new(w, h);
    let base = [
        f32::from(color.r()),
        f32::from(color.g()),
        f32::from(color.b()),
    ];
    for y in 0..h {
        for x in 0..w {
            let grain = (hash(81, (y * w + x) as i64) - 0.5) * 7.0;
            let fibre = fractal(82, x as f32 / 40.0 + y as f32 / 300.0, 2, 0.5, 64) * 2.5;
            let shade = grain + fibre;
            c.set(
                x,
                y,
                [base[0] + shade, base[1] + shade, base[2] + shade * 1.1],
                1.0,
            );
        }
    }
    Layer {
        image: c.into_image(80),
        depth: 0.0,
        drift: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_builds_with_a_sky_and_readable_ink() {
        for (name, kind) in ThemeKind::ALL {
            let scene = kind.build();
            assert!(!scene.layers.is_empty(), "{name}");
            assert_eq!(scene.dark, kind.is_dark(), "{name}");
            // The back layer covers the whole screen.
            assert!(
                scene.layers[0].image.pixels.iter().all(|p| p.a() == 255),
                "{name}"
            );
            let ink_lum =
                f32::from(scene.ink.r()) + f32::from(scene.ink.g()) + f32::from(scene.ink.b());
            assert_eq!(ink_lum > 382.0, scene.dark, "{name}: ink contrast");
            for layer in &scene.layers {
                assert!((0.0..=1.0).contains(&layer.depth), "{name}");
            }
        }
    }

    #[test]
    fn thumbnails_are_opaque_previews_of_the_scene() {
        let thumb = ThemeKind::NightSky.build().thumbnail(46, 30);
        assert_eq!(thumb.size, [46, 30]);
        assert!(thumb.pixels.iter().all(|p| p.a() == 255));
        let dark = thumb.pixels.iter().map(|p| u32::from(p.r())).sum::<u32>() / 1380;
        assert!(dark < 80, "night sky should look dark, got {dark}");
    }

    #[test]
    fn periodic_noise_tiles_seamlessly() {
        for seed in [1, 7, 42] {
            let a = fractal(seed, 0.0, 4, 0.5, 3);
            let b = fractal(seed, 3.0, 4, 0.5, 3);
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn blur_preserves_total_coverage() {
        let mut c = Canvas::new(32, 32);
        c.set(16, 16, [255.0; 3], 1.0);
        let before: f32 = c.px.iter().map(|p| p[3]).sum();
        c.blur(3.0);
        let after: f32 = c.px.iter().map(|p| p[3]).sum();
        assert!((before - after).abs() / before < 0.01);
    }

    #[test]
    fn custom_images_pick_contrasting_ink() {
        let light = ColorImage::new([2, 2], vec![Color32::from_gray(230); 4]);
        let dark = ColorImage::new([2, 2], vec![Color32::from_gray(20); 4]);
        assert!(!from_image(light).dark);
        assert!(from_image(dark).dark);
    }
}
