//! Documents: decoding, encoding, line endings and safe writes.

use std::fs;
use std::path::Path;

use proptest::prelude::*;
use samadhi::document::{Document, SaveOutcome, conflict_copy_path, count_words, write_atomically};

use crate::{config, tricky_char};

/// Text as the editor holds it: `\n` line endings only.
fn editor_text(max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(tricky_char(), 0..max)
        .prop_map(|c| c.into_iter().filter(|&c| c != '\r').collect())
}

fn ending() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["\n", "\r\n", "\r"])
}

/// Writes `bytes` to a file, opens it, saves it again and returns the
/// opened document and the bytes written back.
fn open_and_resave(dir: &Path, bytes: &[u8]) -> anyhow::Result<(Document, Vec<u8>)> {
    let path = dir.join("doc.md");
    fs::write(&path, bytes)?;
    let mut doc = Document::open(&path)?;
    doc.mark_dirty();
    doc.save()?;
    Ok((doc, fs::read(&path)?))
}

fn utf16(text: &str, little_endian: bool, bom: bool) -> Vec<u8> {
    let to_bytes = |u: u16| {
        if little_endian {
            u.to_le_bytes()
        } else {
            u.to_be_bytes()
        }
    };
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(&to_bytes(0xFEFF));
    }
    for unit in text.encode_utf16() {
        out.extend_from_slice(&to_bytes(unit));
    }
    out
}

/// A reference word count written without `split_whitespace`.
fn reference_count(text: &str) -> usize {
    let mut count = 0;
    let (mut in_word, mut has_alnum) = (false, false);
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_whitespace() {
            if in_word && has_alnum {
                count += 1;
            }
            in_word = false;
            has_alnum = false;
        } else {
            in_word = true;
            has_alnum |= c.is_alphanumeric();
        }
    }
    count
}

proptest! {
    #![proptest_config(config(128))]

    /// UTF-8 files, with or without a byte-order mark and with any one
    /// line-ending style, open to `\n` text and save back byte for byte.
    #[test]
    fn utf8_files_round_trip_byte_for_byte(
        text in editor_text(200),
        bom in any::<bool>(),
        nl in ending(),
    ) {
        // A NUL makes a BOM-less file look binary, and a leading U+FEFF
        // reads as a byte-order mark; both are covered separately.
        let text: String = text.chars().filter(|&c| c != '\0').collect();
        prop_assume!(bom || !text.starts_with('\u{FEFF}'));
        let mut bytes = if bom { b"\xEF\xBB\xBF".to_vec() } else { Vec::new() };
        bytes.extend_from_slice(text.replace('\n', nl).as_bytes());
        let dir = tempfile::tempdir().unwrap();
        let (doc, written) = open_and_resave(dir.path(), &bytes).unwrap();
        prop_assert_eq!(&doc.text, &text);
        prop_assert_eq!(written, bytes);
    }

    /// UTF-16 with a byte-order mark, either byte order, any characters.
    #[test]
    fn utf16_files_with_a_bom_round_trip(
        text in editor_text(200),
        little_endian in any::<bool>(),
        nl in ending(),
    ) {
        let bytes = utf16(&text.replace('\n', nl), little_endian, true);
        let dir = tempfile::tempdir().unwrap();
        let (doc, written) = open_and_resave(dir.path(), &bytes).unwrap();
        prop_assert_eq!(&doc.text, &text);
        prop_assert_eq!(written, bytes);
    }

    /// BOM-less UTF-16 is recognised for mostly-ASCII text.
    #[test]
    fn bomless_utf16_ascii_is_detected(
        text in "[ -~\n]{2,120}",
        little_endian in any::<bool>(),
    ) {
        let bytes = utf16(&text, little_endian, false);
        let dir = tempfile::tempdir().unwrap();
        let (doc, written) = open_and_resave(dir.path(), &bytes).unwrap();
        prop_assert_eq!(&doc.text, &text);
        prop_assert_eq!(written, bytes);
    }

    /// Legacy Windows-1252 bytes (invalid as UTF-8) decode and re-encode to
    /// exactly the same bytes.
    #[test]
    fn windows_1252_files_round_trip(
        lines in prop::collection::vec(
            prop::collection::vec(1u8..=255, 0..40)
                .prop_map(|l| l.into_iter().filter(|&b| b != b'\r' && b != b'\n').collect::<Vec<u8>>()),
            1..6,
        ),
        nl in ending(),
    ) {
        // `é` then ASCII is never valid UTF-8, and starts no byte-order mark.
        let mut bytes = b"\xE9x".to_vec();
        bytes.extend_from_slice(&lines.join(nl.as_bytes()));
        let dir = tempfile::tempdir().unwrap();
        let (doc, written) = open_and_resave(dir.path(), &bytes).unwrap();
        prop_assert!(doc.text.starts_with("éx"));
        prop_assert_eq!(written, bytes);
    }

    /// Text a Windows-1252 file cannot hold is saved as UTF-8 instead,
    /// and nothing is lost.
    #[test]
    fn unmappable_text_in_a_legacy_file_falls_back_to_utf8(extra in editor_text(40)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.txt");
        fs::write(&path, b"caf\xE9").unwrap();
        let mut doc = Document::open(&path).unwrap();
        let extra: String = extra.chars().filter(|&c| c != '\0').collect();
        doc.text.push('中');
        doc.text.push_str(&extra);
        prop_assert_eq!(doc.save().unwrap(), SaveOutcome::SavedAsUtf8);
        let reopened = Document::open(&path).unwrap();
        prop_assert_eq!(reopened.text, doc.text);
    }

    /// Mixed line endings become `\n` inside, whatever the mix.
    #[test]
    fn mixed_line_endings_normalise(
        parts in prop::collection::vec(("[a-z ]{1,8}", ending()), 0..20),
    ) {
        let raw: String = parts.iter().flat_map(|(s, nl)| [s.as_str(), nl]).collect();
        let expected: String = parts.iter().flat_map(|(s, _)| [s.as_str(), "\n"]).collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed.txt");
        fs::write(&path, &raw).unwrap();
        let mut doc = Document::open(&path).unwrap();
        prop_assert_eq!(&doc.text, &expected);
        doc.mark_dirty();
        doc.save().unwrap();
        prop_assert_eq!(Document::open(&path).unwrap().text, expected);
    }

    /// Opening arbitrary bytes never panics. Whatever opens, saves, and
    /// opens again to the same text (saving is stable).
    #[test]
    fn arbitrary_bytes_never_panic_and_resave_stably(
        prefix in prop::sample::select(vec![
            &b""[..], b"\xEF\xBB\xBF", b"\xFF\xFE", b"\xFE\xFF",
        ]),
        body in prop::collection::vec(any::<u8>(), 0..300),
    ) {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&body);
        let dir = tempfile::tempdir().unwrap();
        if let Ok((doc, written)) = open_and_resave(dir.path(), &bytes) {
            let path = dir.path().join("doc.md");
            let mut again = Document::open(&path).unwrap();
            prop_assert_eq!(&again.text, &doc.text);
            again.mark_dirty();
            again.save().unwrap();
            prop_assert_eq!(fs::read(&path).unwrap(), written);
        }
    }

    /// Mostly-zero byte patterns (the shape of UTF-16) never panic either.
    #[test]
    fn utf16_shaped_bytes_never_panic(
        units in prop::collection::vec((any::<u8>(), prop::sample::select(vec![0u8, 0, 0, 0xD8, 0xDC, 1])), 0..200),
        swap in any::<bool>(),
    ) {
        let bytes: Vec<u8> = units
            .into_iter()
            .flat_map(|(a, b)| if swap { [b, a] } else { [a, b] })
            .collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u16.txt");
        fs::write(&path, &bytes).unwrap();
        let _ = Document::open(&path);
    }

    #[test]
    fn count_words_matches_a_reference(text in crate::tricky_text(80)) {
        prop_assert_eq!(count_words(&text), reference_count(&text));
    }

    /// Counting is additive across a whitespace boundary.
    #[test]
    fn count_words_is_additive(a in crate::tricky_text(30), b in crate::tricky_text(30)) {
        prop_assert_eq!(
            count_words(&format!("{a} {b}")),
            count_words(&a) + count_words(&b)
        );
    }

    /// Conflict copies sit beside the original, keep its extension and
    /// never reuse a name, however many there are.
    #[test]
    fn conflict_copies_are_fresh_siblings(
        stem in "[A-Za-z0-9 ._\u{e9}\u{4e2d}-]{1,16}",
        ext in prop::option::of("[A-Za-z0-9]{1,5}"),
        copies in 1usize..5,
    ) {
        let name = ext.as_ref().map_or_else(|| stem.clone(), |e| format!("{stem}.{e}"));
        prop_assume!(name != "." && name != "..");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(&name);
        fs::write(&path, b"original").unwrap();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..copies {
            let copy = conflict_copy_path(&path);
            prop_assert_eq!(copy.parent(), path.parent());
            prop_assert!(!copy.exists());
            prop_assert_ne!(&copy, &path);
            prop_assert_eq!(copy.extension(), path.extension());
            prop_assert!(seen.insert(copy.clone()));
            fs::write(&copy, b"copy").unwrap();
        }
        prop_assert_eq!(fs::read(&path).unwrap(), b"original");
    }

    /// Atomic writes leave exactly the new bytes and no temporary files,
    /// and keep the permissions of the file they replace.
    #[test]
    fn write_atomically_replaces_contents(
        first in prop::collection::vec(any::<u8>(), 0..2000),
        second in prop::collection::vec(any::<u8>(), 0..2000),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.bin");
        write_atomically(&path, &first).unwrap();
        prop_assert_eq!(fs::read(&path).unwrap(), first);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        write_atomically(&path, &second).unwrap();
        prop_assert_eq!(fs::read(&path).unwrap(), second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            prop_assert_eq!(mode, 0o640);
        }
        prop_assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

/// Text that begins with U+FEFF: in a UTF-8 file that character at the
/// start *is* a byte-order mark, so on reopening it is read as one (and
/// not shown). The file itself stays exactly the same through later saves.
#[test]
fn a_leading_feff_is_read_as_a_byte_order_mark_and_the_file_is_stable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("feff.md");
    let mut doc = Document::default();
    doc.text = "\u{FEFF}x".to_owned();
    doc.save_as(&path).unwrap();
    let first = fs::read(&path).unwrap();
    let mut reopened = Document::open(&path).unwrap();
    assert_eq!(reopened.text, "x");
    reopened.mark_dirty();
    reopened.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), first);
}

/// A document containing a NUL character (e.g. pasted from a binary-ish
/// source) saves as UTF-8 without complaint, but can then never be opened
/// again: the loader rejects any BOM-less file with a zero byte.
#[test]
fn text_with_a_nul_can_be_reopened_after_saving() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nul.md");
    let mut doc = Document::default();
    doc.text = "a\0b".to_owned();
    doc.save_as(&path).unwrap();
    assert_eq!(Document::open(&path).unwrap().text, "a\0b");
}
