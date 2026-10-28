//! Reading and writing plain-text and Markdown documents.
//!
//! Any plain-text file can be opened, whatever its extension. Files written
//! by other apps, especially on Windows, may be UTF-16 or in the legacy
//! Windows-1252 code page, and may use CRLF line endings, so the original
//! encoding, byte-order mark and line ending style are detected on load and
//! restored on save. That way a document round-trips unchanged.

use std::fs;
use std::hash::{Hash as _, Hasher as _};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context as _, bail};

/// File extensions offered in the open/save dialogs.
pub const TEXT_EXTENSIONS: &[&str] = &["md", "markdown", "txt", "text"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8 { bom: bool },
    Utf16Le { bom: bool },
    Utf16Be { bom: bool },
    Windows1252,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// What the file on disk looked like when it was last read or written, to
/// notice when something else (another app, or a sync from another device)
/// changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    hash: u64,
}

impl Stamp {
    fn of(meta: &fs::Metadata, bytes: &[u8]) -> Self {
        let mut hasher = std::hash::DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self {
            modified: meta.modified().ok(),
            len: meta.len(),
            hash: hasher.finish(),
        }
    }

    fn read(path: &Path, bytes: &[u8]) -> Option<Self> {
        fs::metadata(path).ok().map(|meta| Self::of(&meta, bytes))
    }
}

/// The open document. `text` always uses `\n` line endings internally.
#[derive(Debug)]
pub struct Document {
    pub text: String,
    path: Option<PathBuf>,
    encoding: TextEncoding,
    line_ending: LineEnding,
    dirty: bool,
    disk: Option<Stamp>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            text: String::new(),
            path: None,
            encoding: TextEncoding::Utf8 { bom: false },
            line_ending: LineEnding::Lf,
            dirty: false,
            disk: None,
        }
    }
}

/// What happened during a save, so the UI can tell the user about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved,
    /// The text contained characters the original legacy encoding cannot
    /// represent, so the file was written as UTF-8 instead.
    SavedAsUtf8,
}

impl Document {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_bytes(path, &bytes, Stamp::read(path, &bytes))
    }

    fn from_bytes(path: &Path, bytes: &[u8], disk: Option<Stamp>) -> anyhow::Result<Self> {
        let (raw, encoding) =
            decode(bytes).with_context(|| format!("decoding {}", path.display()))?;
        let (text, line_ending) = normalize_line_endings(&raw);
        Ok(Self {
            text,
            path: Some(path.to_path_buf()),
            encoding,
            line_ending,
            dirty: false,
            disk,
        })
    }

    /// Whether a file looking like this (size and modification time, cheap
    /// to read off the interface thread) is the one last read or written.
    /// When it isn't, [`Self::changed_on_disk`] reads it to tell.
    pub fn disk_matches(&self, meta: &fs::Metadata) -> bool {
        self.disk
            .is_some_and(|seen| meta.len() == seen.len && meta.modified().ok() == seen.modified)
    }

    /// The version of the file now on disk, if something else has changed
    /// it since this document last read or wrote it: another app, or a sync
    /// from another device. Each change is reported once.
    pub fn changed_on_disk(&mut self) -> Option<Self> {
        let path = self.path.as_deref()?;
        let seen = self.disk?;
        let meta = fs::metadata(path).ok()?;
        if self.disk_matches(&meta) {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        let now = Stamp::of(&meta, &bytes);
        self.disk = Some(now);
        if now.hash == seen.hash {
            return None; // Touched, not changed (a sync tool rewrote it).
        }
        Self::from_bytes(path, &bytes, Some(now)).ok()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn display_name(&self) -> String {
        self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn word_count(&self) -> usize {
        count_words(&self.text)
    }

    /// Saves to the current path. Fails if the document has never been saved.
    pub fn save(&mut self) -> anyhow::Result<SaveOutcome> {
        let Some(path) = self.path.clone() else {
            bail!("document has no file name yet");
        };
        self.save_as(&path)
    }

    /// Saves to `path` atomically and makes it the document's path.
    pub fn save_as(&mut self, path: &Path) -> anyhow::Result<SaveOutcome> {
        let with_endings = if self.line_ending == LineEnding::Lf {
            self.text.clone()
        } else {
            self.text.replace('\n', self.line_ending.as_str())
        };
        let (bytes, outcome) = encode(&with_endings, self.encoding);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        write_atomically(path, &bytes)?;
        self.disk = Stamp::read(path, &bytes);
        if outcome == SaveOutcome::SavedAsUtf8 {
            self.encoding = TextEncoding::Utf8 { bom: false };
        }
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(outcome)
    }
}

/// A free name beside `path` for keeping a second version of it, e.g.
/// `Essay (conflicted copy).md`, then `Essay (conflicted copy 2).md`.
pub fn conflict_copy_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "Untitled".into(), |s| s.to_string_lossy());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..u32::MAX)
        .map(|n| {
            let tag = if n == 1 {
                "conflicted copy".to_owned()
            } else {
                format!("conflicted copy {n}")
            };
            path.with_file_name(format!("{stem} ({tag}){ext}"))
        })
        .find(|candidate| !candidate.exists())
        .expect("some name is free")
}

/// Words in `text`: runs of non-whitespace containing a letter or digit, so
/// markdown symbols (`#`, `>`, `-` bullets, `---`) and stray punctuation
/// are not counted.
pub fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

/// Writes `bytes` to a temporary file next to `path`, syncs it and renames it
/// over `path`, so a crash or full disk never leaves a half-written document.
/// The permissions of an existing file are preserved.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let mut tmp = tempfile::Builder::new()
        .prefix(".samadhi-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .with_context(|| format!("creating a temporary file in {}", dir.display()))?;
    tmp.write_all(bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .with_context(|| format!("writing {}", path.display()))?;
    if let Ok(meta) = fs::metadata(path) {
        // Best effort: a failure here should not block saving the text.
        let _ = fs::set_permissions(tmp.path(), meta.permissions());
    }
    tmp.persist(path)
        .with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

fn decode(bytes: &[u8]) -> anyhow::Result<(String, TextEncoding)> {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        let text = std::str::from_utf8(rest).context("invalid UTF-8 after byte-order mark")?;
        return Ok((text.to_owned(), TextEncoding::Utf8 { bom: true }));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return Ok((
            decode_utf16(rest, true)?,
            TextEncoding::Utf16Le { bom: true },
        ));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return Ok((
            decode_utf16(rest, false)?,
            TextEncoding::Utf16Be { bom: true },
        ));
    }
    if let Some(little_endian) = sniff_bomless_utf16(bytes) {
        let text = decode_utf16(bytes, little_endian)?;
        let enc = if little_endian {
            TextEncoding::Utf16Le { bom: false }
        } else {
            TextEncoding::Utf16Be { bom: false }
        };
        return Ok((text, enc));
    }
    // Valid UTF-8 is text, even with the odd NUL in it (pasted from
    // somewhere), so whatever Samādhi saved it can always open again.
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_owned(), TextEncoding::Utf8 { bom: false }));
    }
    if bytes.contains(&0) {
        bail!("this does not look like a plain-text document");
    }
    let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
    Ok((text.into_owned(), TextEncoding::Windows1252))
}

/// Detects UTF-16 without a BOM: mostly-ASCII text has a zero in every other
/// byte. Returns `Some(true)` for little endian.
fn sniff_bomless_utf16(bytes: &[u8]) -> Option<bool> {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let pairs = bytes.len() / 2;
    let zeros_at = |offset: usize| {
        bytes
            .iter()
            .skip(offset)
            .step_by(2)
            .filter(|&&b| b == 0)
            .count()
    };
    let (even, odd) = (zeros_at(0), zeros_at(1));
    let mostly = |n: usize| n * 10 >= pairs * 4; // at least 40% of code units
    let rarely = |n: usize| n * 10 <= pairs;
    match (mostly(odd) && rarely(even), mostly(even) && rarely(odd)) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> anyhow::Result<String> {
    if !bytes.len().is_multiple_of(2) {
        bail!("UTF-16 text has an odd number of bytes");
    }
    let units = bytes.as_chunks::<2>().0.iter().map(|&pair| {
        if little_endian {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    });
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .context("invalid UTF-16 text")
}

fn encode(text: &str, encoding: TextEncoding) -> (Vec<u8>, SaveOutcome) {
    let utf16 = |bom: bool, to_bytes: fn(u16) -> [u8; 2]| {
        let mut out = Vec::with_capacity(text.len() * 2 + 2);
        if bom {
            out.extend_from_slice(&to_bytes(0xFEFF));
        }
        for unit in text.encode_utf16() {
            out.extend_from_slice(&to_bytes(unit));
        }
        out
    };
    match encoding {
        TextEncoding::Utf8 { bom } => {
            let mut out = Vec::with_capacity(text.len() + 3);
            if bom {
                out.extend_from_slice(b"\xEF\xBB\xBF");
            }
            out.extend_from_slice(text.as_bytes());
            (out, SaveOutcome::Saved)
        }
        TextEncoding::Utf16Le { bom } => (utf16(bom, u16::to_le_bytes), SaveOutcome::Saved),
        TextEncoding::Utf16Be { bom } => (utf16(bom, u16::to_be_bytes), SaveOutcome::Saved),
        TextEncoding::Windows1252 => {
            let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(text);
            if unmappable {
                (text.as_bytes().to_vec(), SaveOutcome::SavedAsUtf8)
            } else {
                (bytes.into_owned(), SaveOutcome::Saved)
            }
        }
    }
}

/// Converts any mix of CRLF / CR / LF to LF, returning the dominant style so
/// it can be restored on save.
fn normalize_line_endings(raw: &str) -> (String, LineEnding) {
    let crlf = raw.matches("\r\n").count();
    let cr = raw.matches('\r').count() - crlf;
    let lf = raw.matches('\n').count() - crlf;
    let style = if crlf >= lf && crlf >= cr && crlf > 0 {
        LineEnding::CrLf
    } else if cr > lf {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    };
    if crlf == 0 && cr == 0 {
        return (raw.to_owned(), style);
    }
    (raw.replace("\r\n", "\n").replace('\r', "\n"), style)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(bytes: &[u8], name: &str) -> (Document, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.mark_dirty();
        doc.save().unwrap();
        let written = fs::read(&path).unwrap();
        (doc, written)
    }

    #[test]
    fn saving_into_a_new_folder_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b/Draft.md");
        let mut doc = Document {
            text: "hi".into(),
            ..Document::default()
        };
        doc.save_as(&nested).unwrap();
        assert_eq!(fs::read_to_string(nested).unwrap(), "hi");
    }

    #[test]
    fn plain_utf8_round_trips_byte_for_byte() {
        let bytes = "Once upon a time…\nthe end.\n".as_bytes();
        let (doc, written) = round_trip(bytes, "story.md");
        assert_eq!(doc.text, "Once upon a time…\nthe end.\n");
        assert_eq!(written, bytes);
        assert_eq!(doc.display_name(), "story.md");
    }

    #[test]
    fn utf8_bom_and_crlf_are_preserved() {
        let bytes = b"\xEF\xBB\xBFline one\r\nline two\r\n";
        let (doc, written) = round_trip(bytes, "win.md");
        assert_eq!(doc.text, "line one\nline two\n");
        assert_eq!(written, bytes);
    }

    #[test]
    fn utf16_with_and_without_bom_round_trips() {
        let text = "Grüße\r\nfrom afar";
        let mut le_bom = vec![0xFF, 0xFE];
        le_bom.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let (doc, written) = round_trip(&le_bom, "a.md");
        assert_eq!(doc.text, "Grüße\nfrom afar");
        assert_eq!(written, le_bom);

        let be: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let (doc, written) = round_trip(&be, "b.md");
        assert_eq!(doc.text, "Grüße\nfrom afar");
        assert_eq!(written, be);
    }

    #[test]
    fn legacy_windows_1252_round_trips() {
        let bytes = b"caf\xe9 \x93quoted\x94";
        let (doc, written) = round_trip(bytes, "old.md");
        assert_eq!(doc.text, "café \u{201c}quoted\u{201d}");
        assert_eq!(written, bytes);
    }

    #[test]
    fn unmappable_text_in_legacy_file_falls_back_to_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.md");
        fs::write(&path, b"caf\xe9").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text.push_str(" 日本");
        assert_eq!(doc.save().unwrap(), SaveOutcome::SavedAsUtf8);
        assert_eq!(fs::read_to_string(&path).unwrap(), "café 日本");
    }

    #[test]
    fn binary_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.md");
        fs::write(&path, [0x89, b'P', b'N', b'G', 0, 0, 0, 13, 1, 2, 3]).unwrap();
        assert!(Document::open(&path).is_err());
    }

    #[test]
    fn save_without_path_fails_and_save_as_sets_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = Document {
            text: "hello".into(),
            ..Document::default()
        };
        doc.mark_dirty();
        assert!(doc.save().is_err());
        let path = dir.path().join("new.md");
        doc.save_as(&path).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn atomic_write_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.txt");
        write_atomically(&path, b"one").unwrap();
        write_atomically(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn line_endings_normalise_and_pick_dominant_style() {
        assert_eq!(
            normalize_line_endings("a\r\nb\r\nc\n"),
            ("a\nb\nc\n".into(), LineEnding::CrLf)
        );
        assert_eq!(
            normalize_line_endings("a\rb\r"),
            ("a\nb\n".into(), LineEnding::Cr)
        );
        assert_eq!(
            normalize_line_endings("a\nb"),
            ("a\nb".into(), LineEnding::Lf)
        );
    }

    #[test]
    fn changes_made_elsewhere_are_noticed_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.md");
        fs::write(&path, "first").unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert!(doc.changed_on_disk().is_none());

        // Our own saves are not changes from elsewhere.
        doc.text = "ours".into();
        doc.save().unwrap();
        assert!(doc.changed_on_disk().is_none());

        fs::write(&path, "theirs, from another device").unwrap();
        let theirs = doc.changed_on_disk().expect("change noticed");
        assert_eq!(theirs.text, "theirs, from another device");
        assert!(doc.changed_on_disk().is_none(), "reported once");

        // Rewriting the same bytes (as sync tools do) is not a change.
        fs::write(&path, "theirs, from another device").unwrap();
        assert!(doc.changed_on_disk().is_none());
        assert!(doc.disk_matches(&fs::metadata(&path).unwrap()));
        fs::write(&path, "a third version").unwrap();
        assert!(!doc.disk_matches(&fs::metadata(&path).unwrap()));
    }

    #[test]
    fn conflict_copies_get_free_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Essay.md");
        let first = conflict_copy_path(&path);
        assert_eq!(first, dir.path().join("Essay (conflicted copy).md"));
        fs::write(&first, "x").unwrap();
        assert_eq!(
            conflict_copy_path(&path),
            dir.path().join("Essay (conflicted copy 2).md")
        );
    }

    #[test]
    fn markdown_symbols_are_not_words() {
        assert_eq!(
            count_words("# Title\n\n> quote here\n\n- one\n- two\n\n---\n**bold** — end"),
            7
        );
    }

    #[test]
    fn word_count_counts_whitespace_separated_words() {
        let doc = Document {
            text: "  one two\nthree\t four ".into(),
            ..Document::default()
        };
        assert_eq!(doc.word_count(), 4);
    }
}
