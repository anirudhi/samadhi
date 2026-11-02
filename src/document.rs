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

/// Every document is saved as Markdown. A file with another extension (a
/// `.txt`, or an older OmmWriter-style document) still opens, but its text
/// is saved to a `.md` file beside it, leaving the original untouched.
pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];

/// Whether `path` is a Markdown file by its extension.
pub fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        MARKDOWN_EXTENSIONS
            .iter()
            .any(|m| m.eq_ignore_ascii_case(e))
    })
}

/// Plain-text extensions that `.md` replaces.
const REPLACED_EXTENSIONS: &[&str] = &["txt", "text", "omm"];

/// Where the text of `path` is saved: `path` itself for Markdown, else the
/// same name ending in `.md` (`Essay.omm` → `Essay.md`). Any other dot is
/// part of the name: `Chapter 1. Intro` → `Chapter 1. Intro.md`.
pub fn markdown_path(path: &Path) -> PathBuf {
    if is_markdown(path) {
        return path.to_path_buf();
    }
    let replaced = path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        REPLACED_EXTENSIONS
            .iter()
            .any(|r| r.eq_ignore_ascii_case(e))
    });
    if replaced {
        path.with_extension("md")
    } else {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".md");
        path.with_file_name(name)
    }
}

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

    /// Same size and modification time: almost certainly untouched.
    fn matches(self, meta: &fs::Metadata) -> bool {
        meta.len() == self.len && meta.modified().ok() == self.modified
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
    /// Counts edits, so a save that finishes after more typing doesn't
    /// mark the newer text as saved.
    revision: u64,
    disk: Option<Stamp>,
}

/// A save, prepared from the text on the interface thread and carried out
/// on another (see `crate::saver`), so a slow disk can never freeze typing.
#[derive(Debug, Clone)]
pub struct SaveJob {
    pub path: PathBuf,
    bytes: Vec<u8>,
    outcome: SaveOutcome,
    /// The text revision being saved.
    pub revision: u64,
    /// The file as this document last read or wrote it. If the disk no
    /// longer matches, something else changed it, and that version is
    /// kept as a conflicted copy before this one is written.
    expected: Option<Stamp>,
    /// A new file, which must never replace one already there: if the name
    /// is taken (say `Essay.md` exists beside `Essay.omm`), a free name is
    /// used instead (`Essay (2).md`), reported in [`Saved::path`].
    pub fresh: bool,
}

/// A finished [`SaveJob`].
#[derive(Debug, Clone)]
pub struct Saved {
    /// Where the text was written.
    pub path: PathBuf,
    /// Where it was asked to go (differs from `path` only for a fresh file
    /// whose name was taken).
    pub requested: PathBuf,
    pub revision: u64,
    pub outcome: SaveOutcome,
    /// Where another version found on disk was kept, if there was one.
    pub conflict_copy: Option<PathBuf>,
    stamp: Option<Stamp>,
}

impl SaveJob {
    /// Writes the file (and a conflicted copy first, if needed). Blocking:
    /// call it off the interface thread.
    pub fn run(&self) -> anyhow::Result<Saved> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let path = if self.fresh {
            free_path(&self.path)
        } else {
            self.path.clone()
        };
        let conflict_copy = match self.theirs() {
            Some(theirs) if !self.fresh => {
                let copy = conflict_copy_path(&path);
                write_atomically(&copy, &theirs)?;
                Some(copy)
            }
            _ => None,
        };
        write_document(&path, &self.bytes)?;
        Ok(Saved {
            stamp: Stamp::read(&path, &self.bytes),
            requested: self.path.clone(),
            path,
            revision: self.revision,
            outcome: self.outcome,
            conflict_copy,
        })
    }

    /// Another version on disk that this save would overwrite: changed
    /// since this document last read or wrote the file, and different.
    fn theirs(&self) -> Option<Vec<u8>> {
        let expected = self.expected?;
        let meta = fs::metadata(&self.path).ok()?;
        if expected.matches(&meta) {
            return None;
        }
        let bytes = fs::read(&self.path).ok()?;
        let changed = Stamp::of(&meta, &bytes).hash != expected.hash && bytes != self.bytes;
        changed.then_some(bytes)
    }
}

/// A look at the open file, taken off the interface thread: cheap when the
/// file is unchanged (size and time), reading it only when not.
#[derive(Debug, Clone)]
pub struct DiskProbe {
    path: PathBuf,
    seen: Option<Stamp>,
}

/// The file's contents, read by a [`DiskProbe`] because they changed.
#[derive(Debug, Clone)]
pub struct DiskVersion {
    pub path: PathBuf,
    bytes: Vec<u8>,
    stamp: Stamp,
}

impl DiskProbe {
    /// Blocking: call it off the interface thread.
    pub fn run(&self) -> Option<DiskVersion> {
        let meta = fs::metadata(&self.path).ok()?;
        if self.seen.is_some_and(|seen| seen.matches(&meta)) {
            return None;
        }
        let bytes = fs::read(&self.path).ok()?;
        Some(DiskVersion {
            path: self.path.clone(),
            stamp: Stamp::of(&meta, &bytes),
            bytes,
        })
    }
}

impl Default for Document {
    fn default() -> Self {
        Self {
            text: String::new(),
            path: None,
            encoding: TextEncoding::Utf8 { bom: false },
            line_ending: LineEnding::Lf,
            dirty: false,
            revision: 0,
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
            revision: 0,
            disk,
        })
    }

    /// Whether a file looking like this (size and modification time, cheap
    /// to read off the interface thread) is the one last read or written.
    /// When it isn't, a [`DiskProbe`] reads it to tell.
    pub fn disk_matches(&self, meta: &fs::Metadata) -> bool {
        self.disk.is_some_and(|seen| seen.matches(meta))
    }

    /// A probe for changes to the open file, to run off the interface
    /// thread (see [`Self::take_disk_version`]).
    pub fn disk_probe(&self) -> Option<DiskProbe> {
        Some(DiskProbe {
            path: self.path.clone()?,
            seen: self.disk,
        })
    }

    /// Takes in what a [`DiskProbe`] found. Returns the other version when
    /// it should replace the text: it differs, it is still this document's
    /// file, and there are no unsaved edits. (With unsaved edits, the next
    /// save keeps the other version as a conflicted copy.)
    pub fn take_disk_version(&mut self, found: DiskVersion) -> Option<Self> {
        let DiskVersion { path, bytes, stamp } = found;
        if self.path.as_deref() != Some(path.as_path()) {
            return None;
        }
        let seen = self.disk?;
        if stamp.hash == seen.hash {
            self.disk = Some(stamp); // Touched, not changed.
            return None;
        }
        if self.dirty {
            return None;
        }
        Self::from_bytes(&path, &bytes, Some(stamp)).ok()
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
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A save of the text as it is now, to the document's file. A document
    /// opened from a file that isn't Markdown (`.omm`, `.txt`) moves to a
    /// new `.md` file beside it on its first save (see [`markdown_path`]);
    /// the original is left as it was.
    pub fn save_job(&mut self) -> Option<SaveJob> {
        let path = self.path.clone()?;
        if is_markdown(&path) {
            return Some(self.job_for(path, self.disk));
        }
        let md = markdown_path(&path);
        self.path = Some(md.clone());
        self.disk = None;
        // Markdown files are UTF-8, whatever the original was.
        self.encoding = TextEncoding::Utf8 { bom: false };
        let mut job = self.job_for(md, None);
        job.fresh = true;
        Some(job)
    }

    /// Makes `path` the document's file (Save as) and returns the save
    /// that writes it there.
    pub fn save_job_as(&mut self, path: &Path) -> SaveJob {
        if self.path.as_deref() != Some(path) {
            self.path = Some(path.to_path_buf());
            self.disk = None;
        }
        self.job_for(path.to_path_buf(), self.disk)
    }

    fn job_for(&self, path: PathBuf, expected: Option<Stamp>) -> SaveJob {
        let with_endings = if self.line_ending == LineEnding::Lf {
            self.text.clone()
        } else {
            self.text.replace('\n', self.line_ending.as_str())
        };
        let (bytes, outcome) = encode(&with_endings, self.encoding);
        SaveJob {
            path,
            bytes,
            outcome,
            revision: self.revision,
            expected,
            fresh: false,
        }
    }

    /// Takes in a finished save. The text counts as saved only if it hasn't
    /// changed since.
    pub fn saved(&mut self, saved: &Saved) {
        if self.path.as_deref() == Some(saved.requested.as_path()) {
            // A fresh file may have had to take a free name.
            self.path = Some(saved.path.clone());
        }
        if self.path.as_deref() != Some(saved.path.as_path()) {
            return;
        }
        self.disk = saved.stamp.or(self.disk);
        if saved.outcome == SaveOutcome::SavedAsUtf8 {
            self.encoding = TextEncoding::Utf8 { bom: false };
        }
        if saved.revision == self.revision {
            self.dirty = false;
        }
    }

    pub fn word_count(&self) -> usize {
        count_words(&self.text)
    }

    /// Saves to the current path right away (blocking), as Markdown (see
    /// [`Self::save_job`]). Fails if the document has never been saved.
    pub fn save(&mut self) -> anyhow::Result<SaveOutcome> {
        let Some(job) = self.save_job() else {
            bail!("document has no file name yet");
        };
        let saved = job.run()?;
        self.saved(&saved);
        Ok(saved.outcome)
    }

    /// Saves to `path` right away (blocking) and makes it the document's
    /// path. The app saves through `crate::saver` instead.
    pub fn save_as(&mut self, path: &Path) -> anyhow::Result<SaveOutcome> {
        let saved = self.save_job_as(path).run()?;
        self.saved(&saved);
        Ok(saved.outcome)
    }
}

/// `path` if nothing is there yet, else the first free `Name (2).md`,
/// `Name (3).md`, …
fn free_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map_or_else(|| "Untitled".into(), |s| s.to_string_lossy());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..u32::MAX)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|candidate| !candidate.exists())
        .expect("some name is free")
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

/// Writes a document's file. On a FUSE mount (rclone, google-drive-ocamlfuse
/// and the like, which sync to the cloud) the file is rewritten in place:
/// the sync tool then updates the one cloud file, keeping its version
/// history, rather than uploading a brand-new file for every save, and the
/// rename that such mounts can hold until an upload finishes is avoided.
/// Elsewhere it is replaced atomically. Unsaved text is always in the local
/// recovery cache too, so an interrupted in-place write loses nothing.
pub fn write_document(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if on_fuse_mount(path) {
        write_in_place(path, bytes)
    } else {
        write_atomically(path, bytes)
    }
}

/// Rewrites `path` with `bytes`: written over the old contents, then cut to
/// length, so the file is never empty in between.
pub fn write_in_place(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.set_len(bytes.len() as u64))
        .and_then(|()| file.sync_all())
        .with_context(|| format!("writing {}", path.display()))
}

/// Whether `path` is on a FUSE file system (Linux only; elsewhere `false`).
fn on_fuse_mount(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        let dir = path.parent().unwrap_or(path);
        let Ok(dir) = dir.canonicalize() else {
            return false;
        };
        fs::read_to_string("/proc/self/mountinfo")
            .is_ok_and(|mounts| mount_type(&mounts, &dir).is_some_and(|t| t.starts_with("fuse")))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

/// The file system type of the mount holding `dir`, from the contents of
/// `/proc/self/mountinfo` (the longest mount point that contains it).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn mount_type<'a>(mountinfo: &'a str, dir: &Path) -> Option<&'a str> {
    let unescape = |s: &str| {
        s.replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\134", "\\")
    };
    mountinfo
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let mount_point = PathBuf::from(unescape(fields.get(4)?));
            let dash = fields.iter().position(|f| *f == "-")?;
            let fs_type = *fields.get(dash + 1)?;
            dir.starts_with(&mount_point)
                .then(|| (mount_point.as_os_str().len(), fs_type))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, fs_type)| fs_type)
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
        assert!(doc.disk_probe().unwrap().run().is_none());

        // Our own saves are not changes from elsewhere.
        doc.text = "ours".into();
        doc.save().unwrap();
        assert!(doc.disk_probe().unwrap().run().is_none());

        fs::write(&path, "theirs, from another device").unwrap();
        let found = doc.disk_probe().unwrap().run().expect("change noticed");
        let theirs = doc
            .take_disk_version(found)
            .expect("no unsaved edits: reload");
        assert_eq!(theirs.text, "theirs, from another device");
        doc = theirs;
        assert!(doc.disk_probe().unwrap().run().is_none(), "reported once");

        // Rewriting the same bytes (as sync tools do) is not a change.
        fs::write(&path, "theirs, from another device").unwrap();
        if let Some(found) = doc.disk_probe().unwrap().run() {
            assert!(doc.take_disk_version(found).is_none());
        }
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
    fn fuse_mounts_are_recognised_from_the_mount_table() {
        let table = "\
22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
98 22 0:52 / /home/me/GoogleDrive rw,nosuid shared:50 - fuse.rclone gdrive: rw,user_id=1000
99 22 0:53 / /home/me/My\\040Box rw - fuse.sshfs host: rw
";
        let at = |p: &str| mount_type(table, Path::new(p));
        assert_eq!(
            at("/home/me/GoogleDrive/Writing/Samadhi"),
            Some("fuse.rclone")
        );
        assert_eq!(at("/home/me/My Box/notes"), Some("fuse.sshfs"));
        assert_eq!(at("/home/me/Documents"), Some("ext4"));
        assert_eq!(
            at("/home/me/GoogleDriveOld"),
            Some("ext4"),
            "whole components only"
        );
    }

    #[test]
    fn writing_in_place_replaces_the_contents_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("in-place.md");
        write_in_place(&path, b"a long first version").unwrap();
        write_in_place(&path, b"short").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"short");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temporary files"
        );
        // Local disks keep the atomic rename.
        write_document(&path, b"atomic").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"atomic");
    }

    #[test]
    fn everything_is_saved_as_markdown() {
        let md = |p: &str| markdown_path(Path::new(p));
        assert_eq!(md("/n/Essay.md"), Path::new("/n/Essay.md"));
        assert_eq!(md("/n/Essay.markdown"), Path::new("/n/Essay.markdown"));
        assert_eq!(md("/n/Essay.omm"), Path::new("/n/Essay.md"));
        assert_eq!(md("/n/Essay.OMM"), Path::new("/n/Essay.md"));
        assert_eq!(md("/n/notes.txt"), Path::new("/n/notes.md"));
        assert_eq!(md("/n/Untitled"), Path::new("/n/Untitled.md"));
        assert_eq!(
            md("/n/Chapter 1. Intro"),
            Path::new("/n/Chapter 1. Intro.md")
        );
    }

    #[test]
    fn editing_an_omm_saves_a_markdown_file_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let omm = dir.path().join("Essay.omm");
        fs::write(&omm, "written in OmmWriter\r\n").unwrap();
        let mut doc = Document::open(&omm).unwrap();
        doc.text.push_str("and more in Samādhi");
        doc.mark_dirty();
        doc.save().unwrap();
        let md = dir.path().join("Essay.md");
        assert_eq!(doc.path(), Some(md.as_path()));
        assert_eq!(
            fs::read_to_string(&md).unwrap(),
            "written in OmmWriter\r\nand more in Samādhi"
        );
        assert_eq!(
            fs::read_to_string(&omm).unwrap(),
            "written in OmmWriter\r\n",
            "the original is left as it was"
        );
        // Later saves go to the .md.
        doc.text.push('!');
        doc.mark_dirty();
        doc.save().unwrap();
        assert!(fs::read_to_string(&md).unwrap().ends_with('!'));
    }

    #[test]
    fn an_existing_markdown_file_is_never_replaced_by_a_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let omm = dir.path().join("Essay.omm");
        fs::write(&omm, "old").unwrap();
        fs::write(dir.path().join("Essay.md"), "a different essay").unwrap();
        let mut doc = Document::open(&omm).unwrap();
        doc.text.push_str(", edited");
        doc.mark_dirty();
        doc.save().unwrap();
        let taken = dir.path().join("Essay (2).md");
        assert_eq!(doc.path(), Some(taken.as_path()));
        assert_eq!(fs::read_to_string(taken).unwrap(), "old, edited");
        assert_eq!(
            fs::read_to_string(dir.path().join("Essay.md")).unwrap(),
            "a different essay"
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
