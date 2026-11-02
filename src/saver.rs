//! Saving off the interface thread.
//!
//! Writing a file can stall for a long time on a synced or network drive:
//! an rclone mount, for one, holds a rename over a file until that file's
//! upload has finished, which took up to a minute in practice. So saves
//! happen on a writer thread, and the interface never waits for a disk.
//!
//! The writer is also gentle with sync tools. Saves of the same file are
//! coalesced (only the newest text is written) and spaced at least
//! [`MIN_INTERVAL`] apart, unless one is urgent (Ctrl+S, switching
//! documents, quitting). Saving every pause in typing would otherwise start
//! a new upload every few seconds; uploads then overlap, fail their
//! checksums when the file changes under them, and pile up retries. Nothing
//! is at risk meanwhile: unsaved text is also kept in the local recovery
//! cache, which is written straight away.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::document::{SaveJob, Saved};

/// The least time between two writes of the same file, unless urgent.
pub const MIN_INTERVAL: Duration = Duration::from_secs(15);

enum Request {
    Save {
        job: SaveJob,
        urgent: bool,
    },
    /// Answers once everything queued before it is written (or failed).
    Flush(Sender<()>),
}

/// The result of one save.
#[derive(Debug)]
pub struct Report {
    pub path: PathBuf,
    pub revision: u64,
    pub result: Result<Saved, String>,
}

/// The interface thread's handle on the writer.
#[derive(Debug)]
pub struct Saver {
    tx: Sender<Request>,
    rx: Receiver<Report>,
    /// Saves queued and not yet reported.
    outstanding: usize,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Save { job, urgent } => f
                .debug_struct("Save")
                .field("path", &job.path)
                .field("urgent", urgent)
                .finish(),
            Self::Flush(_) => f.write_str("Flush"),
        }
    }
}

impl Saver {
    /// Starts the writer thread. `wake` is called after each save, so the
    /// interface can collect the report.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Self {
        Self::with_interval(MIN_INTERVAL, wake)
    }

    pub fn with_interval(interval: Duration, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, requests) = mpsc::channel();
        let (reports, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("saver".into())
            .spawn(move || Writer::new(interval, reports, wake).run(&requests))
            .expect("spawning a thread should not fail");
        Self {
            tx,
            rx,
            outstanding: 0,
        }
    }

    /// Queues a save. Never blocks.
    pub fn save(&mut self, job: SaveJob, urgent: bool) {
        if self.tx.send(Request::Save { job, urgent }).is_ok() {
            self.outstanding += 1;
        }
    }

    /// Finished saves, oldest first. Never blocks.
    pub fn reports(&mut self) -> Vec<Report> {
        let reports: Vec<Report> = self.rx.try_iter().collect();
        self.outstanding = self.outstanding.saturating_sub(reports.len());
        reports
    }

    /// Whether any save is queued or being written.
    pub fn busy(&self) -> bool {
        self.outstanding > 0
    }

    /// Writes everything queued now, waiting at most `timeout`. Returns
    /// whether it all finished. Used when quitting: if the disk is stuck,
    /// the app still closes, and the text is in the recovery cache.
    pub fn flush(&self, timeout: Duration) -> bool {
        let (done, wait) = mpsc::channel();
        if self.tx.send(Request::Flush(done)).is_err() {
            return true;
        }
        wait.recv_timeout(timeout).is_ok()
    }
}

/// The writer thread's state.
struct Writer<W> {
    interval: Duration,
    reports: Sender<Report>,
    wake: W,
    /// The newest save waiting for each file, and whether it is urgent.
    waiting: HashMap<PathBuf, (SaveJob, bool)>,
    last_write: HashMap<PathBuf, Instant>,
    /// Fresh files that had to take a free name: later saves aimed at the
    /// name asked for go to the one used.
    redirects: HashMap<PathBuf, PathBuf>,
}

impl<W: Fn()> Writer<W> {
    fn new(interval: Duration, reports: Sender<Report>, wake: W) -> Self {
        Self {
            interval,
            reports,
            wake,
            waiting: HashMap::new(),
            last_write: HashMap::new(),
            redirects: HashMap::new(),
        }
    }

    fn run(mut self, requests: &Receiver<Request>) {
        loop {
            let request = match self.next_due() {
                // Something is waiting: sleep until it is due, unless a new
                // request arrives first.
                Some(wait) => match requests.recv_timeout(wait) {
                    Ok(request) => Some(request),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.write_all();
                        return;
                    }
                },
                None => match requests.recv() {
                    Ok(request) => Some(request),
                    Err(_) => return,
                },
            };
            match request {
                Some(Request::Save { job, urgent }) => self.queue(job, urgent),
                Some(Request::Flush(done)) => {
                    // Take in everything already sent, then write it all.
                    while let Ok(request) = requests.try_recv() {
                        match request {
                            Request::Save { job, urgent } => self.queue(job, urgent),
                            Request::Flush(other) => {
                                let _ = other.send(());
                            }
                        }
                    }
                    self.write_all();
                    let _ = done.send(());
                }
                None => {}
            }
            self.write_due();
        }
    }

    /// Keeps only the newest save for each file; a superseded one is
    /// reported as done by the newer one, so the interface's count of
    /// outstanding saves stays right.
    fn queue(&mut self, mut job: SaveJob, urgent: bool) {
        if let Some(actual) = self.redirects.get(&job.path) {
            job.path.clone_from(actual);
            job.fresh = false;
        }
        let path = job.path.clone();
        let earlier = self.waiting.get(&path);
        let urgent = urgent || earlier.is_some_and(|(_, u)| *u);
        // A merged save keeps "never replace an existing file" if any of
        // its parts had it.
        job.fresh |= earlier.is_some_and(|(j, _)| j.fresh);
        if let Some((old, _)) = self.waiting.insert(path.clone(), (job, urgent)) {
            let _ = self.reports.send(Report {
                path,
                revision: old.revision,
                result: Err(SUPERSEDED.into()),
            });
        }
    }

    /// How long until the next waiting save is due.
    fn next_due(&self) -> Option<Duration> {
        self.waiting
            .iter()
            .map(|(path, (_, urgent))| {
                if *urgent {
                    Duration::ZERO
                } else {
                    self.last_write.get(path).map_or(Duration::ZERO, |t| {
                        self.interval.saturating_sub(t.elapsed())
                    })
                }
            })
            .min()
    }

    fn write_due(&mut self) {
        let due: Vec<PathBuf> = self
            .waiting
            .iter()
            .filter(|(path, (_, urgent))| {
                *urgent
                    || self
                        .last_write
                        .get(*path)
                        .is_none_or(|t| t.elapsed() >= self.interval)
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in due {
            self.write(&path);
        }
    }

    fn write_all(&mut self) {
        let paths: Vec<PathBuf> = self.waiting.keys().cloned().collect();
        for path in paths {
            self.write(&path);
        }
    }

    fn write(&mut self, path: &PathBuf) {
        let Some((job, _)) = self.waiting.remove(path) else {
            return;
        };
        let result = job.run().map_err(|err| format!("{err:#}"));
        self.last_write.insert(path.clone(), Instant::now());
        if let Ok(saved) = &result
            && saved.path != saved.requested
        {
            self.redirects
                .insert(saved.requested.clone(), saved.path.clone());
        }
        let _ = self.reports.send(Report {
            path: path.clone(),
            revision: job.revision,
            result,
        });
        (self.wake)();
    }
}

/// The error reported for a save replaced by a newer one before it was
/// written. Not a failure: the newer save carries the text.
pub const SUPERSEDED: &str = "superseded by a newer save";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use std::path::PathBuf;

    fn doc_at(path: &std::path::Path, text: &str) -> Document {
        let mut doc = Document::default();
        doc.text = text.into();
        doc.save_as(path).unwrap();
        doc
    }

    fn wait_for(saver: &mut Saver, n: usize) -> Vec<Report> {
        let mut got = Vec::new();
        let start = Instant::now();
        while got.len() < n && start.elapsed() < Duration::from_secs(10) {
            got.extend(saver.reports());
            std::thread::sleep(Duration::from_millis(5));
        }
        got
    }

    #[test]
    fn saves_happen_off_thread_and_report_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let mut doc = doc_at(&path, "one");
        doc.text = "two".into();
        doc.mark_dirty();
        let mut saver = Saver::spawn(|| {});
        saver.save(doc.save_job().unwrap(), true);
        assert!(saver.busy());
        let reports = wait_for(&mut saver, 1);
        let written = reports[0].result.as_ref().unwrap();
        doc.saved(written);
        assert!(!doc.is_dirty());
        assert!(!saver.busy());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    }

    #[test]
    fn quick_saves_of_one_file_are_coalesced_and_spaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let mut doc = doc_at(&path, "v0");
        let interval = Duration::from_millis(300);
        let mut saver = Saver::with_interval(interval, || {});
        // The first save is written at once.
        doc.text = "v1".into();
        doc.mark_dirty();
        saver.save(doc.save_job().unwrap(), false);
        let first = wait_for(&mut saver, 1);
        assert!(first[0].result.is_ok());
        let written_at = Instant::now();
        // Ten more in quick succession: nine superseded, one written, and
        // not before the interval has passed.
        for i in 2..=11 {
            doc.text = format!("v{i}");
            doc.mark_dirty();
            saver.save(doc.save_job().unwrap(), false);
        }
        let rest = wait_for(&mut saver, 10);
        let written: Vec<_> = rest.iter().filter(|r| r.result.is_ok()).collect();
        assert_eq!(written.len(), 1, "coalesced into one write");
        assert!(written_at.elapsed() >= interval.saturating_sub(Duration::from_millis(50)));
        assert_eq!(written[0].revision, doc.revision());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v11");
        assert!(!saver.busy());
    }

    #[test]
    fn urgent_saves_and_flush_skip_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let mut doc = doc_at(&path, "v0");
        let mut saver = Saver::with_interval(Duration::from_secs(3600), || {});
        doc.text = "v1".into();
        doc.mark_dirty();
        saver.save(doc.save_job().unwrap(), false);
        wait_for(&mut saver, 1);
        doc.text = "v2".into();
        doc.mark_dirty();
        saver.save(doc.save_job().unwrap(), false);
        // Due in an hour, but flushing writes it now.
        assert!(saver.flush(Duration::from_secs(5)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2");
        doc.text = "v3".into();
        doc.mark_dirty();
        saver.save(doc.save_job().unwrap(), true);
        assert!(wait_for(&mut saver, 2).iter().any(|r| r.result.is_ok()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v3");
    }

    #[test]
    fn a_save_finishing_after_more_typing_leaves_the_text_unsaved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let mut doc = doc_at(&path, "v0");
        doc.text = "v1".into();
        doc.mark_dirty();
        let saved = doc.save_job().unwrap().run().unwrap();
        doc.text = "v1 and more".into();
        doc.mark_dirty();
        doc.saved(&saved);
        assert!(doc.is_dirty(), "the newer text still needs saving");
    }

    #[test]
    fn a_change_made_elsewhere_is_kept_as_a_conflicted_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.md");
        let mut doc = doc_at(&path, "base");
        std::fs::write(&path, "theirs, from the other computer").unwrap();
        doc.text = "ours".into();
        doc.mark_dirty();
        let saved = doc.save_job().unwrap().run().unwrap();
        let copy = saved.conflict_copy.clone().expect("kept theirs");
        assert_eq!(
            std::fs::read_to_string(copy).unwrap(),
            "theirs, from the other computer"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ours");
        // Saving again (no new outside change) makes no second copy.
        doc.saved(&saved);
        doc.text = "ours, later".into();
        doc.mark_dirty();
        assert!(
            doc.save_job()
                .unwrap()
                .run()
                .unwrap()
                .conflict_copy
                .is_none()
        );
    }

    /// A soak test against a real (synced) folder, e.g. an rclone mount:
    /// `SAMADHI_SOAK_DIR=~/GoogleDrive/tmp cargo test --release soak -- --ignored --nocapture`.
    /// Types for a minute, saving at every change as the app would.
    #[test]
    #[ignore = "needs SAMADHI_SOAK_DIR pointing at a real synced folder"]
    fn soak_on_a_real_drive() {
        let Some(dir) = std::env::var_os("SAMADHI_SOAK_DIR").map(PathBuf::from) else {
            return;
        };
        let path = dir.join("samadhi-soak-test.md");
        let mut doc = Document::default();
        doc.text = "soak test\n".into();
        let mut saver = Saver::spawn(|| {});
        saver.save(doc.save_job_as(&path), true);
        let (mut slowest, mut writes, mut failures) = (Duration::ZERO, 0, 0);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(60) {
            doc.text.push_str("word ");
            doc.mark_dirty();
            let t = Instant::now();
            saver.save(doc.save_job().unwrap(), false);
            slowest = slowest.max(t.elapsed());
            for r in saver.reports() {
                match &r.result {
                    Ok(done) => {
                        writes += 1;
                        doc.saved(done);
                    }
                    Err(e) if e == SUPERSEDED => {}
                    Err(e) => {
                        failures += 1;
                        eprintln!("failed: {e}");
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        assert!(saver.flush(Duration::from_secs(120)), "final save stuck");
        for r in saver.reports() {
            if let Ok(done) = &r.result {
                writes += 1;
                doc.saved(done);
            }
        }
        let on_disk = std::fs::read_to_string(&path).unwrap();
        eprintln!(
            "soak: {writes} writes in 60 s, {failures} failures, slowest queueing {slowest:?}"
        );
        assert_eq!(on_disk, doc.text);
        assert!(!doc.is_dirty());
        assert!(
            slowest < Duration::from_millis(5),
            "queueing a save blocked"
        );
        assert_eq!(failures, 0);
    }

    #[test]
    fn saves_after_a_name_was_taken_follow_the_free_name() {
        let dir = tempfile::tempdir().unwrap();
        let omm = dir.path().join("Essay.omm");
        std::fs::write(&omm, "old").unwrap();
        std::fs::write(dir.path().join("Essay.md"), "someone else's").unwrap();
        let mut doc = Document::open(&omm).unwrap();
        let mut saver = Saver::with_interval(Duration::ZERO, || {});
        doc.text = "first".into();
        doc.mark_dirty();
        saver.save(doc.save_job().unwrap(), true);
        // Queued before hearing where the first went: still aimed at Essay.md.
        doc.text = "second".into();
        doc.mark_dirty();
        let second = doc.save_job().unwrap();
        assert_eq!(second.path, dir.path().join("Essay.md"));
        assert!(saver.flush(Duration::from_secs(5)));
        saver.save(second, true);
        for r in wait_for(&mut saver, 2) {
            doc.saved(r.result.as_ref().unwrap());
        }
        let free = dir.path().join("Essay (2).md");
        assert_eq!(doc.path(), Some(free.as_path()));
        assert_eq!(std::fs::read_to_string(&free).unwrap(), "second");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Essay.md")).unwrap(),
            "someone else's"
        );
    }

    #[test]
    fn a_failed_save_is_reported_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-folder");
        std::fs::write(&blocker, "").unwrap();
        let mut doc = Document::default();
        doc.text = "text".into();
        let job = doc.save_job_as(&blocker.join("a.md"));
        let mut saver = Saver::spawn(|| {});
        saver.save(job, true);
        let reports = wait_for(&mut saver, 1);
        assert!(reports[0].result.is_err());
        assert!(!saver.busy());
    }
}
