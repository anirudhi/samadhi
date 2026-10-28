//! Property-based stress tests for Samādhi's library modules.
//!
//! Everything lives in one test binary (one module per area) so the heavy
//! GUI dependencies are linked once rather than once per file.

mod catalog;
mod document;
mod frost;
#[cfg(unix)]
mod instance;
mod markdown;
mod page;
mod recovery;
mod scene;
mod settings;
mod spell;
mod timer;

use proptest::prelude::*;
use proptest::test_runner::Config;

/// A proptest configuration with `cases` cases and no regression files
/// written into the source tree; known failures are kept as `#[ignore]`d
/// tests instead.
pub fn config(cases: u32) -> Config {
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

/// Characters that tend to break text handling: markdown syntax, line
/// breaks, combining marks, emoji, right-to-left scripts, byte-order marks
/// and control characters, plus anything at all now and then.
pub fn tricky_char() -> impl Strategy<Value = char> {
    prop_oneof![
        4 => prop::sample::select(vec![
            '*', '_', '`', '#', '>', '-', '+', '[', ']', '(', ')', '~', '!', '.', '\\', '\'',
            ' ', ' ', '\t', '\n', '\n', '\r', '1', '9', 'a', 'b', 'z', 'Q', '@', '/', ':',
        ]),
        2 => prop::sample::select(vec![
            'é', '\u{301}', '\u{308}', '👍', '\u{1F3FD}', '\u{200D}', '👨', '\u{FE0F}', 'ש', 'ל',
            'م', '\u{202E}', '\u{200F}', '中', '\u{FEFF}', 'ß', 'İ', '\u{2019}', '\u{A0}',
            '\u{2028}', '\u{85}', '\u{0}', '\u{7F}', '\u{FFFD}',
        ]),
        1 => any::<char>(),
    ]
}

/// Whole fragments of markdown, so block and inline syntax appear in
/// combinations random characters rarely hit.
pub fn tricky_token() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => tricky_char().prop_map(String::from),
        2 => prop::sample::select(vec![
            "# ", "## ", "###### ", "####### ", "> ", ">", "- ", "* ", "+ ", "1. ", "12) ",
            "1234. ", "```", "```\n", "---", "***", "___", "**", "__", "~~", "*", "_", "`",
            "[", "](", ")", "[a](b)", "**b**", "_i_", "`c`", "\r\n", "\n\n", "word", "Wrod",
            "teh", "don't", "e.g.", "https://x.y/z", "a@b.c", "#tag", "NASA", "naïve",
            "שלום", "مرحبا", "👍🏽", "👨\u{200D}👩\u{200D}👧", "e\u{301}", "  ", "\t",
        ])
        .prop_map(str::to_owned),
    ]
}

/// Text built from [`tricky_token`]s, up to `max` tokens long.
pub fn tricky_text(max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(tricky_token(), 0..max).prop_map(|t| t.concat())
}
