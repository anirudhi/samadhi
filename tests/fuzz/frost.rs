//! Frosted glass from arbitrary images.

use eframe::egui::{Color32, ColorImage};
use proptest::prelude::*;
use samadhi::frost::frost;

use crate::config;

/// A premultiplied image filled from a seed (much faster than generating
/// every pixel through proptest).
fn image(w: usize, h: usize, seed: u32, opaque: bool) -> ColorImage {
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let pixels = (0..w * h)
        .map(|_| {
            let v = next();
            let a = if opaque { 255 } else { (v >> 24) as u8 };
            // Scaled by alpha, so never brighter than the pixel is opaque.
            let channel = |shift: u32| (((v >> shift) & 0xff) * u32::from(a) / 255) as u8;
            Color32::from_rgba_premultiplied(channel(0), channel(8), channel(16), a)
        })
        .collect();
    ColorImage::new([w, h], pixels)
}

proptest! {
    #![proptest_config(config(64))]

    #[test]
    fn frost_shrinks_and_stays_premultiplied(
        w in 1usize..600,
        h in 1usize..300,
        seed in any::<u32>(),
        opaque in any::<bool>(),
        wraps in any::<bool>(),
    ) {
        let input = image(w, h, seed, opaque);
        let out = frost(&input, wraps);
        let [ow, oh] = out.size;
        prop_assert!(ow >= 1 && oh >= 1);
        prop_assert!(ow <= w && oh <= h);
        prop_assert!(ow <= 240);
        prop_assert_eq!(out.pixels.len(), ow * oh);
        for p in &out.pixels {
            prop_assert!(p.r() <= p.a() && p.g() <= p.a() && p.b() <= p.a(), "{:?}", p);
            if opaque {
                prop_assert_eq!(p.a(), 255);
            }
        }
    }

    /// A flat colour stays that colour.
    #[test]
    fn frost_keeps_flat_colours(
        w in 1usize..600,
        h in 1usize..300,
        rgba in any::<[u8; 4]>(),
        wraps in any::<bool>(),
    ) {
        let [r, g, b, a] = rgba;
        let color = Color32::from_rgba_premultiplied(r.min(a), g.min(a), b.min(a), a);
        let out = frost(&ColorImage::new([w, h], vec![color; w * h]), wraps);
        for p in &out.pixels {
            for (x, y) in p.to_array().into_iter().zip(color.to_array()) {
                prop_assert!(x.abs_diff(y) <= 1, "{:?} became {:?}", color, p);
            }
        }
    }
}

#[test]
fn empty_images_pass_through() {
    for size in [[0, 0], [0, 5], [5, 0]] {
        let empty = ColorImage::new(size, Vec::new());
        assert_eq!(frost(&empty, false).size, size);
    }
}
