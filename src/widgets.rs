//! Minimal, chrome-free controls: bare icons, text choices and hairline
//! sliders that sit directly on the scene.

use eframe::egui::{self, Color32, FontId, Painter, Rect, Sense, Stroke, Vec2, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme;

pub const ICON_BUTTON: f32 = 26.0;
/// Drawn size of an icon inside its button.
/// Lucide is drawn on a 24-unit grid; at 16 px its 1.5 strokes are exactly
/// one pixel wide, which renders crisply.
const ICON_SIZE: f32 = 16.0;
const SLIDER_WIDTH: f32 = 88.0;
/// Where the baseline of text choices sits within their row.
const CHOICE_BASELINE: f32 = 18.0;

/// Rounds `p` to the nearest physical pixel, so small text and icons are
/// drawn 1:1 instead of being smeared across two pixels (most visible on
/// 1080p and other standard-density screens).
fn snap(ui: &egui::Ui, p: egui::Pos2) -> egui::Pos2 {
    let ppp = ui.ctx().pixels_per_point();
    pos2((p.x * ppp).round() / ppp, (p.y * ppp).round() / ppp)
}

/// One physical pixel, in points.
fn one_px(ui: &egui::Ui) -> f32 {
    1.0 / ui.ctx().pixels_per_point()
}

/// The opposite tone of `ink`, used as a faint halo so marks stay legible
/// on busy photographs.
fn halo(ink: Color32) -> Color32 {
    let light = u16::from(ink.r()) + u16::from(ink.g()) + u16::from(ink.b()) > 382;
    if light {
        Color32::from_black_alpha(40)
    } else {
        Color32::from_white_alpha(50)
    }
}

/// Paints `icon` with a soft halo behind it.
fn paint_icon(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let h = halo(color);
    // A single faint one-pixel shadow keeps edges crisp (a blurred halo of
    // several offset copies softens them).
    let px = 1.0 / painter.ctx().pixels_per_point();
    icons::paint(icon, painter, rect.translate(vec2(0.0, px)), h);
    icons::paint(icon, painter, rect, color);
}

/// A bare icon button: slightly faded at rest, solid on hover, accent when
/// `active` (selected section or toggled on).
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, label: &str, active: bool) -> egui::Response {
    icon_button_bare(ui, icon, label, active).on_hover_text(label)
}

/// An [`icon_button`] without a tooltip, for buttons whose hover shows
/// something else in its place.
pub fn icon_button_bare(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    active: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_BUTTON), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let p = theme::current(ui);
    let color = if active {
        p.accent
    } else if response.hovered() {
        p.ink
    } else {
        p.ink.gamma_multiply(0.72)
    };
    paint_icon(
        ui.painter(),
        icon,
        Rect::from_center_size(rect.center(), Vec2::splat(ICON_SIZE)),
        color,
    );
    response
}

/// A word set in `font`; the selected one is in the accent colour with a
/// small dot beneath it.
pub fn text_choice(ui: &mut egui::Ui, text: &str, font: FontId, selected: bool) -> egui::Response {
    let p = theme::current(ui);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, p.ink);
    let size = vec2(galley.size().x + 14.0, ICON_BUTTON);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, text)
    });
    let color = if selected {
        p.accent
    } else if response.hovered() {
        p.ink
    } else {
        p.ink.gamma_multiply(0.62)
    };
    // Place every face on the same baseline (their ascents differ, so
    // centring each one vertically would leave them ragged).
    let baseline = galley
        .rows
        .first()
        .and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y))
        .unwrap_or(galley.size().y * 0.8);
    let top_left = snap(
        ui,
        pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.top() + CHOICE_BASELINE - baseline,
        ),
    );
    ui.painter().galley_with_override_text_color(
        top_left + vec2(0.0, one_px(ui)),
        galley.clone(),
        halo(color),
    );
    ui.painter()
        .galley_with_override_text_color(top_left, galley, color);
    if selected {
        ui.painter()
            .circle_filled(pos2(rect.center().x, rect.bottom() - 4.0), 2.0, p.accent);
    }
    response
}

/// A hairline 0–1 slider with a small round handle.
pub fn slider(ui: &mut egui::Ui, value: &mut f32, label: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(SLIDER_WIDTH, ICON_BUTTON), Sense::click_and_drag());
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), label));
    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let v = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            changed = true;
        }
    }
    let p = theme::current(ui);
    let y = rect.center().y;
    let x = rect.left() + *value * rect.width();
    let painter = ui.painter();
    let strong = response.hovered() || response.dragged();
    painter.line_segment(
        [pos2(rect.left(), y), pos2(rect.right(), y)],
        Stroke::new(1.0, p.ink.gamma_multiply(0.28)),
    );
    painter.line_segment(
        [pos2(rect.left(), y), pos2(x, y)],
        Stroke::new(1.5, p.ink.gamma_multiply(if strong { 0.9 } else { 0.7 })),
    );
    painter.circle_filled(pos2(x, y), if strong { 5.0 } else { 4.0 }, p.ink);
    response.on_hover_text(label);
    changed
}

/// Small caption text with a halo, for flyout titles and details.
pub fn caption(ui: &mut egui::Ui, text: &str, size: f32, color: Color32, spacing: f32) {
    let font = FontId::proportional(size);
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    job.wrap.max_width = f32::INFINITY;
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let at = snap(ui, rect.min);
    ui.painter().galley_with_override_text_color(
        at + vec2(0.0, one_px(ui)),
        galley.clone(),
        halo(color),
    );
    ui.painter().galley(at, galley, color);
}

/// A theme preview tile; the selected one is ringed in the accent colour.
pub fn thumb_button(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    name: &str,
    selected: bool,
) -> egui::Response {
    let size = vec2(46.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, name)
    });
    let p = theme::current(ui);
    let radius = 4.0;
    match texture {
        Some(tex) => {
            egui::Image::from_texture(tex)
                .corner_radius(radius)
                .paint_at(ui, rect);
        }
        None => {
            ui.painter()
                .rect_filled(rect, radius, p.ink.gamma_multiply(0.12));
        }
    }
    let ring = if selected {
        Some(Stroke::new(1.5, p.accent))
    } else if response.hovered() {
        Some(Stroke::new(1.0, p.ink.gamma_multiply(0.6)))
    } else {
        None
    };
    if let Some(ring) = ring {
        ui.painter().rect_stroke(
            rect.expand(2.5),
            radius + 2.0,
            ring,
            egui::StrokeKind::Middle,
        );
    }
    response.on_hover_text(name)
}

/// A name in a list; the selected one is in the accent colour with a dot.
pub fn list_choice(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let p = theme::current(ui);
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), p.ink);
    let size = vec2(galley.size().x + 18.0, 21.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, text)
    });
    let color = if selected {
        p.accent
    } else if response.hovered() {
        p.ink
    } else {
        p.ink.gamma_multiply(0.68)
    };
    let left = rect.left() + 12.0;
    let y = rect.center().y;
    if selected {
        ui.painter()
            .circle_filled(pos2(rect.left() + 5.0, y), 2.2, p.accent);
    }
    // Lay out once and place the galley on a whole pixel.
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let at = snap(ui, pos2(left, y - galley.size().y / 2.0));
    ui.painter().galley_with_override_text_color(
        at + vec2(0.0, one_px(ui)),
        galley.clone(),
        halo(color),
    );
    ui.painter().galley(at, galley, color);
    response
}

/// An icon and a small switch; the name appears as a tooltip. On: the track
/// fills with the accent colour and the knob slides right. Off: an outlined
/// track with the knob on the left.
pub fn switch(ui: &mut egui::Ui, icon: Icon, label: &str, on: bool) -> egui::Response {
    const TRACK: Vec2 = vec2(24.0, 13.0);
    let p = theme::current(ui);
    let width = ICON_SIZE + 6.0 + TRACK.x + 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, ICON_BUTTON), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), on, label));

    let hovered = response.hovered();
    let ink = if hovered {
        p.ink
    } else {
        p.ink.gamma_multiply(0.8)
    };
    let y = rect.center().y;
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + ICON_SIZE / 2.0, y),
        Vec2::splat(ICON_SIZE),
    );
    paint_icon(ui.painter(), icon, icon_rect, ink);

    // Track and knob, with the knob easing across.
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), on, 0.12);
    let track = Rect::from_center_size(pos2(rect.right() - 4.0 - TRACK.x / 2.0, y), TRACK);
    let radius = TRACK.y / 2.0;
    let off_line = p.ink.gamma_multiply(if hovered { 0.7 } else { 0.5 });
    if t > 0.0 {
        ui.painter()
            .rect_filled(track, radius, p.accent.gamma_multiply(t));
    }
    if t < 1.0 {
        ui.painter().rect_stroke(
            track,
            radius,
            Stroke::new(1.0, off_line.gamma_multiply(1.0 - t)),
            egui::StrokeKind::Inside,
        );
    }
    let knob_x = egui::lerp(track.left() + radius..=track.right() - radius, t);
    let knob_color = Color32::WHITE
        .gamma_multiply(t)
        .blend(off_line.gamma_multiply(1.0 - t));
    ui.painter()
        .circle_filled(pos2(knob_x, y), radius - 2.5, knob_color);
    response.on_hover_text(if on {
        format!("{label}: on")
    } else {
        format!("{label}: off")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    /// Presses at `x` (a fraction of the slider's width) and releases at
    /// `to`, returning the harness so the result can be read.
    fn slide(start: f32, x: f32, to: f32) -> Harness<'static, (f32, usize)> {
        let mut h = Harness::new_ui_state(
            |ui, (value, changes): &mut (f32, usize)| {
                if slider(ui, value, "Volume") {
                    *changes += 1;
                }
            },
            (start, 0),
        );
        let rect = h.get_by_role_and_label(Role::Slider, "Volume").rect();
        let at = |f: f32| pos2(rect.left() + f * rect.width(), rect.center().y);
        h.hover_at(at(x));
        h.run();
        h.drag_at(at(x));
        h.run();
        h.hover_at(at(to));
        h.run();
        h.drop_at(at(to));
        h.run();
        h
    }

    #[test]
    fn clicking_a_slider_jumps_to_that_point() {
        let h = slide(0.0, 0.25, 0.25);
        let (value, changes) = *h.state();
        assert!((value - 0.25).abs() < 0.02, "{value}");
        assert!(changes >= 1);
    }

    #[test]
    fn dragging_a_slider_follows_and_clamps() {
        let h = slide(0.5, 0.5, 3.0);
        assert!((h.state().0 - 1.0).abs() < f32::EPSILON);
        let h = slide(0.5, 0.5, -2.0);
        assert!(h.state().0.abs() < f32::EPSILON);
    }

    #[test]
    fn sliders_report_their_value_to_assistive_tech() {
        let h = Harness::new_ui(|ui| {
            let mut v = 0.4;
            slider(ui, &mut v, "Music volume");
        });
        let node = h.get_by_role_and_label(Role::Slider, "Music volume");
        let value = node.accesskit_node().numeric_value().unwrap();
        assert!((value - 0.4).abs() < 1e-6);
        assert_eq!(node.rect().size(), vec2(SLIDER_WIDTH, ICON_BUTTON));
    }

    #[test]
    fn switches_toggle_and_say_so() {
        let mut h = Harness::new_ui_state(
            |ui, on: &mut bool| {
                if switch(ui, Icon::Motion, "Motion", *on).clicked() {
                    *on = !*on;
                }
            },
            false,
        );
        let toggled = |h: &Harness<'_, bool>| {
            h.get_by_role_and_label(Role::Switch, "Motion")
                .accesskit_node()
                .toggled()
        };
        assert_eq!(toggled(&h), Some(Toggled::False));
        h.get_by_role_and_label(Role::Switch, "Motion").click();
        h.run();
        assert!(*h.state());
        assert_eq!(toggled(&h), Some(Toggled::True));
        h.get_by_role_and_label(Role::Switch, "Motion").click();
        h.run();
        assert!(!*h.state());
    }

    #[test]
    fn icon_buttons_are_labelled_and_clickable() {
        let mut h = Harness::new_ui_state(
            |ui, clicks: &mut u32| {
                if icon_button(ui, Icon::Save, "Save", false).clicked() {
                    *clicks += 1;
                }
                if icon_button_bare(ui, Icon::Open, "Open", true).clicked() {
                    *clicks += 100;
                }
            },
            0,
        );
        let save = h.get_by_role_and_label(Role::Button, "Save");
        assert_eq!(save.rect().size(), Vec2::splat(ICON_BUTTON));
        save.click();
        h.run();
        assert_eq!(*h.state(), 1);
        h.get_by_role_and_label(Role::Button, "Open").click();
        h.run();
        assert_eq!(*h.state(), 101);
    }

    #[test]
    fn hovering_an_icon_button_shows_its_name() {
        let mut h = Harness::new_ui(|ui| {
            icon_button(ui, Icon::Timer, "Focus timer", false);
        });
        h.get_by_role_and_label(Role::Button, "Focus timer").hover();
        // Tooltips appear after a short delay.
        for _ in 0..10 {
            h.run();
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        assert!(h.get_all_by_label("Focus timer").count() >= 2);
    }

    #[test]
    fn choices_report_selection() {
        let mut h = Harness::new_ui_state(
            |ui, picked: &mut usize| {
                for (i, name) in ["Serif", "Sans"].into_iter().enumerate() {
                    if text_choice(ui, name, FontId::proportional(14.0), *picked == i).clicked() {
                        *picked = i;
                    }
                }
                for (i, name) in ["Rain", "Waves"].into_iter().enumerate() {
                    if list_choice(ui, name, *picked == i + 2).clicked() {
                        *picked = i + 2;
                    }
                }
                if thumb_button(ui, None, "Hills", *picked == 4).clicked() {
                    *picked = 4;
                }
            },
            0,
        );
        // egui reports a selected button as toggled on.
        let selected = |h: &Harness<'_, usize>, name: &str| {
            h.get_by_role_and_label(Role::Button, name)
                .accesskit_node()
                .toggled()
                .map(|t| t == Toggled::True)
        };
        assert_eq!(selected(&h, "Serif"), Some(true));
        assert_eq!(selected(&h, "Sans"), Some(false));
        for (name, index) in [("Sans", 1), ("Waves", 3), ("Hills", 4), ("Rain", 2)] {
            h.get_by_role_and_label(Role::Button, name).click();
            h.run();
            assert_eq!(*h.state(), index, "{name}");
            assert_eq!(selected(&h, name), Some(true), "{name}");
        }
    }

    #[test]
    fn captions_take_room_for_their_text() {
        let h = Harness::new_ui(|ui| {
            caption(ui, "MUSIC", 11.0, Color32::WHITE, 1.5);
            caption(ui, "", 11.0, Color32::BLACK, 0.0);
            caption(ui, "Ünïcødé — “quoted”", 30.0, Color32::GRAY, -2.0);
        });
        drop(h);
    }

    #[test]
    fn halos_contrast_with_the_ink() {
        assert!(halo(Color32::WHITE).r() < 128);
        assert_eq!(halo(Color32::WHITE), Color32::from_black_alpha(40));
        assert_eq!(halo(Color32::BLACK), Color32::from_white_alpha(50));
    }

    #[test]
    fn snapping_lands_on_physical_pixels() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(2.0);
        let mut snapped = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            snapped = Some((snap(ui, pos2(10.3, 4.74)), one_px(ui)));
        });
        output.textures_delta.clear();
        let (p, px) = snapped.unwrap();
        assert!(
            (p.x - 10.5).abs() < 1e-6 && (p.y - 4.5).abs() < 1e-6,
            "{p:?}"
        );
        assert!((px - 0.5).abs() < f32::EPSILON);
    }
}
