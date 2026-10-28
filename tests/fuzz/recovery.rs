//! Crash recovery of arbitrary drafts.

use std::path::PathBuf;

use proptest::prelude::*;
use samadhi::recovery::{Draft, Recovery};

use crate::{config, tricky_text};

proptest! {
    #![proptest_config(config(128))]

    #[test]
    fn drafts_round_trip(
        text in tricky_text(200),
        path in prop::option::of("[^\\s\u{0}](\\PC{0,30}[^\\s\u{0}])?"),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let recovery = Recovery::new(Some(dir.path().join("recovery")));
        let path = path.map(PathBuf::from);
        recovery.store(&text, path.as_deref()).unwrap();
        let expected = (!text.trim().is_empty()).then(|| Draft { text: text.clone(), path });
        prop_assert_eq!(recovery.load(), expected);
        recovery.clear();
        prop_assert_eq!(recovery.load(), None);
    }

    /// A newer draft replaces the older one entirely.
    #[test]
    fn later_drafts_replace_earlier_ones(
        first in "\\PC{1,40}",
        second in "[a-z]\\PC{0,40}",
        path in prop::option::of("/[a-z]{1,8}\\.md"),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let recovery = Recovery::new(Some(dir.path().to_path_buf()));
        recovery.store(&first, Some(std::path::Path::new("/old.md"))).unwrap();
        let path = path.map(PathBuf::from);
        recovery.store(&second, path.as_deref()).unwrap();
        prop_assert_eq!(recovery.load(), Some(Draft { text: second, path }));
    }
}

/// A file whose name ends in a space (legal on Linux and macOS) is
/// recovered as a different file: the stored origin is trimmed.
#[test]
fn origin_paths_with_trailing_spaces_survive() {
    let dir = tempfile::tempdir().unwrap();
    let recovery = Recovery::new(Some(dir.path().to_path_buf()));
    let path = PathBuf::from("/notes/notes.md ");
    recovery.store("text", Some(&path)).unwrap();
    assert_eq!(recovery.load().unwrap().path, Some(path));
}
