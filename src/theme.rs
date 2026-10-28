//! The app's calm visual language: soft frosted panels, gentle shadows and
//! muted ink, in a light and a dark variant that follow the current theme.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle,
};

pub const PANEL_RADIUS: u8 = 14;
pub const BUTTON_RADIUS: u8 = 10;
pub const DANGER: Color32 = Color32::from_rgb(0xc2, 0x55, 0x45);

/// Colours of the interface chrome for a light or dark theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub ink: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub panel_fill: Color32,
    pub panel_stroke: Color32,
    pub window_fill: Color32,
    /// Overlay used for hovered/selected icon buttons.
    pub highlight: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            ink: Color32::from_rgb(0xe8, 0xe9, 0xee),
            muted: Color32::from_rgb(0x9a, 0x9d, 0xa8),
            accent: Color32::from_rgb(0x86, 0xc0, 0xb6),
            panel_fill: Color32::from_rgba_unmultiplied(26, 28, 36, 200),
            panel_stroke: Color32::from_white_alpha(18),
            window_fill: Color32::from_rgb(0x22, 0x24, 0x2c),
            highlight: Color32::from_white_alpha(22),
        }
    } else {
        Palette {
            ink: Color32::from_rgb(0x33, 0x35, 0x3b),
            muted: Color32::from_rgb(0x7b, 0x7e, 0x86),
            accent: Color32::from_rgb(0x4f, 0x8a, 0x83),
            panel_fill: Color32::from_rgba_unmultiplied(252, 252, 250, 214),
            panel_stroke: Color32::from_black_alpha(14),
            window_fill: Color32::from_rgb(0xfb, 0xfb, 0xf9),
            highlight: Color32::from_black_alpha(16),
        }
    }
}

/// The palette matching the visuals currently installed on `ui`.
pub fn current(ui: &egui::Ui) -> Palette {
    palette(ui.visuals().dark_mode)
}

pub fn panel_shadow(dark: bool) -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 70 } else { 26 }),
    }
}

/// The frame used for the dock, flyouts, dialogs and toasts.
pub fn panel_frame(dark: bool) -> egui::Frame {
    let p = palette(dark);
    egui::Frame::NONE
        .fill(p.panel_fill)
        .stroke(Stroke::new(1.0, p.panel_stroke))
        .shadow(panel_shadow(dark))
        .corner_radius(PANEL_RADIUS)
        .inner_margin(Margin::same(8))
}

/// Installs the light or dark theme on every egui style.
pub fn install(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let p = palette(dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(19.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(6.0, 6.0);
        s.button_padding = egui::vec2(14.0, 7.0);
        s.interact_size.y = 30.0;
        s.slider_rail_height = 4.0;
        s.window_margin = Margin::same(18);
        s.menu_margin = Margin::same(6);

        let v = &mut style.visuals;
        *v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        let tint = |alpha: u8| {
            if dark {
                Color32::from_white_alpha(alpha)
            } else {
                Color32::from_black_alpha(alpha)
            }
        };
        v.override_text_color = Some(p.ink);
        v.panel_fill = Color32::TRANSPARENT;
        v.window_fill = p.window_fill;
        v.window_stroke = Stroke::new(1.0, p.panel_stroke);
        v.window_shadow = Shadow {
            offset: [0, 14],
            blur: 40,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
        };
        v.window_corner_radius = CornerRadius::same(PANEL_RADIUS);
        v.menu_corner_radius = CornerRadius::same(BUTTON_RADIUS);
        v.popup_shadow = panel_shadow(dark);
        v.extreme_bg_color = tint(14);
        v.faint_bg_color = tint(8);
        v.selection.bg_fill = p.accent.gamma_multiply(0.32);
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.hyperlink_color = p.accent;
        v.slider_trailing_fill = true;

        let radius = CornerRadius::same(BUTTON_RADIUS);
        let w = &mut v.widgets;
        for state in [
            &mut w.noninteractive,
            &mut w.inactive,
            &mut w.hovered,
            &mut w.active,
            &mut w.open,
        ] {
            state.corner_radius = radius;
            state.expansion = 0.0;
            state.bg_stroke = Stroke::NONE;
            state.fg_stroke = Stroke::new(1.3, p.ink);
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, tint(18));
        w.inactive.weak_bg_fill = tint(12);
        w.inactive.bg_fill = tint(24);
        w.hovered.weak_bg_fill = tint(22);
        w.hovered.bg_fill = tint(34);
        w.active.weak_bg_fill = tint(34);
        w.active.bg_fill = p.accent;
        w.open.weak_bg_fill = tint(22);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b())
    }

    #[test]
    fn ink_contrasts_with_the_panels_in_both_variants() {
        for dark in [false, true] {
            let p = palette(dark);
            let (ink, window) = (luminance(p.ink), luminance(p.window_fill));
            assert!(
                (ink - window).abs() > 120.0,
                "dark {dark}: {ink} vs {window}"
            );
            assert_eq!(ink > window, dark, "dark themes use light ink");
            // Muted text sits between the ink and the background.
            let muted = luminance(p.muted);
            assert!(muted > ink.min(window) && muted < ink.max(window));
            // Panels are translucent so the scene shows through.
            assert!(p.panel_fill.a() < 255 && p.panel_fill.a() > 128);
        }
    }

    #[test]
    fn panel_frames_are_rounded_and_shadowed() {
        for dark in [false, true] {
            let frame = panel_frame(dark);
            assert_eq!(frame.fill, palette(dark).panel_fill);
            assert_eq!(frame.corner_radius, CornerRadius::same(PANEL_RADIUS));
            assert!(frame.shadow.blur > 0);
        }
        assert!(panel_shadow(true).color.a() > panel_shadow(false).color.a());
    }

    #[test]
    fn install_switches_visuals_and_current_follows() {
        let ctx = egui::Context::default();
        for dark in [true, false, true] {
            install(&ctx, dark);
            let mut seen = None;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                seen = Some((ui.visuals().dark_mode, current(ui).ink));
            });
            output.textures_delta.clear();
            assert_eq!(seen, Some((dark, palette(dark).ink)));
            ctx.all_styles_mut(|style| {
                assert_eq!(style.visuals.override_text_color, Some(palette(dark).ink));
                assert_eq!(style.visuals.panel_fill, Color32::TRANSPARENT);
                assert_eq!(
                    style.text_styles[&TextStyle::Body],
                    FontId::new(14.5, FontFamily::Proportional)
                );
            });
        }
    }
}
