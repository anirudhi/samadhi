//! Spell checking with Hunspell dictionaries (via `spellbook`).
//!
//! US English is bundled (SCOWL word list, see
//! `assets/dictionaries/en_US-COPYRIGHT.txt`); if a Hunspell dictionary for
//! the system language is installed (e.g. `en_GB` in `/usr/share/hunspell`)
//! that is used instead. Words added by the user go into a personal
//! dictionary file, one per line.

use std::collections::HashSet;
use std::fs;
use std::io::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::markdown::{self, Role};

const BUNDLED_AFF: &str = include_str!("../assets/dictionaries/en_US.aff");
const BUNDLED_DIC: &str = include_str!("../assets/dictionaries/en_US.dic");
const SYSTEM_DIRS: &[&str] = &["/usr/share/hunspell", "/usr/share/myspell"];
const MAX_SUGGESTIONS: usize = 6;

pub struct SpellChecker {
    dict: spellbook::Dictionary,
    /// Name of the dictionary in use, e.g. `en_US`.
    pub language: String,
    personal: HashSet<String>,
    ignored: HashSet<String>,
    personal_path: Option<PathBuf>,
}

impl std::fmt::Debug for SpellChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpellChecker")
            .field("language", &self.language)
            .field("personal_words", &self.personal.len())
            .finish_non_exhaustive()
    }
}

/// The system language as a Hunspell dictionary name, e.g. `en_GB`.
fn system_language() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .map(|v| v.split(['.', '@']).next().unwrap_or_default().to_owned())
}

impl SpellChecker {
    /// Loads the best available dictionary and the personal word list.
    /// Parsing takes a moment, so the app does this on a worker thread.
    pub fn load(personal_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let system = system_language().and_then(|lang| {
            SYSTEM_DIRS.iter().find_map(|dir| {
                let aff = fs::read_to_string(Path::new(dir).join(format!("{lang}.aff"))).ok()?;
                let dic = fs::read_to_string(Path::new(dir).join(format!("{lang}.dic"))).ok()?;
                Some((lang.clone(), aff, dic))
            })
        });
        let (language, dict) = match system {
            Some((lang, aff, dic)) => match spellbook::Dictionary::new(&aff, &dic) {
                Ok(dict) => (lang, dict),
                Err(err) => {
                    log::warn!("system dictionary {lang} unusable ({err}); using en_US");
                    ("en_US".to_owned(), Self::bundled()?)
                }
            },
            None => ("en_US".to_owned(), Self::bundled()?),
        };
        let mut checker = Self {
            dict,
            language,
            personal: HashSet::new(),
            ignored: HashSet::new(),
            personal_path,
        };
        if let Some(path) = &checker.personal_path
            && let Ok(text) = fs::read_to_string(path)
        {
            for word in text.lines().map(str::trim).filter(|w| !w.is_empty()) {
                checker.personal.insert(word.to_owned());
            }
        }
        Ok(checker)
    }

    fn bundled() -> anyhow::Result<spellbook::Dictionary> {
        spellbook::Dictionary::new(BUNDLED_AFF, BUNDLED_DIC)
            .map_err(|e| anyhow::anyhow!("parsing the bundled dictionary: {e}"))
    }

    pub fn is_correct(&self, word: &str) -> bool {
        let word = normalize(word);
        self.personal.contains(&word) || self.ignored.contains(&word) || self.dict.check(&word)
    }

    /// Suggestions for `word`, best first.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let word = normalize(word);
        let mut out = Vec::new();
        self.dict.suggest(&word, &mut out);
        // For a capitalised word, also try the lower-case form (so "Teh"
        // finds "the") and capitalise those answers back.
        let mut chars = word.chars();
        if let Some(first) = chars.next()
            && first.is_uppercase()
        {
            let lower: String = first.to_lowercase().chain(chars).collect();
            let mut more = Vec::new();
            self.dict.suggest(&lower, &mut more);
            out.extend(more.into_iter().map(|s| capitalize(&s)));
        }
        rank(&word, out)
    }

    /// Accepts `word` for the rest of this session.
    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(normalize(word));
    }

    /// Adds `word` to the personal dictionary file.
    pub fn add_word(&mut self, word: &str) -> anyhow::Result<()> {
        let word = normalize(word);
        if self.personal.insert(word.clone())
            && let Some(path) = &self.personal_path
        {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("opening {}", path.display()))?;
            writeln!(file, "{word}")?;
        }
        Ok(())
    }

    /// Byte ranges of misspelled words in `text`, skipping code, links and
    /// markdown syntax.
    pub fn misspellings(&self, text: &str) -> Vec<Range<usize>> {
        let excluded: Vec<Range<usize>> = markdown::parse(text)
            .into_iter()
            .filter(|s| matches!(s.style.role, Role::Code | Role::Fence | Role::Marker))
            .map(|s| s.range.0..s.range.1)
            .collect();
        words(text)
            .filter(|r| !excluded.iter().any(|e| e.start < r.end && r.start < e.end))
            .filter(|r| !self.is_correct(&text[r.clone()]))
            .collect()
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(chars).collect())
}

/// Orders suggestions the way people expect: a swapped pair of letters (the
/// most common slip) first, then by edit distance, preferring the same
/// length and first letter, and otherwise keeping the dictionary's order.
fn rank(word: &str, suggestions: Vec<String>) -> Vec<String> {
    let w: Vec<char> = word.to_lowercase().chars().collect();
    let mut seen = HashSet::new();
    let mut scored: Vec<(bool, usize, bool, bool, usize, String)> = suggestions
        .into_iter()
        .filter(|s| seen.insert(s.clone()))
        .enumerate()
        .map(|(order, s)| {
            let c: Vec<char> = s.to_lowercase().chars().collect();
            (
                !is_swap(&w, &c),
                edit_distance(&w, &c),
                c.len() != w.len(),
                c.first() != w.first(),
                order,
                s,
            )
        })
        .collect();
    scored.sort();
    scored
        .into_iter()
        .map(|t| t.5)
        .take(MAX_SUGGESTIONS)
        .collect()
}

/// True if `b` is `a` with exactly one pair of adjacent letters swapped.
fn is_swap(a: &[char], b: &[char]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diffs: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
    diffs.len() == 2
        && diffs[1] == diffs[0] + 1
        && a[diffs[0]] == b[diffs[1]]
        && a[diffs[1]] == b[diffs[0]]
}

/// Damerau–Levenshtein distance (with adjacent transpositions).
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[n][m]
}

/// Curly apostrophes are checked as straight ones.
fn normalize(word: &str) -> String {
    word.replace('\u{2019}', "'")
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

/// Byte ranges of the words worth checking: runs of letters (with inner
/// apostrophes), skipping anything that looks like a URL, path, e-mail,
/// number, hashtag or acronym.
pub fn words(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut chunks = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                chunks.push(s..i);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        chunks.push(s..text.len());
    }
    chunks.into_iter().flat_map(move |chunk| {
        let s = &text[chunk.clone()];
        let technical = s.contains("://")
            || s.contains('@')
            || s.contains('/')
            || s.contains('\\')
            || s.contains('_')
            || s.starts_with('#') && s.len() > 1 && !s[1..].starts_with([' ', '#'])
            || s.chars().any(|c| c.is_ascii_digit());
        let mut out = Vec::new();
        if technical {
            return out;
        }
        let mut word_start: Option<usize> = None;
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        for (k, &(i, c)) in chars.iter().enumerate() {
            let inner_apostrophe = is_apostrophe(c)
                && word_start.is_some()
                && chars.get(k + 1).is_some_and(|&(_, n)| n.is_alphabetic());
            if c.is_alphabetic() || inner_apostrophe {
                word_start.get_or_insert(i);
            } else if let Some(ws) = word_start.take() {
                out.push(chunk.start + ws..chunk.start + i);
            }
        }
        if let Some(ws) = word_start {
            out.push(chunk.start + ws..chunk.end);
        }
        out.retain(|r| {
            let w = &text[r.clone()];
            let acronym = w.chars().count() > 1 && w.chars().all(|c| !c.is_lowercase());
            w.chars().count() > 1 && !acronym
        });
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(dir: &Path) -> SpellChecker {
        // Force the bundled dictionary so tests don't depend on the system.
        SpellChecker {
            dict: SpellChecker::bundled().unwrap(),
            language: "en_US".into(),
            personal: HashSet::new(),
            ignored: HashSet::new(),
            personal_path: Some(dir.join("dictionary.txt")),
        }
    }

    fn found<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
        ranges.iter().map(|r| &text[r.clone()]).collect()
    }

    #[test]
    fn tokenizer_skips_technical_text() {
        let text =
            "Don't visit https://x.io or mail a@b.c; NASA ran 3rd #tag snake_case it’s fine.";
        let words: Vec<_> = words(text).map(|r| &text[r]).collect();
        assert_eq!(
            words,
            ["Don't", "visit", "or", "mail", "ran", "it’s", "fine"]
        );
    }

    #[test]
    fn finds_misspellings_but_not_in_code_or_markdown_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let c = checker(dir.path());
        let text =
            "# Teh heading\nThis is **wrng** but `codez` and [lnk](httpz) are fine. It’s calm.";
        assert_eq!(found(text, &c.misspellings(text)), ["Teh", "wrng", "lnk"]);
    }

    #[test]
    fn the_obvious_fix_comes_first() {
        let dir = tempfile::tempdir().unwrap();
        let c = checker(dir.path());
        assert_eq!(c.suggest("teh")[0], "the");
        assert_eq!(c.suggest("Teh")[0], "The");
        assert_eq!(c.suggest("recieve")[0], "receive");
        assert!(c.suggest("speling").iter().any(|s| s == "spelling"));
    }

    #[test]
    fn edit_distance_counts_swaps_as_one() {
        let v = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(edit_distance(&v("teh"), &v("the")), 1);
        assert_eq!(edit_distance(&v("kitten"), &v("sitting")), 3);
        assert!(is_swap(&v("teh"), &v("the")));
        assert!(!is_swap(&v("teh"), &v("ten")));
    }

    #[test]
    fn personal_words_persist_and_ignored_words_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = checker(dir.path());
        assert!(!c.is_correct("Zenwrite"));
        c.add_word("Zenwrite").unwrap();
        c.ignore("vipassanaa");
        assert!(c.is_correct("Zenwrite") && c.is_correct("vipassanaa"));

        let reloaded = SpellChecker::load(Some(dir.path().join("dictionary.txt"))).unwrap();
        assert!(reloaded.is_correct("Zenwrite"));
        assert!(!reloaded.is_correct("vipassanaa"));
    }
}
