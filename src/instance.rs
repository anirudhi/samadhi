//! One running copy. The first launch listens for later ones; a later
//! launch (clicking the icon again, or opening a file from the file
//! manager) hands its request to the running app, which comes to the
//! front, and then exits.
//!
//! On Linux and macOS the two talk over a per-user Unix socket. On Windows
//! the running copy listens on a loopback TCP port, written with a random
//! token to a small file in the user's temporary folder; a later launch
//! must present the token, so only Samādhi itself can hand over requests.

use std::io::{self, Read as _, Write as _};
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
            Self::Open(path) => [OPEN, &path_bytes(path)].concat(),
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes == SHOW {
            return Some(Self::Show);
        }
        bytes.strip_prefix(OPEN).map(|p| Self::Open(bytes_path(p)))
    }
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn bytes_path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(not(unix))]
fn bytes_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// The outcome of starting up.
#[derive(Debug)]
pub enum Claim {
    /// No other copy is running: this one is the app.
    First(Instance),
    /// Another copy is running and has been given the request.
    HandedOver,
}

/// The running copy's end of the connection.
#[derive(Debug)]
pub struct Instance {
    listener: transport::Listener,
    guard: Guard,
}

/// Removes the socket (or port file) when the app exits.
#[derive(Debug)]
pub struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Where the socket (or port file) lives: the per-user runtime folder on
/// Linux, the per-user temporary folder elsewhere.
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
    // A couple of tries: one left behind by a crash is removed and retried.
    for _ in 0..3 {
        match transport::bind(socket) {
            Ok(listener) => {
                return Ok(Claim::First(Instance {
                    listener,
                    guard: Guard(socket.to_path_buf()),
                }));
            }
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                if let Ok(mut stream) = transport::connect(socket) {
                    stream.write_all(&message.encode())?;
                    return Ok(Claim::HandedOver);
                }
                // Nobody is listening: left behind by a crash.
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
                for mut stream in transport::incoming(&listener) {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut bytes = Vec::new();
                    if stream.read_to_end(&mut bytes).is_ok()
                        && let Some(message) =
                            transport::accept(&listener, &bytes).and_then(Message::decode)
                    {
                        on_message(message);
                    }
                }
            })
            .ok();
        guard
    }
}

#[cfg(unix)]
mod transport {
    use std::io;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;

    pub type Listener = UnixListener;

    /// Fails with `AddrInUse` if the socket exists.
    pub fn bind(path: &Path) -> io::Result<Listener> {
        UnixListener::bind(path)
    }

    pub fn connect(path: &Path) -> io::Result<UnixStream> {
        UnixStream::connect(path)
    }

    pub fn incoming(listener: &Listener) -> impl Iterator<Item = UnixStream> + '_ {
        listener.incoming().flatten()
    }

    /// The request in what was received (only this user can connect).
    #[allow(clippy::unnecessary_wraps)] // same shape as the Windows version
    pub fn accept<'a>(_: &Listener, bytes: &'a [u8]) -> Option<&'a [u8]> {
        Some(bytes)
    }
}

#[cfg(not(unix))]
mod transport {
    use std::fs;
    use std::hash::{BuildHasher as _, Hasher as _};
    use std::io::{self, Write as _};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};
    use std::path::Path;

    #[derive(Debug)]
    pub struct Listener {
        inner: TcpListener,
        token: String,
    }

    /// A fresh random token (std's hasher keys are random per process).
    fn token() -> String {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        hasher.write_u32(std::process::id());
        format!("{:016x}", hasher.finish())
    }

    /// Listens on a loopback port and records it, with a token, at `path`.
    /// Fails with `AddrInUse` if the file exists.
    pub fn bind(path: &Path) -> io::Result<Listener> {
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                return Err(io::ErrorKind::AddrInUse.into());
            }
            Err(err) => return Err(err),
        };
        let inner = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let token = token();
        writeln!(file, "{} {token}", inner.local_addr()?.port())?;
        Ok(Listener { inner, token })
    }

    /// Connects to the port recorded at `path`, presenting its token.
    pub fn connect(path: &Path) -> io::Result<TcpStream> {
        let record = fs::read_to_string(path)?;
        let mut parts = record.split_whitespace();
        let port: u16 = parts
            .next()
            .and_then(|p| p.parse().ok())
            .ok_or_else(|| io::Error::other("bad port file"))?;
        let token = parts
            .next()
            .ok_or_else(|| io::Error::other("bad port file"))?;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
        writeln!(stream, "{token}")?;
        Ok(stream)
    }

    pub fn incoming(listener: &Listener) -> impl Iterator<Item = TcpStream> + '_ {
        listener.inner.incoming().flatten()
    }

    /// The request in what was received, if it carries the right token.
    pub fn accept<'a>(listener: &Listener, bytes: &'a [u8]) -> Option<&'a [u8]> {
        let newline = bytes.iter().position(|&b| b == b'\n')?;
        let (token, rest) = bytes.split_at(newline);
        (token == listener.token.as_bytes()).then(|| &rest[1..])
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

        let file = dir.path().join("a thought.md");
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
    fn one_left_behind_by_a_crash_is_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("app.sock");
        // Claimed and then abandoned without cleanup, as after a crash.
        let Claim::First(crashed) = claim(&socket, &Message::Show).unwrap() else {
            panic!("nothing else is running");
        };
        std::mem::forget(crashed.guard);
        drop(crashed.listener);
        assert!(socket.exists());
        assert!(matches!(
            claim(&socket, &Message::Show).unwrap(),
            Claim::First(_)
        ));
    }

    #[test]
    fn messages_survive_encoding() {
        for message in [
            Message::Show,
            Message::Open(PathBuf::from("notes/Ünïcödé file.md")),
        ] {
            assert_eq!(Message::decode(&message.encode()), Some(message));
        }
        assert_eq!(Message::decode(b"nonsense"), None);
    }
}
