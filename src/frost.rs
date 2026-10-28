//! Frosted glass for the sheet behind the text: a small, heavily blurred
//! copy of each scene layer, drawn inside the page so the landscape shows
//! through softly. Made once per theme on the loader thread; the GPU's
//! smooth upscaling finishes the blur.

use eframe::egui::{Color32, ColorImage};

/// Width the layer is reduced to before blurring. Small enough to blur
/// cheaply, and the reduction itself is half the softening.
const WIDTH: usize = 240;
/// Box-blur radius at that size; three passes approximate a Gaussian
/// (about 28 px of blur on a 1920 px wide screen).
const RADIUS: usize = 3;

/// A blurred copy of `image` (premultiplied RGBA). `wraps` is for layers
/// that tile horizontally (drifting fog), so the blur wraps too and the
/// seam stays invisible.
pub fn frost(image: &ColorImage, wraps: bool) -> ColorImage {
    let [w, h] = image.size;
    if w == 0 || h == 0 {
        return image.clone();
    }
    let factor = w.div_ceil(WIDTH).max(1);
    let (sw, sh) = (w.div_ceil(factor), h.div_ceil(factor));

    // Average each factor × factor block (premultiplied, so this is exact).
    let mut sum = vec![[0.0_f32; 5]; sw * sh];
    for y in 0..h {
        let row = (y / factor) * sw;
        for x in 0..w {
            let p = image.pixels[y * w + x].to_array();
            let cell = &mut sum[row + x / factor];
            for c in 0..4 {
                cell[c] += f32::from(p[c]);
            }
            cell[4] += 1.0;
        }
    }
    let mut px: Vec<[f32; 4]> = sum
        .iter()
        .map(|s| [s[0] / s[4], s[1] / s[4], s[2] / s[4], s[3] / s[4]])
        .collect();

    for _ in 0..3 {
        px = box_pass(&px, sw, sh, true, wraps);
        px = box_pass(&px, sw, sh, false, false);
    }
    let pixels = px
        .iter()
        .map(|p| {
            let [r, g, b, a] = p.map(|v| v.round().clamp(0.0, 255.0) as u8);
            Color32::from_rgba_premultiplied(r, g, b, a)
        })
        .collect();
    ColorImage::new([sw, sh], pixels)
}

/// One running-sum box blur along rows (or columns). Edges either repeat
/// the last pixel or wrap around.
fn box_pass(px: &[[f32; 4]], w: usize, h: usize, horizontal: bool, wraps: bool) -> Vec<[f32; 4]> {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: isize| {
        let i = if wraps {
            i.rem_euclid(len as isize) as usize
        } else {
            i.clamp(0, len as isize - 1) as usize
        };
        if horizontal {
            px[line * w + i]
        } else {
            px[i * w + line]
        }
    };
    let r = RADIUS as isize;
    let window = (2 * RADIUS + 1) as f32;
    let mut out = vec![[0.0; 4]; px.len()];
    for line in 0..lines {
        let mut acc = [0.0_f32; 4];
        for i in -r..=r {
            let p = at(line, i);
            for c in 0..4 {
                acc[c] += p[c];
            }
        }
        for i in 0..len {
            let o = if horizontal {
                &mut out[line * w + i]
            } else {
                &mut out[i * w + line]
            };
            *o = acc.map(|v| v / window);
            let (add, sub) = (at(line, i as isize + r + 1), at(line, i as isize - r));
            for c in 0..4 {
                acc[c] += add[c] - sub[c];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frost_is_small_smooth_and_keeps_the_colour() {
        // Left half black, right half white.
        let (w, h) = (1920, 1080);
        let pixels = (0..w * h)
            .map(|i| {
                if i % w < w / 2 {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                }
            })
            .collect();
        let frosted = frost(&ColorImage::new([w, h], pixels), false);
        let [fw, fh] = frosted.size;
        assert!(fw <= WIDTH && fh < h / 4, "{fw}×{fh}");

        let row: Vec<u8> = (0..fw)
            .map(|x| frosted.pixels[fh / 2 * fw + x].r())
            .collect();
        assert!(
            row[0] < 5 && row[fw - 1] > 250,
            "the ends keep their colour"
        );
        let mid = row[fw / 2 - 2..fw / 2 + 2].to_vec();
        assert!(
            mid.iter().all(|&v| v > 20 && v < 235),
            "the edge is soft: {mid:?}"
        );
        assert!(frosted.pixels.iter().all(|p| p.a() == 255), "still opaque");
    }

    #[test]
    fn tiling_layers_blur_across_the_seam() {
        // A bright column at the left edge bleeds round to the right edge.
        let (w, h) = (480, 10);
        let pixels = (0..w * h)
            .map(|i| {
                if i % w < 4 {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                }
            })
            .collect();
        let image = ColorImage::new([w, h], pixels);
        let right_edge = |img: &ColorImage| img.pixels[img.size[0] - 1].r();
        assert!(right_edge(&frost(&image, true)) > right_edge(&frost(&image, false)));
    }
}
