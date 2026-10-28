//! The focus timer under arbitrary sequences of operations.

use std::time::{Duration, Instant};

use proptest::prelude::*;
use samadhi::timer::{MAX_MINUTES, MIN_MINUTES, Timer, clock};

use crate::config;

#[derive(Debug, Clone, Copy)]
enum Op {
    Toggle,
    Start,
    Reset,
    SetMinutes(u32),
    Poll,
    Wait(Duration),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Toggle),
        1 => Just(Op::Start),
        1 => Just(Op::Reset),
        1 => any::<u32>().prop_map(Op::SetMinutes),
        3 => Just(Op::Poll),
        4 => prop_oneof![
            0u64..2_000,
            0u64..120_000,
            0u64..4_000_000,
        ]
        .prop_map(|ms| Op::Wait(Duration::from_millis(ms))),
    ]
}

fn check_clock(text: &str) -> Result<(), TestCaseError> {
    let (m, s) = text.split_once(':').expect("m:ss");
    prop_assert!(
        !m.is_empty() && m.chars().all(|c| c.is_ascii_digit()),
        "{}",
        text
    );
    prop_assert!(
        s.len() == 2 && s.chars().all(|c| c.is_ascii_digit()),
        "{}",
        text
    );
    prop_assert!(s < "60", "{}", text);
    Ok(())
}

proptest! {
    #![proptest_config(config(256))]

    #[test]
    fn operations_keep_the_timer_consistent(
        minutes in any::<u32>(),
        ops in prop::collection::vec(op(), 0..60),
    ) {
        let mut now = Instant::now();
        let mut timer = Timer::new(minutes);
        // Whether the current countdown has already reported finishing.
        let mut finished = false;
        for op in ops {
            match op {
                Op::Toggle => {
                    if !timer.is_active() {
                        finished = false;
                    }
                    timer.toggle(now);
                }
                Op::Start => {
                    finished = false;
                    timer.start(now);
                    prop_assert!(timer.is_running());
                }
                Op::Reset => {
                    timer.reset();
                    prop_assert!(!timer.is_active());
                }
                Op::SetMinutes(m) => {
                    let was_running = timer.is_running();
                    timer.set_minutes(m, now);
                    prop_assert_eq!(timer.is_running(), was_running);
                    if timer.is_active() {
                        let full = Duration::from_secs(u64::from(timer.minutes()) * 60);
                        prop_assert_eq!(timer.remaining(now), full);
                    }
                }
                Op::Poll => {
                    let due = timer.is_running() && timer.remaining(now).is_zero();
                    let done = timer.poll(now);
                    prop_assert_eq!(done, due);
                    if done {
                        prop_assert!(!finished, "finished twice in one run");
                        finished = true;
                        prop_assert!(!timer.is_active());
                    }
                }
                Op::Wait(d) => now += d,
            }
            let minutes = timer.minutes();
            prop_assert!((MIN_MINUTES..=MAX_MINUTES).contains(&minutes));
            let length = Duration::from_secs(u64::from(minutes) * 60);
            let left = timer.remaining(now);
            prop_assert!(left <= length);
            let fraction = timer.fraction_left(now);
            prop_assert!((0.0..=1.0).contains(&fraction), "{}", fraction);
            check_clock(&clock(left))?;
            match timer.next_tick(now) {
                Some(tick) => {
                    prop_assert!(timer.is_running());
                    prop_assert!(tick > Duration::ZERO && tick <= Duration::from_secs(1));
                }
                None => prop_assert!(!timer.is_running()),
            }
        }
    }

    #[test]
    fn clock_is_minutes_and_seconds(secs in 0u64..10_000_000_000, nanos in 0u32..1_000_000_000) {
        let d = Duration::new(secs, nanos);
        let text = clock(d);
        check_clock(&text)?;
        let (m, s) = text.split_once(':').unwrap();
        let shown = m.parse::<u64>().unwrap() * 60 + s.parse::<u64>().unwrap();
        prop_assert_eq!(shown, secs + u64::from(nanos > 0));
    }
}
