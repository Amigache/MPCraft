//! File transport.
//!
//! This transport replays a recorded byte stream. It is what makes the whole stack testable
//! without hardware: a test writes a byte stream to a temporary file, points a session at
//! it, and drives the session exactly as it would drive a real link.
//!
//! # Rate control
//!
//! A replay rate of `0.0` replays as fast as the reader can consume, which is what a
//! deterministic test wants. A rate of `1.0` replays at the speed the data was originally
//! captured, assuming a known byte rate; higher rates play faster.
//!
//! # Log timing
//!
//! This transport is byte-oriented: it knows nothing about MAVLink. Replaying a `.tlog`
//! with its *original* timing needs to understand the log's record structure, which belongs
//! to the log format rather than to the transport. That replay lives in `mpcraft-log`, which
//! implements the same [`Transport`] trait.

use std::io::Seek;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

use crate::{Transport, TransportKind};

/// A transport that replays a file.
pub struct FileTransport {
    path: PathBuf,
    file: Option<tokio::fs::File>,
    /// The replay rate; `0.0` means as fast as possible.
    rate: f64,
    /// The number of bytes replayed so far, used to pace the replay.
    replayed: u64,
    /// When the current replay started.
    started: Option<Instant>,
}

impl FileTransport {
    /// Opens a file for replay.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be opened.
    pub async fn open(path: impl AsRef<Path>, rate: f64) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = tokio::fs::File::open(&path).await?;

        Ok(Self {
            path,
            file: Some(file),
            rate: rate.max(0.0),
            replayed: 0,
            started: None,
        })
    }

    /// The path being replayed.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The number of bytes replayed so far.
    #[must_use]
    pub fn replayed(&self) -> u64 {
        self.replayed
    }

    /// Waits until enough time has passed for the bytes replayed so far to match the rate.
    async fn pace(&mut self, bytes: usize) {
        let _ = bytes;

        if self.rate <= 0.0 {
            return;
        }

        self.started.get_or_insert_with(Instant::now);

        let target = Duration::from_secs_f64(self.replayed as f64 / self.rate);
        let elapsed = self.started.map_or(Duration::ZERO, |start| start.elapsed());

        if target > elapsed {
            tokio::time::sleep(target - elapsed).await;
        }
    }
}

#[async_trait]
impl Transport for FileTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(file) = self.file.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "the file is closed",
            ));
        };

        let read = file.read(buf).await?;
        self.replayed += read as u64;

        // A read of zero bytes is a clean end of stream.
        if read == 0 {
            self.file = None;
            return Ok(0);
        }

        self.pace(read).await;

        Ok(read)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // A replay is read-only: anything written to it is discarded, which keeps a session
        // that writes frames during a replay working exactly as it would against hardware.
        //
        // The write is reported as accepted. Reporting zero would look like a stalled
        // transport to the caller, which would turn every write during a replay into an
        // error.
        Ok(buf.len())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::File
    }

    fn is_open(&self) -> bool {
        self.file.is_some()
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        // Reconnecting a replay rewinds it to the start.
        let file = tokio::fs::File::open(&self.path).await?;
        self.file = Some(file);
        self.replayed = 0;
        self.started = None;
        Ok(())
    }

    fn describe(&self) -> String {
        format!("file {}", self.path.display())
    }
}

impl std::fmt::Debug for FileTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileTransport")
            .field("path", &self.path)
            .field("rate", &self.rate)
            .field("replayed", &self.replayed)
            .field("open", &self.file.is_some())
            .finish()
    }
}

/// Rewinds a file to its start.
///
/// Used by the tests and by the replay controls.
pub fn rewind(file: &mut std::fs::File) -> std::io::Result<()> {
    file.seek(std::io::SeekFrom::Start(0))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `contents` to a temporary file and returns its path.
    fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mpcraft-transport-{name}"));
        std::fs::write(&path, contents).expect("write the temporary file");
        path
    }

    #[tokio::test]
    async fn the_file_contents_are_replayed() {
        let path = temp_file("replay.bin", b"hello mavlink");
        let mut transport = FileTransport::open(&path, 0.0).await.unwrap();

        let mut buf = [0u8; 64];
        let len = transport.read(&mut buf).await.unwrap();

        assert_eq!(&buf[..len], b"hello mavlink");
        assert_eq!(transport.replayed(), 13);
    }

    #[tokio::test]
    async fn the_end_of_the_file_is_a_clean_end_of_stream() {
        let path = temp_file("eof.bin", b"abc");
        let mut transport = FileTransport::open(&path, 0.0).await.unwrap();

        let mut buf = [0u8; 64];
        assert_eq!(transport.read(&mut buf).await.unwrap(), 3);

        // Reading past the end returns zero bytes, not an error.
        assert_eq!(transport.read(&mut buf).await.unwrap(), 0);
        assert!(!transport.is_open());
    }

    #[tokio::test]
    async fn reconnecting_rewinds_the_replay() {
        let path = temp_file("rewind.bin", b"abc");
        let mut transport = FileTransport::open(&path, 0.0).await.unwrap();

        let mut buf = [0u8; 64];
        assert_eq!(transport.read(&mut buf).await.unwrap(), 3);
        assert_eq!(transport.read(&mut buf).await.unwrap(), 0);

        transport.reconnect().await.unwrap();

        assert_eq!(transport.read(&mut buf).await.unwrap(), 3);
        assert_eq!(transport.replayed(), 3);
    }

    #[tokio::test]
    async fn writing_to_a_replay_is_accepted_and_discarded() {
        let path = temp_file("write.bin", b"abc");
        let mut transport = FileTransport::open(&path, 0.0).await.unwrap();

        // A session writes frames during a replay; that must not fail.
        transport.write(b"ignored").await.unwrap();
    }

    #[tokio::test]
    async fn opening_a_missing_file_is_an_error() {
        let result = FileTransport::open("this-file-does-not-exist.tlog", 0.0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn a_rate_limited_replay_takes_at_least_the_expected_time() {
        let path = temp_file("rate.bin", &vec![0u8; 1024]);
        let mut transport = FileTransport::open(&path, 1024.0).await.unwrap();

        let start = Instant::now();
        let mut buf = [0u8; 512];
        transport.read(&mut buf).await.unwrap();
        transport.read(&mut buf).await.unwrap();
        let elapsed = start.elapsed();

        // 1024 bytes at 1024 bytes per second takes about a second.
        assert!(
            elapsed >= Duration::from_millis(900),
            "the replay was not paced: {elapsed:?}"
        );
    }
}
