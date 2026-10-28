//! Spell checking arbitrary text.

use std::sync::LazyLock;

use proptest::prelude::*;
use samadhi::spell::{SpellChecker, words};

use crate::{config, tricky_text};

/// Parsing a dictionary takes a moment, so every test shares one.
static CHECKER: LazyLock<SpellChecker> =
    LazyLock::new(|| SpellChecker::load(None).expect("a dictionary loads"));

fn check_ranges(text: &str, ranges: &[std::ops::Range<usize>]) -> Result<(), TestCaseError> {
    let mut last_end = 0;
    for r in ranges {
        prop_assert!(r.start < r.end, "empty range in {:?}", text);
        prop_assert!(r.end <= text.len());
        prop_assert!(r.start >= last_end, "overlapping ranges in {:?}", text);
        prop_assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
        let word = &text[r.clone()];
        prop_assert!(!word.chars().any(char::is_whitespace), "{:?}", word);
        prop_assert!(
            word.chars().next().is_some_and(char::is_alphabetic),
            "{:?}",
            word
        );
        last_end = r.end;
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn words_are_valid_ranges(text in tricky_text(100)) {
        let ranges: Vec<_> = words(&text).collect();
        check_ranges(&text, &ranges)?;
    }

    /// Misspellings are a subset of the words, and correct words (as the
    /// checker itself judges them) are never flagged.
    #[test]
    fn misspellings_are_valid_words(text in tricky_text(80)) {
        let flagged = CHECKER.misspellings(&text);
        check_ranges(&text, &flagged)?;
        let all: Vec<_> = words(&text).collect();
        for r in &flagged {
            prop_assert!(all.contains(r));
            prop_assert!(!CHECKER.is_correct(&text[r.clone()]));
        }
    }

    #[test]
    fn is_correct_never_panics(word in "\\PC{0,30}") {
        let _ = CHECKER.is_correct(&word);
    }
}

proptest! {
    // Suggestions search the dictionary and are the slowest call here.
    #![proptest_config(config(24))]

    #[test]
    fn suggestions_are_few_and_distinct(word in "[A-Za-z\u{e9}\u{2019}']{1,10}") {
        let suggestions = CHECKER.suggest(&word);
        prop_assert!(suggestions.len() <= 6);
        let unique: std::collections::HashSet<_> = suggestions.iter().collect();
        prop_assert_eq!(unique.len(), suggestions.len());
        prop_assert!(suggestions.iter().all(|s| !s.is_empty()));
    }

    #[test]
    fn suggest_never_panics_on_odd_words(word in "\\PC{1,8}") {
        let _ = CHECKER.suggest(&word);
    }
}
