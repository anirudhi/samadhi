//! Cyclable catalogues under arbitrary navigation.

use std::path::PathBuf;

use proptest::prelude::*;
use samadhi::catalog::{Catalog, display_name};

use crate::config;

#[derive(Debug, Clone)]
enum Op {
    Forward,
    Back,
    SelectIndex(usize),
    Select(String),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Forward),
        Just(Op::Back),
        (0usize..30).prop_map(Op::SelectIndex),
        "[a-c]{0,2}|Built-in [0-9]".prop_map(Op::Select),
    ]
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn the_cursor_always_points_at_an_entry(
        builtins in 1usize..6,
        files in prop::collection::vec("[a-c_ ]{0,4}\\.(mp3|png)", 0..8),
        ops in prop::collection::vec(op(), 0..40),
    ) {
        let builtin: Vec<(String, usize)> =
            (0..builtins).map(|i| (format!("Built-in {i}"), i)).collect();
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        let mut catalog = Catalog::new(builtin, files.clone());
        prop_assert_eq!(catalog.len(), builtins + files.len());
        prop_assert!(!catalog.is_empty());
        for op in ops {
            let before = catalog.current_index();
            match op {
                Op::Forward => {
                    catalog.step_forward();
                    catalog.step_back();
                    prop_assert_eq!(catalog.current_index(), before);
                    catalog.step_forward();
                }
                Op::Back => {
                    catalog.step_back();
                }
                Op::SelectIndex(i) => {
                    catalog.select_index(i);
                    let expected = if i < catalog.len() { i } else { before };
                    prop_assert_eq!(catalog.current_index(), expected);
                }
                Op::Select(name) => {
                    if catalog.select(&name) {
                        prop_assert_eq!(&catalog.current().name, &name);
                    } else {
                        prop_assert_eq!(catalog.current_index(), before);
                    }
                }
            }
            prop_assert!(catalog.current_index() < catalog.len());
            prop_assert_eq!(&catalog.entries()[catalog.current_index()], catalog.current());
        }
    }

    /// Display names drop the extension and turn underscores into single
    /// spaces, with none at either end.
    #[test]
    fn display_names_are_tidy(stem in "[A-Za-z0-9_ \u{e9}\u{4e2d}]{0,20}", ext in "[a-z0-9]{1,4}") {
        // An empty stem makes a hidden file (`.mp3`), whose stem is `.mp3`.
        prop_assume!(!stem.is_empty());
        let name = display_name(&PathBuf::from(format!("/music/{stem}.{ext}")));
        let dotted = format!(".{ext}");
        prop_assert!(!name.ends_with(&dotted) || stem.ends_with(&dotted));
        let words: Vec<&str> = stem.split(['_', ' ']).filter(|w| !w.is_empty()).collect();
        if !words.is_empty() {
            prop_assert!(!name.contains('_'));
            prop_assert!(!name.starts_with(' ') && !name.ends_with(' '));
            prop_assert!(!name.contains("  "));
            prop_assert_eq!(name, words.join(" "));
        }
    }

    #[test]
    fn display_name_never_panics(path in "\\PC{0,40}") {
        let _ = display_name(&PathBuf::from(path));
    }
}
