//! One running copy. The first launch listens on a local socket; a later
//! launch (clicking the icon again, or opening a file from the file
//! manager) hands its request to the running app, which comes to the
//! front, and then exits.

use std::ffi::OsStr;
use std::io::{self, Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const OPEN: &[u8] = b"open ";
const SHOW: &[u8] = b"show";

/// A request from a later launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Come to the front.
    Show,
    /// Come to the front and open this file.
    Open(PathBuf),
}

impl Message {
    fn encode(&self) -> Vec<u8> {
        match self {
            Self::Show => SHOW.to_vec(),
            Self::Open(path) => [OPEN, path.as_os_str().as_bytes()].concat(),
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes == SHOW {
            return Some(Self::Show);
        }
        let path = bytes.strip_prefix(OPEN)?;
        Some(Self::Open(PathBuf::from(OsStr::from_bytes(path))))
    }
}

/// The outcome of starting up.
#[derive(Debug)]
pub enum Claim {
    /// No other copy is running: this one is the app.
    First(Instance),
    /// Another copy is running and has been given the request.
    HandedOver,
}

/// The running copy's end of the socket.
#[derive(Debug)]
pub struct Instance {
    listener: UnixListener,
    guard: Guard,
}

/// Removes the socket when the app exits.
#[derive(Debug)]
pub struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Where the socket lives: the per-user runtime folder on Linux, the
/// per-user temporary folder on macOS.
pub fn default_socket() -> PathBuf {
    let dir = crate::project_dirs()
        .and_then(|d| d.runtime_dir().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir);
    dir.join(format!("{}.sock", crate::APP_ID))
}

/// Becomes the running copy, or hands `message` to the one already running.
pub fn claim(socket: &Path, message: &Message) -> io::Result<Claim> {
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // A couple of tries: a socket left by a crash is removed and retried.
    for _ in 0..3 {
        match UnixListener::bind(socket) {
            Ok(listener) => {
                return Ok(Claim::First(Instance {
                    listener,
                    guard: Guard(socket.to_path_buf()),
                }));
            }
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                if let Ok(mut stream) = UnixStream::connect(socket) {
                    stream.write_all(&message.encode())?;
                    return Ok(Claim::HandedOver);
                }
                // Nobody is listening: a stale socket from a crash.
                let _ = std::fs::remove_file(socket);
            }
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::other("could not claim the app socket"))
}

impl Instance {
    /// Listens for later launches on a background thread, passing each
    /// request to `on_message`. Keep the returned guard for as long as the
    /// app runs.
    pub fn serve(self, on_message: impl Fn(Message) + Send + 'static) -> Guard {
        let Self { listener, guard } = self;
        std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                for mut stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut bytes = Vec::new();
                    if stream.read_to_end(&mut bytes).is_ok()
                        && let Some(message) = Message::decode(&bytes)
                    {
                        on_message(message);
                    }
                }
            })
            .ok();
        guard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_second_launch_hands_its_file_to_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("app.sock");
        let Claim::First(first) = claim(&socket, &Message::Show).unwrap() else {
            panic!("nothing else is running");
        };
        let (tx, rx) = mpsc::channel();
        let guard = first.serve(move |m| tx.send(m).unwrap());

        let file = PathBuf::from("/notes/a thought.md");
        assert!(matches!(
            claim(&socket, &Message::Open(file.clone())).unwrap(),
            Claim::HandedOver
        ));
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, Message::Open(file));

        assert!(matches!(
            claim(&socket, &Message::Show).unwrap(),
            Claim::HandedOver
        ));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Message::Show
        );

        drop(guard);
        assert!(!socket.exists(), "socket removed on exit");
    }

    #[test]
    fn a_socket_left_by_a_crash_is_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("app.sock");
        // Bound and then abandoned without cleanup, as after a crash.
        let crashed = UnixListener::bind(&socket).unwrap();
        drop(crashed);
        assert!(socket.exists());
        assert!(matches!(
            claim(&socket, &Message::Show).unwrap(),
            Claim::First(_)
        ));
    }
}
