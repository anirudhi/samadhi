//! Settings: loading hand-edited files and keeping values in range.

use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use samadhi::settings::{MAX_FONT_SIZE, MIN_FONT_SIZE, PageRect, RECENT_FILES, Settings};
use samadhi::timer::{MAX_MINUTES, MIN_MINUTES};

use crate::config;

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => -200.0f32..200.0,
        1 => prop::sample::select(vec![
            0.0, -0.0, 1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::MIN,
            f32::MIN_POSITIVE, 1e-40,
        ]),
        1 => any::<f32>(),
    ]
}

fn path() -> impl Strategy<Value = PathBuf> {
    prop::sample::select(vec![
        "/a.md",
        "/b.md",
        "/c d/e.txt",
        "rel.md",
        "/ü/中.md",
        "/a.md",
    ])
    .prop_map(PathBuf::from)
}

fn page() -> impl Strategy<Value = Option<PageRect>> {
    prop::option::of((any_f32(), any_f32(), any_f32(), any_f32()).prop_map(
        |(x, y, width, height)| PageRect {
            x,
            y,
            width,
            height,
        },
    ))
}

prop_compose! {
    fn settings(extension: BoxedStrategy<String>)(
        page in page(),
        font_family in prop::option::of("\\PC{0,12}"),
        font_size in any_f32(),
        page_opacity in any_f32(),
        theme in prop::option::of("\\PC{0,12}"),
        motion in any::<bool>(),
        music in prop::option::of("[a-z ]{0,8}"),
        music_muted in any::<bool>(),
        music_volume in any_f32(),
        click in prop::option::of("[a-z ]{0,8}"),
        click_muted in any::<bool>(),
        click_volume in any_f32(),
        markdown in any::<bool>(),
        spell_check in any::<bool>(),
        last_file in prop::option::of(path()),
        recent_files in prop::collection::vec(path(), 0..20),
        documents_dir in prop::option::of(path()),
        default_extension in extension,
        timer_minutes in any::<u32>(),
    ) -> Settings {
        Settings {
            page,
            font_family,
            font_size,
            page_opacity,
            theme,
            motion,
            music,
            music_muted,
            music_volume,
            click,
            click_muted,
            click_volume,
            markdown,
            spell_check,
            last_file,
            recent_files,
            documents_dir,
            default_extension,
            timer_minutes,
        }
    }
}

fn check_in_range(s: &Settings) -> Result<(), TestCaseError> {
    prop_assert!((MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&s.font_size));
    for v in [s.page_opacity, s.music_volume, s.click_volume] {
        prop_assert!((0.0..=1.0).contains(&v), "{}", v);
    }
    prop_assert!((MIN_MINUTES..=MAX_MINUTES).contains(&s.timer_minutes));
    prop_assert!(s.recent_files.len() <= RECENT_FILES);
    let unique: std::collections::HashSet<_> = s.recent_files.iter().collect();
    prop_assert_eq!(unique.len(), s.recent_files.len());
    prop_assert!(!s.default_extension.is_empty());
    prop_assert!(!s.default_extension.starts_with('.'));
    if let Some(p) = s.page {
        prop_assert!([p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite()));
        prop_assert!(p.width > 0.0 && p.height > 0.0);
    }
    Ok(())
}

/// Fragments of a settings file, valid and not.
fn toml_line() -> impl Strategy<Value = String> {
    let key = prop::sample::select(vec![
        "font_size",
        "page_opacity",
        "music_volume",
        "click_volume",
        "timer_minutes",
        "default_extension",
        "recent_files",
        "motion",
        "theme",
        "last_file",
        "unknown",
    ]);
    let value = prop::sample::select(vec![
        "1",
        "-1",
        "0",
        "1e40",
        "-1e40",
        "nan",
        "inf",
        "-inf",
        "16.5",
        "4294967295",
        "4294967296",
        "-5",
        "true",
        "\"md\"",
        "\" .MD \"",
        "\"\"",
        "[]",
        "[\"/a\", \"/a\", \"/b\"]",
        "{ x = 1 }",
        "\"\\u0000\"",
        "'''multi\nline'''",
        "[1, 2",
        "\"unterminated",
    ]);
    prop_oneof![
        4 => (key, value).prop_map(|(k, v)| format!("{k} = {v}")),
        1 => prop::sample::select(vec![
            "[page]", "x = 1", "width = -3", "height = nan", "y = 2", "[page", "= 3", "#",
            "\u{FEFF}", "font_size = 12\nfont_size = 13",
        ])
        .prop_map(str::to_owned),
        1 => "\\PC{0,20}",
    ]
}

proptest! {
    #![proptest_config(config(256))]

    /// Any file at all loads without panicking, and what loads is sane.
    #[test]
    fn any_settings_file_loads_sanely(lines in prop::collection::vec(toml_line(), 0..12)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, lines.join("\n")).unwrap();
        check_in_range(&Settings::load_or_default(Some(&path)))?;
    }

    /// Raw bytes, including invalid UTF-8, fall back to defaults.
    #[test]
    fn binary_settings_files_load(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, bytes).unwrap();
        check_in_range(&Settings::load_or_default(Some(&path)))?;
    }

    #[test]
    fn sanitized_values_are_in_range(s in settings("\\PC{0,8}".boxed())) {
        check_in_range(&s.sanitized())?;
    }

    /// What is saved loads back exactly.
    #[test]
    fn save_then_load_round_trips(s in settings("[A-Za-z0-9]{0,6}".boxed())) {
        let s = s.sanitized();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.toml");
        s.save(&path).unwrap();
        let loaded = Settings::load_or_default(Some(&path));
        prop_assert_eq!(loaded, s);
    }

    /// Sanitising twice changes nothing more. (Extensions mixing spaces
    /// and dots are not idempotent: see the ignored test below.)
    #[test]
    fn sanitizing_is_idempotent(s in settings("[a-zA-Z.]{0,4}".boxed())) {
        let once = s.sanitized();
        prop_assert_eq!(once.clone().sanitized(), once);
    }

    #[test]
    fn recent_files_stay_unique_and_bounded(
        adds in prop::collection::vec(path(), 0..40),
    ) {
        let mut s = Settings::default();
        for p in &adds {
            s.add_recent(p);
            prop_assert_eq!(s.recent_files.first(), Some(p));
            check_in_range(&s)?;
        }
    }
}

/// A hand-edited extension like `". md"` keeps a leading space: dots are
/// stripped after trimming, so the space they hid survives, and new
/// documents would be named `Untitled. md`.
#[test]
fn extension_with_a_dot_and_a_space_is_cleaned() {
    let s = Settings {
        default_extension: ". md".to_owned(),
        ..Settings::default()
    }
    .sanitized();
    assert_eq!(s.default_extension, "md");
}
