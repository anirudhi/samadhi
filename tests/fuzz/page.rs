//! Page geometry for arbitrary screens, pages and drags.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
use proptest::prelude::*;
use samadhi::background::cover_uv;
use samadhi::page::{
    Drag, Edges, MIN_SIZE, apply_drag, clamp_into, default_rect, handles, hit_test,
};

use crate::config;

const EPS: f32 = 1e-2;

fn coord() -> impl Strategy<Value = f32> {
    // Screens to the left of or above the primary one have negative
    // coordinates; much beyond this, f32 rounding exceeds `EPS`.
    prop_oneof![-5000.0f32..5000.0, -20_000.0f32..20_000.0]
}

/// A screen of any plausible size, from a postage stamp to a video wall.
fn screen() -> impl Strategy<Value = Rect> {
    (coord(), coord(), 1.0f32..8000.0, 1.0f32..5000.0)
        .prop_map(|(x, y, w, h)| Rect::from_min_size(pos2(x, y), vec2(w, h)))
}

/// Any page, including ones larger than the screen or far off it.
fn any_rect() -> impl Strategy<Value = Rect> {
    (coord(), coord(), 0.0f32..10_000.0, 0.0f32..10_000.0)
        .prop_map(|(x, y, w, h)| Rect::from_min_size(pos2(x, y), vec2(w, h)))
}

fn edges() -> impl Strategy<Value = Edges> {
    any::<[bool; 4]>().prop_map(|[left, right, top, bottom]| Edges {
        left,
        right,
        top,
        bottom,
    })
}

/// The drags the app can start: moving, or one of the eight handles.
fn drag() -> impl Strategy<Value = Drag> {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 1000.0));
    let handles: Vec<Drag> = handles(screen)
        .into_iter()
        .map(|(_, e)| Drag::Resize(e))
        .collect();
    prop_oneof![Just(Drag::Move), prop::sample::select(handles)]
}

/// Any combination of edges, including ones no handle produces.
fn any_drag() -> impl Strategy<Value = Drag> {
    prop_oneof![Just(Drag::Move), edges().prop_map(Drag::Resize)]
}

fn inside(outer: Rect, inner: Rect) -> bool {
    outer.expand(EPS).contains_rect(inner)
}

/// The smallest a page may be on this screen.
fn min_size(bounds: Rect) -> Vec2 {
    MIN_SIZE.min(bounds.size())
}

proptest! {
    #![proptest_config(config(512))]

    #[test]
    fn clamped_pages_fit_the_screen(rect in any_rect(), bounds in screen()) {
        let r = clamp_into(rect, bounds);
        prop_assert!(inside(bounds, r), "{:?} not in {:?}", r, bounds);
        prop_assert!(r.width() >= min_size(bounds).x - EPS);
        prop_assert!(r.height() >= min_size(bounds).y - EPS);
        // Already fitting pages are left alone.
        if inside(bounds, rect) && rect.width() >= MIN_SIZE.x && rect.height() >= MIN_SIZE.y {
            prop_assert!((r.min - rect.min).length() < EPS && (r.max - rect.max).length() < EPS);
        }
    }

    #[test]
    fn default_page_fits_any_screen(bounds in screen()) {
        let r = default_rect(bounds);
        prop_assert!(inside(bounds, r));
        prop_assert!(r.width() > 0.0 && r.height() > 0.0);
    }

    /// Dragging keeps the page on screen and at least the minimum size (or
    /// the whole screen, on one smaller than that), on any screen.
    #[test]
    fn drags_keep_the_page_on_screen(
        bounds in screen(),
        rect in any_rect(),
        drag in drag(),
        delta in (coord(), coord()).prop_map(|(x, y)| vec2(x, y)),
    ) {
        let page = clamp_into(rect, bounds);
        let r = apply_drag(page, drag, delta, bounds);
        prop_assert!(inside(bounds, r), "{:?} not in {:?}", r, bounds);
        let min = min_size(bounds);
        prop_assert!(r.width() >= min.x - EPS && r.height() >= min.y - EPS);
    }

    #[test]
    fn hit_testing_never_panics(rect in any_rect(), x in coord(), y in coord(), other in any_drag()) {
        let hit = hit_test(rect, pos2(x, y));
        if let Some(drag) = hit {
            let _ = drag.cursor();
        }
        let _ = other.cursor();
        prop_assert_eq!(handles(rect).len(), 8);
    }

    /// `cover_uv` picks a centred sub-rectangle of the image that spans it
    /// fully in one direction.
    #[test]
    fn cover_uv_is_a_centred_crop(
        iw in 1e-3f32..1e6, ih in 1e-3f32..1e6, tw in 1e-3f32..1e6, th in 1e-3f32..1e6,
    ) {
        let uv = cover_uv(vec2(iw, ih), vec2(tw, th));
        let unit = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        prop_assert!(unit.expand(1e-5).contains_rect(uv), "{:?}", uv);
        prop_assert!(uv.width() >= 0.0 && uv.height() >= 0.0);
        prop_assert!((uv.center() - pos2(0.5, 0.5)).length() < 1e-4);
        prop_assert!((uv.width() - 1.0).abs() < 1e-5 || (uv.height() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cover_uv_degenerate_sizes_use_the_whole_image(
        iw in -10.0f32..10.0, ih in -10.0f32..10.0, tw in -10.0f32..10.0, th in -10.0f32..10.0,
    ) {
        prop_assume!(iw <= 0.0 || ih <= 0.0 || tw <= 0.0 || th <= 0.0);
        let uv = cover_uv(vec2(iw, ih), vec2(tw, th));
        prop_assert_eq!(uv, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)));
    }
}

/// In a window smaller than the minimum page size (the app sets no minimum
/// window size), the page is clamped to the window and then grabbing a
/// handle panics: `f32::clamp` is called with `min > max`.
#[test]
fn resizing_in_a_tiny_window_does_not_panic() {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(150.0, 100.0));
    let page = clamp_into(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)), screen);
    let left = Drag::Resize(Edges {
        left: true,
        ..Edges::default()
    });
    let r = apply_drag(page, left, vec2(1.0, 0.0), screen);
    assert!(screen.contains_rect(r));
}
