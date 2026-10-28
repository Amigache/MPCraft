//! The logs a companion computer keeps, reached over SSH.
//!
//! A vehicle with a companion computer keeps its logs in the companion's filesystem, which is not
//! the filesystem of the autopilot and is not reached by the file-transfer protocol of the link.
//! The companion answers SSH, so the logs are listed and brought over SFTP.
//!
//! The address is the one the published client asks for: `user:password@host:/dir/path/`, where the
//! directory is the one the companion keeps its logs in. An optional `:port` is read before the
//! path, because a companion that is not on 22 is a companion all the same.
//!
//! Two things are said rather than hidden. The key the companion presents is **accepted**, not
//! compared with one remembered from before: a station that must verify it needs a keyring of its
//! own, which this module does not carry. And only a password is read; a key of the operator's own
//! is not offered, because the published client asks for the same pair.
//!
//! What travels is only the name of a file: a name with a separator, or one that is `.` or `..`, is
//! refused before a local file is created, so a companion cannot name a place outside the folder the
//! operator chose.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use russh::client;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tokio::io::{copy, AsyncWriteExt as _};

use crate::error::{CommandError, CommandResult};

/// How long reaching the companion is waited for before it is said as no answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long one listing or one file is waited for before it is opened.
const OPERATION_TIMEOUT: Duration = Duration::from_secs(60);

/// How long one file is given to arrive, once it is open.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(300);

/// The address of a companion, as the operator writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScpTarget {
    /// The user the companion knows.
    pub user: String,
    /// The password of that user.
    pub password: String,
    /// The machine the companion is on.
    pub host: String,
    /// The port SSH answers on.
    pub port: u16,
    /// The directory the logs are in.
    pub path: String,
}

impl ScpTarget {
    /// Reads `user:password@host[:port]:/dir/path/`.
    ///
    /// # Errors
    ///
    /// Returns an error when a part the address needs is not there. Nothing is guessed: a string
    /// that is missing the user, the password, the host or the directory is refused rather than
    /// filled in with a default that would reach another machine.
    pub fn parse(text: &str) -> CommandResult<Self> {
        let text = text.trim();
        let (userinfo, rest) = text.split_once('@').ok_or_else(|| {
            CommandError::invalid_request(
                "the address needs a user and a password before the @, as user:password@host:/path/"
                    .to_owned(),
            )
        })?;

        let (user, password) = userinfo.split_once(':').ok_or_else(|| {
            CommandError::invalid_request(
                "the user and the password are separated by a colon".to_owned(),
            )
        })?;

        // The path is what follows the colon that comes before its first slash. Splitting from the
        // right keeps a colon in the directory from being read as the one of the port.
        let (host_part, path) = rest.rsplit_once(":/").ok_or_else(|| {
            CommandError::invalid_request(
                "the address needs a host and a directory, as host:/path/".to_owned(),
            )
        })?;

        let (host, port) = match host_part.rsplit_once(':') {
            Some((host, port)) => {
                let port: u16 = port
                    .parse()
                    .map_err(|_| CommandError::invalid_request(format!("{port} is not a port")))?;
                (host, port)
            }
            None => (host_part, 22),
        };

        if user.is_empty() {
            return Err(CommandError::invalid_request(
                "the address names no user".to_owned(),
            ));
        }

        if password.is_empty() {
            return Err(CommandError::invalid_request(
                "the address names no password, and only a password is read".to_owned(),
            ));
        }

        if host.is_empty() {
            return Err(CommandError::invalid_request(
                "the address names no machine".to_owned(),
            ));
        }

        if port == 0 {
            return Err(CommandError::invalid_request(
                "port zero is not a port".to_owned(),
            ));
        }

        Ok(Self {
            user: user.to_owned(),
            password: password.to_owned(),
            host: host.to_owned(),
            port,
            path: format!("/{path}"),
        })
    }
}

/// One log the companion holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLog {
    /// The name of the file, which is what it is called on the companion.
    pub name: String,
    /// How big the companion says it is, in bytes.
    pub size: u64,
}

/// One log that was brought to the disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedLog {
    /// The name of the file, as the companion called it.
    pub name: String,
    /// How many bytes were written, which is what arrived rather than what was announced.
    pub bytes: u64,
}

/// Lists the logs of a companion.
///
/// # Errors
///
/// Returns an error when the companion cannot be reached, refuses the user and password, does not
/// answer the file-transfer subsystem, or does not answer the directory.
pub async fn list_logs(target: &ScpTarget) -> CommandResult<Vec<RemoteLog>> {
    let sftp = connect(target).await?;

    let entries = tokio::time::timeout(OPERATION_TIMEOUT, sftp.read_dir(target.path.clone()))
        .await
        .map_err(|_| {
            CommandError::internal(
                "the companion did not answer the list of the directory".to_owned(),
            )
        })?
        .map_err(|error| {
            CommandError::internal(format!(
                "the directory {} could not be read: {error}",
                target.path
            ))
        })?;

    Ok(select_logs(entries.map(|entry| {
        let size = entry.metadata().size.unwrap_or(0);
        (entry.file_name(), size)
    })))
}

/// Brings the named logs to a folder of this machine.
///
/// # Errors
///
/// Returns an error when a name is not one of the companion's logs, when the companion cannot be
/// reached, when a file cannot be read over the link, or when the local file cannot be written.
pub async fn download_logs(
    target: &ScpTarget,
    names: &[String],
    folder: &Path,
) -> CommandResult<Vec<DownloadedLog>> {
    let wanted = names
        .iter()
        .map(|name| {
            let safe = safe_file_name(name).ok_or_else(|| {
                CommandError::invalid_request(format!(
                    "{name} is not a name this station writes to the disk"
                ))
            })?;

            if !safe.to_ascii_lowercase().ends_with(".bin") {
                return Err(CommandError::invalid_request(format!(
                    "{name} is not a log of the companion"
                )));
            }

            Ok(safe.to_owned())
        })
        .collect::<CommandResult<Vec<_>>>()?;

    let sftp = connect(target).await?;
    let mut done = Vec::with_capacity(wanted.len());

    for name in wanted {
        let remote = join_remote(&target.path, &name);

        let mut source = tokio::time::timeout(OPERATION_TIMEOUT, sftp.open(remote.clone()))
            .await
            .map_err(|_| CommandError::internal(format!("the companion did not answer {remote}")))?
            .map_err(|error| {
                CommandError::internal(format!("{remote} could not be opened: {error}"))
            })?;

        let local_path = folder.join(&name);
        let mut local = tokio::fs::File::create(&local_path)
            .await
            .map_err(|error| {
                CommandError::internal(format!(
                    "{} could not be created: {error}",
                    local_path.display()
                ))
            })?;

        let bytes = tokio::time::timeout(TRANSFER_TIMEOUT, copy(&mut source, &mut local))
            .await
            .map_err(|_| CommandError::internal(format!("{remote} was not finished in time")))?
            .map_err(|error| {
                CommandError::internal(format!("{remote} could not be read: {error}"))
            })?;

        local.flush().await.map_err(|error| {
            CommandError::internal(format!(
                "{} could not be finished: {error}",
                local_path.display()
            ))
        })?;

        done.push(DownloadedLog { name, bytes });
    }

    Ok(done)
}

/// The logs among the names of a directory, in the order the operator reads them.
///
/// The published client keeps the files whose name ends in `.bin`, in either case, and this keeps
/// the same rule. It also drops a name this station will not write to the disk, which the published
/// client would have tried.
#[must_use]
pub fn select_logs(entries: impl IntoIterator<Item = (String, u64)>) -> Vec<RemoteLog> {
    let mut logs: Vec<RemoteLog> = entries
        .into_iter()
        .filter(|(name, _)| name.to_ascii_lowercase().ends_with(".bin"))
        .filter(|(name, _)| safe_file_name(name).is_some())
        .map(|(name, size)| RemoteLog { name, size })
        .collect();

    logs.sort_by(|left, right| left.name.cmp(&right.name));

    logs
}

/// A name this station writes to the disk, or nothing.
fn safe_file_name(name: &str) -> Option<&str> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return None;
    }

    Some(name)
}

/// The path of a file inside the directory of the companion.
fn join_remote(directory: &str, name: &str) -> String {
    format!("{}/{}", directory.trim_end_matches('/'), name)
}

/// The handler of the SSH client: the key the companion presents is accepted.
struct AcceptAnyKey;

impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Opens the file-transfer subsystem of the companion.
async fn connect(target: &ScpTarget) -> CommandResult<SftpSession> {
    let config = Arc::new(client::Config::default());

    let mut session = tokio::time::timeout(
        CONNECT_TIMEOUT,
        client::connect(config, (target.host.as_str(), target.port), AcceptAnyKey),
    )
    .await
    .map_err(|_| {
        CommandError::internal(format!(
            "{} did not answer within {} seconds",
            target.host,
            CONNECT_TIMEOUT.as_secs()
        ))
    })?
    .map_err(|error| {
        CommandError::internal(format!("{} could not be reached: {error}", target.host))
    })?;

    let answer = session
        .authenticate_password(target.user.clone(), target.password.clone())
        .await
        .map_err(|error| {
            CommandError::internal(format!("the password could not be offered: {error}"))
        })?;

    if !answer.success() {
        return Err(CommandError::invalid_request(
            "the companion refused the user and password".to_owned(),
        ));
    }

    let channel = session.channel_open_session().await.map_err(|error| {
        CommandError::internal(format!("the companion opened no session: {error}"))
    })?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| {
            CommandError::internal(format!("the companion answered no file transfer: {error}"))
        })?;

    SftpSession::new(channel.into_stream())
        .await
        .map_err(|error| {
            CommandError::internal(format!("the file transfer did not start: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_read_the_way_the_published_client_writes_it() {
        let target = ScpTarget::parse("user:secret@10.0.1.128:/home/user/dflogger/dataflash/")
            .expect("the address is read");

        assert_eq!(target.user, "user");
        assert_eq!(target.password, "secret");
        assert_eq!(target.host, "10.0.1.128");
        assert_eq!(target.port, 22);
        assert_eq!(target.path, "/home/user/dflogger/dataflash/");

        // A companion that is not on the default port says so before the path.
        let target =
            ScpTarget::parse("root:pw@edison:2222:/log/dataflash/").expect("the port is read");
        assert_eq!(target.host, "edison");
        assert_eq!(target.port, 2222);
        assert_eq!(target.path, "/log/dataflash/");
    }

    #[test]
    fn an_address_that_is_missing_a_part_is_refused_rather_than_guessed() {
        for bad in [
            "10.0.1.128:/logs/",
            "user@10.0.1.128:/logs/",
            "user:@10.0.1.128:/logs/",
            ":pw@10.0.1.128:/logs/",
            "user:pw@:/logs/",
            "user:pw@10.0.1.128:logs/",
            "user:pw@10.0.1.128:0:/logs/",
            "user:pw@10.0.1.128:notaport:/logs/",
        ] {
            let error = ScpTarget::parse(bad).expect_err(&format!("{bad} is not an address"));
            assert_eq!(
                error.code,
                CommandError::CODE_INVALID_REQUEST,
                "{bad} was accepted"
            );
        }
    }

    #[test]
    fn only_the_logs_are_kept_and_only_ones_this_station_writes() {
        let entries = vec![
            ("00000001.BIN".to_owned(), 10),
            ("notes.txt".to_owned(), 4),
            ("00000002.bin".to_owned(), 20),
            ("../escape.bin".to_owned(), 30),
            ("nested/00000003.bin".to_owned(), 40),
            ("00000004.bin.bak".to_owned(), 50),
        ];

        let logs = select_logs(entries);

        assert_eq!(
            logs,
            vec![
                RemoteLog {
                    name: "00000001.BIN".to_owned(),
                    size: 10
                },
                RemoteLog {
                    name: "00000002.bin".to_owned(),
                    size: 20
                },
            ]
        );
    }

    #[test]
    fn a_name_is_joined_to_the_directory_without_doubling_its_separator() {
        assert_eq!(join_remote("/logs/", "a.bin"), "/logs/a.bin");
        assert_eq!(join_remote("/logs", "a.bin"), "/logs/a.bin");
        assert_eq!(join_remote("/", "a.bin"), "/a.bin");
    }

    #[test]
    fn a_name_that_climbs_out_of_the_folder_is_not_one_of_ours() {
        assert_eq!(safe_file_name("00000001.bin"), Some("00000001.bin"));
        assert_eq!(safe_file_name("../secret.bin"), None);
        assert_eq!(safe_file_name("a/b.bin"), None);
        assert_eq!(safe_file_name("a\\b.bin"), None);
        assert_eq!(safe_file_name(".."), None);
        assert_eq!(safe_file_name(""), None);
    }
}
