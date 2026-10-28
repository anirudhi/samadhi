//! A watchdog for freezes. The interface marks where each frame is
//! (`Phase`) and when it ends; a background thread notices when a frame
//! runs long, or when no frame comes although one was due, and logs where
//! the time went. The log (see `main.rs`) is how a freeze that can't be
//! reproduced on demand gets diagnosed.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a frame may take before it is logged as a stall.
pub const SLOW_FRAME: Duration = Duration::from_millis(500);
const CHECK_EVERY: Duration = Duration::from_millis(100);

/// Where the interface thread is within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    /// Between frames: in eframe, the window system, the GPU or the
    /// clipboard (not our code).
    Outside,
    Background,
    Input,
    Scene,
    Editor,
    Controls,
    Saving,
    Dialogs,
}

impl Phase {
    const ALL: [Self; 8] = [
        Self::Outside,
        Self::Background,
        Self::Input,
        Self::Scene,
        Self::Editor,
        Self::Controls,
        Self::Saving,
        Self::Dialogs,
    ];

    fn describe(self) -> &'static str {
        match self {
            Self::Outside => "outside Samādhi (window system, GPU or clipboard)",
            Self::Background => "collecting background work",
            Self::Input => "handling keys and requests",
            Self::Scene => "painting the scene",
            Self::Editor => "laying out the text",
            Self::Controls => "drawing the controls",
            Self::Saving => "saving",
            Self::Dialogs => "opening a file dialog",
        }
    }
}

#[derive(Debug)]
struct Shared {
    origin: Instant,
    /// Milliseconds since `origin` when the current frame began (0: none).
    frame_started: AtomicU64,
    /// When the next frame is due by at the latest (0: not due, idle).
    due_by: AtomicU64,
    phase: AtomicU8,
}

impl Shared {
    fn now(&self) -> u64 {
        // +1 so a timestamp is never 0, which means "none".
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX) + 1
    }

    fn phase(&self) -> Phase {
        let i = usize::from(self.phase.load(Ordering::Relaxed));
        Phase::ALL.get(i).copied().unwrap_or(Phase::Outside)
    }
}

/// The interface thread's handle. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Watchdog(Arc<Shared>);

/// A stall the watchdog noticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stall {
    pub phase: Phase,
    pub for_at_least: Duration,
}

impl Stall {
    pub fn describe(&self) -> String {
        format!(
            "the interface has been stuck for {:.1} s {}",
            self.for_at_least.as_secs_f32(),
            self.phase.describe()
        )
    }
}

impl Watchdog {
    /// A watchdog with no thread, checked by calling [`Self::check`].
    pub fn new() -> Self {
        Self(Arc::new(Shared {
            origin: Instant::now(),
            frame_started: AtomicU64::new(0),
            due_by: AtomicU64::new(0),
            phase: AtomicU8::new(Phase::Outside as u8),
        }))
    }

    /// Starts the background thread, which logs each stall once.
    pub fn spawn() -> Self {
        let watchdog = Self::new();
        let watcher = watchdog.clone();
        std::thread::Builder::new()
            .name("watchdog".into())
            .spawn(move || {
                let mut reported: Option<Phase> = None;
                loop {
                    std::thread::sleep(CHECK_EVERY);
                    match watcher.check() {
                        Some(stall) if reported != Some(stall.phase) => {
                            log::warn!("{}", stall.describe());
                            reported = Some(stall.phase);
                        }
                        Some(_) => {}
                        None => {
                            if reported.take().is_some() {
                                log::warn!("the interface is responding again");
                            }
                        }
                    }
                }
            })
            .ok();
        watchdog
    }

    /// Marks the start of a frame.
    pub fn begin_frame(&self) {
        self.0.frame_started.store(self.0.now(), Ordering::Relaxed);
        self.0.due_by.store(0, Ordering::Relaxed);
        self.enter(Phase::Background);
    }

    /// Marks where the frame is now.
    pub fn enter(&self, phase: Phase) {
        self.0.phase.store(phase as u8, Ordering::Relaxed);
    }

    /// Marks the end of a frame. `next_within` is how soon the next one
    /// must come (a repaint is due), or `None` when the app may idle.
    pub fn end_frame(&self, next_within: Option<Duration>) {
        let due = next_within.map_or(0, |d| {
            self.0.now() + u64::try_from(d.as_millis()).unwrap_or(u64::MAX / 2)
        });
        self.0.due_by.store(due, Ordering::Relaxed);
        self.0.frame_started.store(0, Ordering::Relaxed);
        self.enter(Phase::Outside);
    }

    /// The current stall, if any: a frame running longer than
    /// [`SLOW_FRAME`], or a due frame that is [`SLOW_FRAME`] late.
    pub fn check(&self) -> Option<Stall> {
        let now = self.0.now();
        let started = self.0.frame_started.load(Ordering::Relaxed);
        let late = if started == 0 {
            let due = self.0.due_by.load(Ordering::Relaxed);
            (due != 0).then(|| now.saturating_sub(due))?
        } else {
            now.saturating_sub(started)
        };
        let late = Duration::from_millis(late);
        (late >= SLOW_FRAME).then(|| Stall {
            phase: self.0.phase(),
            for_at_least: late,
        })
    }
}

impl Default for Watchdog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: Duration = Duration::from_millis(600);

    #[test]
    fn quick_frames_and_idling_are_not_stalls() {
        let w = Watchdog::new();
        w.begin_frame();
        w.enter(Phase::Editor);
        w.end_frame(None);
        std::thread::sleep(LONG);
        assert_eq!(w.check(), None, "idle with nothing due is fine");
    }

    #[test]
    fn a_long_frame_is_reported_with_its_phase() {
        let w = Watchdog::new();
        w.begin_frame();
        w.enter(Phase::Saving);
        std::thread::sleep(LONG);
        let stall = w.check().expect("stalled");
        assert_eq!(stall.phase, Phase::Saving);
        assert!(stall.describe().contains("saving"));
        w.end_frame(None);
        assert_eq!(w.check(), None);
    }

    #[test]
    fn a_missing_frame_points_outside_our_code() {
        let w = Watchdog::new();
        w.begin_frame();
        w.end_frame(Some(Duration::from_millis(10)));
        assert_eq!(w.check(), None);
        std::thread::sleep(LONG);
        let stall = w.check().expect("frame overdue");
        assert_eq!(stall.phase, Phase::Outside);
        assert!(stall.describe().contains("outside Samādhi"));
    }
}
