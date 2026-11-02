//! Crash recovery: unsaved work is mirrored to a small local cache so that
//! a crash, power cut, forced shutdown or a drive that never finishes
//! saving never loses it.
//!
//! Each document has its own slot: the never-saved draft, and one per file
//! whose latest text hasn't been confirmed written yet. Saves happen in the
//! background (see `crate::saver`), so several can be outstanding when you
//! switch documents. A slot is cleared once its text is safely in the file.

use std::fs;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

use crate::document::write_atomically;

const TEXT: &str = "txt";
const ORIGIN: &str = "origin";
/// The slot of the never-saved draft. (Not `draft`: older versions kept
/// their one slot in `draft.txt` and `origin.txt`, migrated on load.)
const DRAFT_SLOT: &str = "untitled";
const LEGACY_TEXT: &str = "draft.txt";
const LEGACY_ORIGIN: &str = "origin.txt";

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

    fn slot(path: Option<&Path>) -> String {
        match path {
            None => DRAFT_SLOT.to_owned(),
            Some(path) => {
                let mut hasher = std::hash::DefaultHasher::new();
                path.hash(&mut hasher);
                format!("file-{:016x}", hasher.finish())
            }
        }
    }

    /// Saves `text` as the unsaved work for `path` (or the draft).
    pub fn store(&self, text: &str, path: Option<&Path>) -> anyhow::Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        fs::create_dir_all(dir)?;
        let slot = Self::slot(path);
        let origin = path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Origin first: a crash between the two writes leaves a text whose
        // origin is at worst missing, never a lost text.
        write_atomically(&dir.join(format!("{slot}.{ORIGIN}")), origin.as_bytes())?;
        write_atomically(&dir.join(format!("{slot}.{TEXT}")), text.as_bytes())
    }

    /// The most recently stored unsaved work, if any has text.
    pub fn load(&self) -> Option<Draft> {
        self.load_all().into_iter().next()
    }

    /// All unsaved work, most recent first.
    pub fn load_all(&self) -> Vec<Draft> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        self.migrate_single_slot(dir);
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut found: Vec<(std::time::SystemTime, Draft)> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == TEXT))
            .filter(|e| e.file_name() != LEGACY_TEXT && e.file_name() != LEGACY_ORIGIN)
            .filter_map(|e| {
                let text = fs::read_to_string(e.path()).ok()?;
                if text.trim().is_empty() {
                    return None;
                }
                let origin =
                    fs::read_to_string(e.path().with_extension(ORIGIN)).unwrap_or_default();
                // Exactly as stored: file names may begin or end with spaces.
                let path = (!origin.is_empty()).then(|| PathBuf::from(origin));
                let when = e.metadata().and_then(|m| m.modified()).ok()?;
                Some((when, Draft { text, path }))
            })
            .collect();
        found.sort_by_key(|(when, _)| std::cmp::Reverse(*when));
        found.into_iter().map(|(_, d)| d).collect()
    }

    /// Forgets the unsaved work for `path` (or the draft): it was saved or
    /// deliberately discarded.
    pub fn clear_for(&self, path: Option<&Path>) {
        if let Some(dir) = &self.dir {
            let slot = Self::slot(path);
            for ext in [TEXT, ORIGIN] {
                let _ = fs::remove_file(dir.join(format!("{slot}.{ext}")));
            }
        }
    }

    /// Forgets all unsaved work.
    pub fn clear(&self) {
        let Some(dir) = &self.dir else {
            return;
        };
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let ext = e
                    .path()
                    .extension()
                    .map(|x| x.to_string_lossy().into_owned());
                if matches!(ext.as_deref(), Some(TEXT | ORIGIN)) {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }

    /// Moves work left by older versions (one `draft.txt` and its
    /// `origin.txt`) into its slot.
    fn migrate_single_slot(&self, dir: &Path) {
        let (old_text, old_origin) = (dir.join(LEGACY_TEXT), dir.join(LEGACY_ORIGIN));
        let Ok(text) = fs::read_to_string(&old_text) else {
            return;
        };
        let origin = fs::read_to_string(&old_origin).unwrap_or_default();
        let path = (!origin.trim().is_empty()).then(|| PathBuf::from(origin.trim()));
        if text.trim().is_empty() || self.store(&text, path.as_deref()).is_ok() {
            let _ = fs::remove_file(&old_text);
            let _ = fs::remove_file(&old_origin);
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

        let file = dir.path().join("story.md");
        r.store("edited", Some(&file)).unwrap();
        assert_eq!(r.load().unwrap().path, Some(file.clone()), "newest first");

        r.clear_for(Some(&file));
        assert_eq!(r.load().unwrap().path, None, "the draft is still there");
        r.clear();
        assert_eq!(r.load(), None);
    }

    #[test]
    fn each_document_keeps_its_own_unsaved_work() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path().to_path_buf()));
        let (a, b) = (dir.path().join("a.md"), dir.path().join("b.md"));
        r.store("a, unsaved", Some(&a)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        r.store("b, unsaved", Some(&b)).unwrap();
        r.store("b, more", Some(&b)).unwrap();
        let all = r.load_all();
        assert_eq!(all.len(), 2, "b's newer text replaced b's, not a's");
        assert_eq!(all[0].text, "b, more");
        assert_eq!(all[1].text, "a, unsaved");
    }

    #[test]
    fn blank_drafts_are_not_offered() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path().to_path_buf()));
        r.store("  \n", None).unwrap();
        assert_eq!(r.load(), None);
    }

    #[test]
    fn work_left_by_older_versions_is_still_recovered() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("draft.txt"), "from before").unwrap();
        std::fs::write(dir.path().join("origin.txt"), "/notes/old.md").unwrap();
        let r = Recovery::new(Some(dir.path().to_path_buf()));
        let draft = r.load().unwrap();
        assert_eq!(draft.text, "from before");
        assert_eq!(draft.path, Some(PathBuf::from("/notes/old.md")));
        assert!(!dir.path().join("draft.txt").exists());
    }
}
