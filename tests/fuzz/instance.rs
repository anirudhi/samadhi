//! Handing requests from a second launch to the running copy.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use samadhi::instance::{Claim, Message, claim};

use crate::config;

/// Any message, with file paths made of arbitrary bytes (Unix paths are
/// bytes, not text), including empty ones and ones that spell `show`.
fn message() -> impl Strategy<Value = Message> {
    prop_oneof![
        1 => Just(Message::Show),
        1 => prop::sample::select(vec!["", "show", "open ", "/", "a\nb", " "])
            .prop_map(|p| Message::Open(PathBuf::from(p))),
        4 => prop::collection::vec(any::<u8>(), 0..300)
            .prop_map(|b| Message::Open(PathBuf::from(OsStr::from_bytes(&b)))),
    ]
}

#[test]
fn every_message_reaches_the_running_copy_intact() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("app.sock");
    let Claim::First(first) = claim(&socket, &Message::Show).unwrap() else {
        panic!("nothing else is running");
    };
    let (tx, rx) = mpsc::channel();
    let guard = first.serve(move |m| tx.send(m).unwrap());

    let mut runner = TestRunner::new(config(64));
    runner
        .run(&message(), |message| {
            let handed = claim(&socket, &message).unwrap();
            prop_assert!(matches!(handed, Claim::HandedOver));
            let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            prop_assert_eq!(got, message);
            Ok(())
        })
        .unwrap();

    drop(guard);
    assert!(!socket.exists());
}
