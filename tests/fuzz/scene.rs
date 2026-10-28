//! Scenes built from arbitrary user images.

use eframe::egui::{Color32, ColorImage};
use proptest::prelude::*;
use samadhi::scene::{ThemeKind, from_image};

use crate::config;

proptest! {
    #![proptest_config(config(48))]

    #[test]
    fn user_images_make_scenes_and_thumbnails(
        w in 1usize..80,
        h in 1usize..80,
        pixels in prop::collection::vec(any::<[u8; 4]>(), 1..64),
        tw in 1usize..64,
        th in 1usize..64,
    ) {
        let image = ColorImage::new(
            [w, h],
            (0..w * h)
                .map(|i| {
                    let [r, g, b, a] = pixels[i % pixels.len()];
                    Color32::from_rgba_unmultiplied(r, g, b, a)
                })
                .collect(),
        );
        let scene = from_image(image);
        prop_assert!(!scene.layers.is_empty());
        for layer in &scene.layers {
            prop_assert!((0.0..=1.0).contains(&layer.depth));
            prop_assert!(layer.drift.is_finite());
        }
        let thumb = scene.thumbnail(tw, th);
        prop_assert_eq!(thumb.size, [tw, th]);
        prop_assert_eq!(thumb.pixels.len(), tw * th);
    }
}

proptest! {
    #![proptest_config(config(16))]

    /// Thumbnails of any size from every built-in theme.
    #[test]
    fn builtin_thumbnails_have_the_requested_size(
        theme in 0usize..ThemeKind::ALL.len(),
        tw in 1usize..120,
        th in 1usize..80,
    ) {
        let (_, kind) = ThemeKind::ALL[theme];
        let thumb = kind.build().thumbnail(tw, th);
        prop_assert_eq!(thumb.size, [tw, th]);
    }
}

#[test]
fn zero_sized_thumbnails_are_empty() {
    let scene = from_image(ColorImage::new([2, 2], vec![Color32::GRAY; 4]));
    for (w, h) in [(0, 0), (0, 5), (5, 0)] {
        assert_eq!(scene.thumbnail(w, h).pixels.len(), 0);
    }
}
