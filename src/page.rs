//! Geometry of the movable, resizable writing area ("page").

use eframe::egui::{CursorIcon, Pos2, Rect, Vec2, pos2, vec2};

pub const MIN_SIZE: Vec2 = vec2(200.0, 120.0);
const DEFAULT_MAX_WIDTH: f32 = 680.0;
/// Side of the square resize handles.
pub const HANDLE: f32 = 9.0;
/// How far either side of the border line still counts as grabbing it.
/// (The original only reacted on the 1 px line itself.)
const BORDER_GRAB: f32 = 5.0;

/// Which edges a resize drag moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // four independent edges
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag {
    Move,
    Resize(Edges),
}

impl Drag {
    pub fn cursor(self) -> CursorIcon {
        match self {
            Self::Move => CursorIcon::Move,
            Self::Resize(e) => match (e.left || e.right, e.top || e.bottom) {
                (true, true) if (e.left && e.top) || (e.right && e.bottom) => {
                    CursorIcon::ResizeNwSe
                }
                (true, true) => CursorIcon::ResizeNeSw,
                (true, false) => CursorIcon::ResizeHorizontal,
                _ => CursorIcon::ResizeVertical,
            },
        }
    }
}

/// The eight handles: corners and edge midpoints.
pub fn handles(rect: Rect) -> [(Rect, Edges); 8] {
    let e = |left, right, top, bottom| Edges {
        left,
        right,
        top,
        bottom,
    };
    let at = |p: Pos2| Rect::from_center_size(p, Vec2::splat(HANDLE));
    let c = rect.center();
    [
        (at(rect.left_top()), e(true, false, true, false)),
        (at(pos2(c.x, rect.top())), e(false, false, true, false)),
        (at(rect.right_top()), e(false, true, true, false)),
        (at(pos2(rect.left(), c.y)), e(true, false, false, false)),
        (at(pos2(rect.right(), c.y)), e(false, true, false, false)),
        (at(rect.left_bottom()), e(true, false, false, true)),
        (at(pos2(c.x, rect.bottom())), e(false, false, false, true)),
        (at(rect.right_bottom()), e(false, true, false, true)),
    ]
}

/// What dragging from `pos` would do: handles win over the border.
pub fn hit_test(rect: Rect, pos: Pos2) -> Option<Drag> {
    if let Some((_, edges)) = handles(rect)
        .into_iter()
        .find(|(h, _)| h.expand(2.0).contains(pos))
    {
        return Some(Drag::Resize(edges));
    }
    let on_border =
        rect.expand(BORDER_GRAB).contains(pos) && !rect.shrink(BORDER_GRAB).contains(pos);
    on_border.then_some(Drag::Move)
}

/// Applies a drag by `delta`, keeping the page at least [`MIN_SIZE`], never
/// inverted, and inside `bounds`.
pub fn apply_drag(rect: Rect, drag: Drag, delta: Vec2, bounds: Rect) -> Rect {
    match drag {
        Drag::Move => clamp_into(rect.translate(delta), bounds),
        Drag::Resize(e) => {
            let mut r = rect;
            if e.left {
                r.min.x = (r.min.x + delta.x).clamp(bounds.min.x, r.max.x - MIN_SIZE.x);
            }
            if e.right {
                r.max.x = (r.max.x + delta.x).clamp(r.min.x + MIN_SIZE.x, bounds.max.x);
            }
            if e.top {
                r.min.y = (r.min.y + delta.y).clamp(bounds.min.y, r.max.y - MIN_SIZE.y);
            }
            if e.bottom {
                r.max.y = (r.max.y + delta.y).clamp(r.min.y + MIN_SIZE.y, bounds.max.y);
            }
            r
        }
    }
}

/// Fits `rect` inside `bounds`, shrinking it only if it is larger.
pub fn clamp_into(rect: Rect, bounds: Rect) -> Rect {
    let size = rect
        .size()
        .min(bounds.size())
        .max(MIN_SIZE.min(bounds.size()));
    let max_min = (bounds.max - size).max(bounds.min);
    let min = rect.min.clamp(bounds.min, max_min);
    Rect::from_min_size(min, size)
}

/// A comfortable default: a pleasant reading measure (about 75 characters)
/// rather than half the screen, nearly full height, centred horizontally so
/// the dock fits to its right.
pub fn default_rect(screen: Rect) -> Rect {
    let width = (screen.width() * 0.5).clamp(MIN_SIZE.x, DEFAULT_MAX_WIDTH);
    let height = (screen.height() - 120.0).max(MIN_SIZE.y);
    let min = pos2(
        screen.center().x - width / 2.0,
        screen.min.y + (screen.height() - height) / 2.0,
    );
    clamp_into(Rect::from_min_size(min, vec2(width, height)), screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))
    }

    fn page() -> Rect {
        Rect::from_min_size(pos2(400.0, 100.0), vec2(800.0, 600.0))
    }

    #[test]
    fn corner_and_edge_handles_resize_border_moves_inside_types() {
        let p = page();
        let Some(Drag::Resize(e)) = hit_test(p, p.right_bottom()) else {
            panic!("expected resize")
        };
        assert!(e.right && e.bottom && !e.left && !e.top);
        let Some(Drag::Resize(e)) = hit_test(p, pos2(p.center().x, p.top())) else {
            panic!("expected resize")
        };
        assert!(e.top && !e.left && !e.right && !e.bottom);
        assert_eq!(hit_test(p, pos2(p.left() + 2.0, 300.0)), Some(Drag::Move));
        assert_eq!(hit_test(p, p.center()), None);
        assert_eq!(hit_test(p, pos2(10.0, 10.0)), None);
    }

    #[test]
    fn resize_respects_minimum_size_and_never_inverts() {
        let p = page();
        let left = Drag::Resize(Edges {
            left: true,
            ..Edges::default()
        });
        let r = apply_drag(p, left, vec2(5000.0, 0.0), screen());
        assert!((r.width() - MIN_SIZE.x).abs() < 1e-3);
        assert!((r.max.x - p.max.x).abs() < 1e-3);
    }

    #[test]
    fn resize_and_move_stay_on_screen() {
        let p = page();
        let br = Drag::Resize(Edges {
            right: true,
            bottom: true,
            ..Edges::default()
        });
        let r = apply_drag(p, br, vec2(5000.0, 5000.0), screen());
        assert_eq!(r.max, screen().max);
        let moved = apply_drag(p, Drag::Move, vec2(-5000.0, 5000.0), screen());
        assert_eq!(moved.size(), p.size());
        assert!(screen().contains_rect(moved));
    }

    #[test]
    fn clamp_shrinks_oversized_pages_from_a_bigger_monitor() {
        let big = Rect::from_min_size(pos2(3000.0, 2000.0), vec2(3000.0, 2000.0));
        let r = clamp_into(big, screen());
        assert_eq!(r, screen());
    }

    #[test]
    fn default_page_is_centred_and_on_screen() {
        let r = default_rect(screen());
        assert!((r.center().x - screen().center().x).abs() < 1e-3);
        assert!(screen().contains_rect(r));
    }

    #[test]
    fn cursors_match_drag_direction() {
        let e = |left, right, top, bottom| {
            Drag::Resize(Edges {
                left,
                right,
                top,
                bottom,
            })
        };
        assert_eq!(e(true, false, true, false).cursor(), CursorIcon::ResizeNwSe);
        assert_eq!(e(false, true, true, false).cursor(), CursorIcon::ResizeNeSw);
        assert_eq!(
            e(false, false, true, false).cursor(),
            CursorIcon::ResizeVertical
        );
        assert_eq!(
            e(true, false, false, false).cursor(),
            CursorIcon::ResizeHorizontal
        );
        assert_eq!(Drag::Move.cursor(), CursorIcon::Move);
    }
}
