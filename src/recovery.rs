//! Crash recovery: unsaved work is mirrored to a small cache so that a
//! crash, power cut or forced shutdown never loses it. The cache holds the
//! text and, for an existing file with unsaved edits, that file's path.

use std::fs;
use std::path::{Path, PathBuf};

use crate::document::write_atomically;

const TEXT_FILE: &str = "draft.txt";
const ORIGIN_FILE: &str = "origin.txt";

/// Recovered unsaved work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    /// The file the edits belong to, or `None` for a never-saved draft.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Recovery {
    dir: Option<PathBuf>,
}

impl Recovery {
    /// `~/.local/share/samadhi/recovery`, unless overridden.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let dir = dir.or_else(|| crate::project_dirs().map(|d| d.data_dir().join("recovery")));
        Self { dir }
    }

    /// Saves `text` (and the file it belongs to, if any) to the cache.
    pub fn store(&self, text: &str, path: Option<&Path>) -> anyhow::Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        fs::create_dir_all(dir)?;
        let origin = path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Origin first: a crash between the two writes leaves a draft whose
        // origin is at worst the previous one, never a lost text.
        write_atomically(&dir.join(ORIGIN_FILE), origin.as_bytes())?;
        write_atomically(&dir.join(TEXT_FILE), text.as_bytes())
    }

    /// The cached draft, if there is one with any text.
    pub fn load(&self) -> Option<Draft> {
        let dir = self.dir.as_ref()?;
        let text = fs::read_to_string(dir.join(TEXT_FILE)).ok()?;
        if text.trim().is_empty() {
            return None;
        }
        let origin = fs::read_to_string(dir.join(ORIGIN_FILE)).unwrap_or_default();
        let path = (!origin.trim().is_empty()).then(|| PathBuf::from(origin.trim()));
        Some(Draft { text, path })
    }

    /// Forgets the cached draft (it was saved or deliberately discarded).
    pub fn clear(&self) {
        if let Some(dir) = &self.dir {
            for name in [TEXT_FILE, ORIGIN_FILE] {
                let _ = fs::remove_file(dir.join(name));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_loads_and_clears_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path().join("recovery")));
        assert_eq!(r.load(), None);

        r.store("an unsaved thought", None).unwrap();
        assert_eq!(
            r.load(),
            Some(Draft {
                text: "an unsaved thought".into(),
                path: None
            })
        );

        let file = dir.path().join("story.omm");
        r.store("edited", Some(&file)).unwrap();
        assert_eq!(r.load().unwrap().path, Some(file));

        r.clear();
        assert_eq!(r.load(), None);
    }

    #[test]
    fn blank_drafts_are_not_offered() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path().to_path_buf()));
        r.store("  \n", None).unwrap();
        assert_eq!(r.load(), None);
    }
}
