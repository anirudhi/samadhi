//! A small, curated set of typefaces (all SIL OFL), bundled so the app looks
//! the same everywhere. Variable fonts are set at a light writing weight and
//! get true bold for markdown; static fonts ship a separate bold file.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};
use eframe::epaint::text::{FontTweak, HintingTarget, SmoothHinting, VariationCoords};

/// Family names used for the page text.
const WRITER: &str = "writer";
const WRITER_BOLD: &str = "writer-bold";
const MONO: &str = "iA Writer Mono";
const MONO_BOLD: &str = "iA Writer Mono Bold";
const UI_FONT: &str = "Inter";
/// The interface's own copy of Inter, rendered for maximum crispness.
const UI_FONT_CRISP: &str = "Inter (interface)";

/// How a typeface gets its weights.
#[derive(Debug, Clone, Copy)]
pub enum Weights {
    /// A variable font: body and bold weights on its `wght` axis.
    Variable { body: f32, bold: f32 },
    /// Static fonts: a separate bold file.
    Static { bold: &'static [u8] },
}

/// A writing typeface: stored name, short label for the picker, files.
#[derive(Debug, Clone, Copy)]
pub struct Typeface {
    pub name: &'static str,
    pub label: &'static str,
    regular: &'static [u8],
    pub weights: Weights,
}

pub const TYPEFACES: [Typeface; 5] = [
    Typeface {
        name: "Literata",
        label: "Literata",
        regular: include_bytes!("../assets/fonts/Literata.ttf"),
        weights: Weights::Variable {
            body: 300.0,
            bold: 600.0,
        },
    },
    Typeface {
        name: "Newsreader",
        label: "Newsreader",
        regular: include_bytes!("../assets/fonts/Newsreader.ttf"),
        weights: Weights::Variable {
            body: 300.0,
            bold: 600.0,
        },
    },
    Typeface {
        name: UI_FONT,
        label: "Sans",
        regular: include_bytes!("../assets/fonts/Inter.ttf"),
        weights: Weights::Variable {
            body: 300.0,
            bold: 600.0,
        },
    },
    Typeface {
        name: "iA Writer Quattro",
        label: "Quattro",
        regular: include_bytes!("../assets/fonts/iAWriterQuattroS-Regular.ttf"),
        weights: Weights::Static {
            bold: include_bytes!("../assets/fonts/iAWriterQuattroS-Bold.ttf"),
        },
    },
    Typeface {
        name: MONO,
        label: "Mono",
        regular: include_bytes!("../assets/fonts/iAWriterMonoS-Regular.ttf"),
        weights: Weights::Static {
            bold: include_bytes!("../assets/fonts/iAWriterMonoS-Bold.ttf"),
        },
    },
];

/// The writing font used until the user picks another.
pub const DEFAULT_WRITING_FONT: &str = TYPEFACES[0].name;

/// Ready-to-use fonts and weight settings for page text in one typeface.
#[derive(Debug, Clone)]
pub struct WritingFonts {
    pub body: FontId,
    pub body_coords: VariationCoords,
    pub bold: FontId,
    pub bold_coords: VariationCoords,
    pub mono: FontId,
}

impl Typeface {
    pub fn writing_fonts(&self, size: f32) -> WritingFonts {
        let writer = FontId::new(size, FontFamily::Name(WRITER.into()));
        let mono = FontId::new(size * 0.92, FontFamily::Name(MONO.into()));
        match self.weights {
            Weights::Variable { body, bold } => WritingFonts {
                body: writer.clone(),
                body_coords: VariationCoords::new([(b"wght", body)]),
                bold: writer,
                bold_coords: VariationCoords::new([(b"wght", bold)]),
                mono,
            },
            Weights::Static { .. } => WritingFonts {
                body: writer,
                body_coords: VariationCoords::default(),
                bold: FontId::new(size, FontFamily::Name(WRITER_BOLD.into())),
                bold_coords: VariationCoords::default(),
                mono,
            },
        }
    }
}

/// A font id that draws in `typeface` itself (used to preview each choice).
pub fn preview_font(typeface: &Typeface, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(typeface.name.into()))
}

/// Resolves a stored name to a known typeface, falling back to the default.
pub fn typeface(name: Option<&str>) -> &'static Typeface {
    name.and_then(|n| TYPEFACES.iter().find(|t| t.name == n))
        .unwrap_or(&TYPEFACES[0])
}

/// Installs the fonts (Inter for the interface) with `writing` as the page
/// font. egui's own fonts stay as fallbacks for symbols the others lack.
pub fn apply(ctx: &egui::Context, writing: &Typeface) {
    let mut defs = FontDefinitions::default();
    let fallbacks = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut family = |name: &str, data: &str| {
        let mut chain = vec![data.to_owned()];
        chain.extend(fallbacks.iter().cloned());
        defs.families.insert(FontFamily::Name(name.into()), chain);
    };
    let mut data: Vec<(String, &'static [u8])> = Vec::new();
    for t in &TYPEFACES {
        data.push((t.name.to_owned(), t.regular));
        family(t.name, t.name);
        if let Weights::Static { bold } = t.weights {
            let bold_name = format!("{} Bold", t.name);
            family(&bold_name, &bold_name);
            data.push((bold_name, bold));
        }
    }
    family(WRITER, writing.name);
    let writing_bold = match writing.weights {
        Weights::Static { .. } => format!("{} Bold", writing.name),
        Weights::Variable { .. } => writing.name.to_owned(),
    };
    family(WRITER_BOLD, &writing_bold);
    family(MONO_BOLD, MONO_BOLD);
    // Small interface text is hinted fully (snapped to the pixel grid in
    // both directions) and drawn without sub-pixel offsets, which egui
    // notes can look blurry. Body text keeps the smoother defaults.
    let crisp = FontTweak {
        hinting: Some(true),
        hinting_target: HintingTarget::Smooth(SmoothHinting {
            light: false,
            symmetric_rendering: false,
            preserve_linear_metrics: false,
        }),
        subpixel_binning: Some(false),
        ..FontTweak::default()
    };
    data.push((UI_FONT_CRISP.to_owned(), TYPEFACES[2].regular));
    let proportional = {
        let mut chain = vec![UI_FONT_CRISP.to_owned()];
        chain.extend(fallbacks.iter().cloned());
        chain
    };
    for (name, bytes) in data {
        let mut font = FontData::from_static(bytes);
        if name == UI_FONT_CRISP {
            font = font.tweak(crisp.clone());
        }
        defs.font_data.insert(name, Arc::new(font));
    }
    defs.families.insert(FontFamily::Proportional, proportional);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_are_font_files() {
        let files = TYPEFACES.iter().flat_map(|t| {
            let bold = match t.weights {
                Weights::Static { bold } => Some(bold),
                Weights::Variable { .. } => None,
            };
            std::iter::once(t.regular).chain(bold)
        });
        for bytes in files {
            assert!(bytes.len() > 10_000);
            assert!(bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"OTTO"));
        }
    }

    #[test]
    fn variable_fonts_have_a_weight_axis_wide_enough() {
        for t in TYPEFACES {
            if let Weights::Variable { body, bold } = t.weights {
                let axes = FontData::from_static(t.regular).variation_axes();
                let wght = axes
                    .iter()
                    .find(|a| a.name.as_deref() == Some("Weight"))
                    .unwrap_or_else(|| panic!("{} has no weight axis", t.name));
                assert!(
                    wght.range.min <= body && bold <= wght.range.max,
                    "{}: {wght:?}",
                    t.name
                );
            }
        }
    }

    #[test]
    fn unknown_names_fall_back_to_the_default() {
        assert_eq!(typeface(None).name, DEFAULT_WRITING_FONT);
        assert_eq!(typeface(Some("Comic Sans")).name, DEFAULT_WRITING_FONT);
        assert_eq!(typeface(Some("iA Writer Mono")).label, "Mono");
        let ctx = egui::Context::default();
        for t in &TYPEFACES {
            apply(&ctx, t);
        }
    }
}
