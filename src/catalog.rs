//! Cyclable lists of backgrounds, music tracks and key-click sounds.
//!
//! Each list starts with sounds and images generated in code (so the app
//! works with no asset files at all), followed by any files the user drops
//! into the matching folder under `~/.local/share/samadhi/`.

use std::fs;
use std::path::{Path, PathBuf};

/// Where an entry comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source<B> {
    Builtin(B),
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<B> {
    /// Stable name stored in the settings file.
    pub name: String,
    pub source: Source<B>,
}

/// A non-empty list with a wrap-around cursor.
#[derive(Debug, Clone)]
pub struct Catalog<B> {
    entries: Vec<Entry<B>>,
    current: usize,
}

impl<B> Catalog<B> {
    /// Builds a catalogue from built-ins followed by files. Built-ins must not
    /// be empty, which guarantees there is always a current entry.
    pub fn new(builtins: Vec<(String, B)>, files: Vec<PathBuf>) -> Self {
        assert!(
            !builtins.is_empty(),
            "a catalogue needs at least one built-in entry"
        );
        let entries = builtins
            .into_iter()
            .map(|(name, b)| Entry {
                name,
                source: Source::Builtin(b),
            })
            .chain(files.into_iter().map(|path| Entry {
                name: display_name(&path),
                source: Source::File(path),
            }))
            .collect();
        Self {
            entries,
            current: 0,
        }
    }

    pub fn current(&self) -> &Entry<B> {
        &self.entries[self.current]
    }

    pub fn entries(&self) -> &[Entry<B>] {
        &self.entries
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Moves to entry `index` (ignored if out of range).
    pub fn select_index(&mut self, index: usize) {
        if index < self.entries.len() {
            self.current = index;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Always `false`: a catalogue always has its built-in entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn step_forward(&mut self) -> &Entry<B> {
        self.current = (self.current + 1) % self.entries.len();
        self.current()
    }

    pub fn step_back(&mut self) -> &Entry<B> {
        self.current = self
            .current
            .checked_sub(1)
            .unwrap_or(self.entries.len() - 1);
        self.current()
    }

    /// Moves to the entry called `name`; returns `false` (and stays put) if
    /// there is no such entry, e.g. because a file was removed.
    pub fn select(&mut self, name: &str) -> bool {
        match self.entries.iter().position(|e| e.name == name) {
            Some(i) => {
                self.current = i;
                true
            }
            None => false,
        }
    }
}

/// A readable name for a file: no extension, underscores as spaces
/// (`Slow_Light_on_Water.mp3` → `Slow Light on Water`).
pub fn display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let words: Vec<&str> = stem.split(['_', ' ']).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        stem
    } else {
        words.join(" ")
    }
}

/// Lists files in `dir` whose extension is in `extensions`, sorted
/// case-insensitively. A missing directory simply yields nothing.
pub fn scan_dir(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
        })
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog<u8> {
        Catalog::new(
            vec![("a".into(), 1), ("b".into(), 2)],
            vec![PathBuf::from("/x/c.mp3")],
        )
    }

    #[test]
    fn cycles_in_both_directions_with_wrap_around() {
        let mut c = catalog();
        assert_eq!(c.len(), 3);
        assert_eq!(c.current().name, "a");
        assert_eq!(c.step_back().name, "c");
        assert_eq!(c.step_forward().name, "a");
        assert_eq!(c.step_forward().name, "b");
        assert_eq!(c.step_forward().name, "c");
        assert_eq!(c.step_forward().name, "a");
    }

    #[test]
    fn select_by_name_and_ignore_unknown() {
        let mut c = catalog();
        assert!(c.select("c"));
        assert_eq!(c.current().source, Source::File("/x/c.mp3".into()));
        assert!(!c.select("gone.mp3"));
        assert_eq!(c.current().name, "c");
    }

    #[test]
    fn file_names_are_shown_cleanly() {
        let name = |p: &str| display_name(Path::new(p));
        assert_eq!(name("/m/Slow_Light_on_Water.mp3"), "Slow Light on Water");
        assert_eq!(name("/m/rain  at night.ogg"), "rain at night");
        assert_eq!(name("/m/__.wav"), "__");
    }

    #[test]
    fn scan_filters_and_sorts_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.MP3", "a.wav", "notes.txt", "C.ogg"] {
            fs::write(dir.path().join(name), b"").unwrap();
        }
        fs::create_dir(dir.path().join("sub.mp3")).unwrap();
        let names: Vec<_> = scan_dir(dir.path(), &["mp3", "wav", "ogg"])
            .into_iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.wav", "b.MP3", "C.ogg"]);
        assert_eq!(
            scan_dir(&dir.path().join("missing"), &["mp3"]),
            Vec::<PathBuf>::new()
        );
    }
}
