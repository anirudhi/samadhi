//! A focus timer that counts down a set number of minutes.
//!
//! The clock is passed in (`now`) rather than read, so behaviour is exact
//! and testable.

use std::time::{Duration, Instant};

pub const MIN_MINUTES: u32 = 1;
pub const MAX_MINUTES: u32 = 60;
pub const DEFAULT_MINUTES: u32 = 25;
/// Lengths offered with one click; the slider covers everything else.
pub const PRESETS: [u32; 2] = [5, 25];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Running { ends: Instant },
    Paused { left: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    minutes: u32,
    state: State,
}

impl Timer {
    pub fn new(minutes: u32) -> Self {
        Self {
            minutes: minutes.clamp(MIN_MINUTES, MAX_MINUTES),
            state: State::Idle,
        }
    }

    pub fn minutes(&self) -> u32 {
        self.minutes
    }

    fn length(&self) -> Duration {
        Duration::from_secs(u64::from(self.minutes) * 60)
    }

    /// Changes the length. A running or paused countdown starts over at the
    /// new length, so what you set is what you get.
    pub fn set_minutes(&mut self, minutes: u32, now: Instant) {
        self.minutes = minutes.clamp(MIN_MINUTES, MAX_MINUTES);
        self.state = match self.state {
            State::Idle => State::Idle,
            State::Running { .. } => State::Running {
                ends: now + self.length(),
            },
            State::Paused { .. } => State::Paused {
                left: self.length(),
            },
        };
    }

    /// Starts from the full length, whatever the timer was doing.
    pub fn start(&mut self, now: Instant) {
        self.state = State::Running {
            ends: now + self.length(),
        };
    }

    /// Play/pause: starts, pauses or resumes.
    pub fn toggle(&mut self, now: Instant) {
        self.state = match self.state {
            State::Idle => State::Running {
                ends: now + self.length(),
            },
            State::Running { ends } => State::Paused {
                left: ends.saturating_duration_since(now),
            },
            State::Paused { left } => State::Running { ends: now + left },
        };
    }

    /// Stops and returns to the full length.
    pub fn reset(&mut self) {
        self.state = State::Idle;
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running { .. })
    }

    /// Running or paused: there is a countdown to show.
    pub fn is_active(&self) -> bool {
        self.state != State::Idle
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        match self.state {
            State::Idle => self.length(),
            State::Running { ends } => ends.saturating_duration_since(now),
            State::Paused { left } => left,
        }
    }

    /// The share of the length still to go, from 1 down to 0.
    pub fn fraction_left(&self, now: Instant) -> f32 {
        self.remaining(now).as_secs_f32() / self.length().as_secs_f32()
    }

    /// Ends a countdown that has reached zero. Returns `true` once, at the
    /// moment it finishes.
    pub fn poll(&mut self, now: Instant) -> bool {
        match self.state {
            State::Running { ends } if ends <= now => {
                self.state = State::Idle;
                true
            }
            _ => false,
        }
    }

    /// Time until the shown seconds change, to schedule the next repaint.
    pub fn next_tick(&self, now: Instant) -> Option<Duration> {
        let State::Running { .. } = self.state else {
            return None;
        };
        let left = self.remaining(now);
        let into_second = left.subsec_nanos();
        Some(if into_second == 0 {
            Duration::from_secs(1)
        } else {
            Duration::from_nanos(u64::from(into_second))
        })
    }
}

/// `m:ss`, rounding up so a timer reads `25:00` when started and `0:00` only
/// when done.
pub fn clock(left: Duration) -> String {
    let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn counts_down_pauses_and_finishes_once() {
        let t0 = Instant::now();
        let mut timer = Timer::new(5);
        assert_eq!(clock(timer.remaining(t0)), "5:00");
        assert!(!timer.is_active());

        timer.toggle(t0);
        assert!(timer.is_running());
        assert_eq!(clock(timer.remaining(t0 + 61 * S)), "3:59");

        timer.toggle(t0 + 120 * S); // pause with 3:00 left
        assert_eq!(clock(timer.remaining(t0 + 999 * S)), "3:00");
        assert!(timer.is_active() && !timer.is_running());

        timer.toggle(t0 + 1000 * S); // resume
        assert!(!timer.poll(t0 + 1179 * S));
        assert!(timer.poll(t0 + 1180 * S), "done after the 3 minutes left");
        assert!(!timer.poll(t0 + 1181 * S), "reported once");
        assert!(!timer.is_active());
    }

    #[test]
    fn changing_the_length_starts_over() {
        let t0 = Instant::now();
        let mut timer = Timer::new(25);
        timer.start(t0);
        timer.set_minutes(5, t0 + 600 * S);
        assert_eq!(clock(timer.remaining(t0 + 600 * S)), "5:00");
        assert!((timer.fraction_left(t0 + 750 * S) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn lengths_are_kept_in_range() {
        assert_eq!(Timer::new(0).minutes(), MIN_MINUTES);
        assert_eq!(Timer::new(500).minutes(), MAX_MINUTES);
    }

    #[test]
    fn clock_rounds_up_partial_seconds() {
        assert_eq!(clock(Duration::from_millis(59_001)), "1:00");
        assert_eq!(clock(Duration::from_millis(400)), "0:01");
        assert_eq!(clock(Duration::ZERO), "0:00");
    }
}
