//! The diagnostics of the application.
//!
//! When something goes wrong, the first thing anybody asks is what the application was
//! doing at the time. The bundle gathers what is known: the version, the machine, the
//! connection profiles, the state of the link and the last lines the application said,
//! and writes it down in one file that can be sent to whoever is asked to look.
//!
//! The bundle is a zip: it is the shape everybody knows, and it holds several files
//! without needing a name for each one.

use std::path::Path;

use crate::error::{CommandError, CommandResult};

/// The signature of a local file header of a zip.
const LOCAL_HEADER: u32 = 0x0403_4b50;

/// The signature of the central directory of a zip.
const CENTRAL_HEADER: u32 = 0x0201_4b50;

/// The signature of the end of the central directory of a zip.
const END_OF_DIRECTORY: u32 = 0x0605_4b50;

/// One file of the bundle.
pub struct Entry {
    /// The name of the file inside the bundle.
    pub name: String,
    /// What the file holds.
    pub contents: String,
}

/// Gathers what is known about the application right now.
///
/// The bundle holds a file per subject: the machine the application runs on, the
/// connection profiles that are saved, and the state of the link.
///
/// # Errors
///
/// Returns an error when the bundle cannot be written.
pub fn write_bundle(output: &Path, entries: &[Entry]) -> CommandResult<()> {
    let zip = zip_of(entries);

    std::fs::write(output, zip).map_err(|error| {
        CommandError::internal(format!(
            "the bundle could not be written to {}: {error}",
            output.display()
        ))
    })
}

/// What is known about the application right now.
pub fn gather(state: &crate::AppState) -> Vec<Entry> {
    let mut entries = vec![Entry {
        name: "machine.txt".to_owned(),
        contents: machine(),
    }];

    // The profiles that are saved: the operator's own settings are not the diagnosis,
    // but the place the vehicle is expected to be is.
    let profiles = state
        .connection_profiles()
        .map(|profiles| {
            profiles
                .iter()
                .map(|profile| {
                    format!(
                        "{} — {} ({})",
                        profile.name, profile.target, profile.auto_connect
                    )
                })
                .collect::<Vec<String>>()
                .join("\n")
        })
        .unwrap_or_else(|error| format!("the profiles could not be read: {error}"));

    entries.push(Entry {
        name: "profiles.txt".to_owned(),
        contents: profiles,
    });

    // The state of the link.
    let link = match state.supervisor.state() {
        Some(session) => format!(
            "the link is open\n\n{:#?}",
            session.vehicles.iter().map(|vehicle| vehicle.id).collect::<Vec<_>>()
        ),
        None => "there is no open link".to_owned(),
    };

    entries.push(Entry {
        name: "link.txt".to_owned(),
        contents: link,
    });

    entries
}

/// What the machine is.
fn machine() -> String {
    format!(
        "the system: {}\nthe architecture: {}\nthe processors: {}\nthe version of the application: {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism()
            .map(|number| number.get())
            .unwrap_or_default(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Puts the files of the bundle into a zip.
fn zip_of(entries: &[Entry]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    let mut count = 0_u16;

    for entry in entries {
        let name = entry.name.as_bytes();
        let data = entry.contents.as_bytes();
        let crc = crc32(data);
        let offset = out.len() as u32;

        // The local header of the file.
        out.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes()); // the version that made it
        out.extend_from_slice(&20_u16.to_le_bytes()); // the version that is needed
        out.extend_from_slice(&0_u16.to_le_bytes()); // no flags
        out.extend_from_slice(&0_u16.to_le_bytes()); // stored as it is
        out.extend_from_slice(&0_u16.to_le_bytes()); // the time
        out.extend_from_slice(&0_u16.to_le_bytes()); // the date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes()); // no extra
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        // The entry of the central directory.
        directory.extend_from_slice(&CENTRAL_HEADER.to_le_bytes());
        directory.extend_from_slice(&20_u16.to_le_bytes());
        directory.extend_from_slice(&20_u16.to_le_bytes());
        directory.extend_from_slice(&0_u16.to_le_bytes());
        directory.extend_from_slice(&0_u16.to_le_bytes());
        directory.extend_from_slice(&0_u16.to_le_bytes());
        directory.extend_from_slice(&0_u16.to_le_bytes());
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        directory.extend_from_slice(&0_u16.to_le_bytes()); // no extra
        directory.extend_from_slice(&0_u16.to_le_bytes()); // no comment
        directory.extend_from_slice(&0_u16.to_le_bytes()); // the disk
        directory.extend_from_slice(&0_u16.to_le_bytes()); // the internal attributes
        directory.extend_from_slice(&0_u32.to_le_bytes()); // the external attributes
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name);

        count += 1;
    }

    let directory_offset = out.len() as u32;
    let directory_size = directory.len() as u32;

    out.extend_from_slice(&directory);

    // The end of the central directory.
    out.extend_from_slice(&END_OF_DIRECTORY.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // the disk
    out.extend_from_slice(&0_u16.to_le_bytes()); // the disk with the directory
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // no comment

    out
}

/// The CRC-32 of a file, which is what a zip says about it.
///
/// The polynomial is the one of the zip, which is the same one that the Ethernet uses
/// turned the other way round.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;

    for byte in data {
        crc ^= u32::from(*byte);

        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }

    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_of_a_file_is_the_one_that_is_known() {
        // The CRC-32 of "123456789" is a number everybody knows.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn the_bundle_is_a_zip_with_the_files_in_it() {
        let entries = vec![
            Entry {
                name: "version.txt".to_owned(),
                contents: "MPCraft 0.1.0".to_owned(),
            },
            Entry {
                name: "link.txt".to_owned(),
                contents: "connected".to_owned(),
            },
        ];

        let zip = zip_of(&entries);

        assert_eq!(&zip[0..4], &LOCAL_HEADER.to_le_bytes());
        assert!(
            zip.windows(4).any(|window| window == CENTRAL_HEADER.to_le_bytes()),
            "the bundle has a central directory"
        );
        assert!(
            zip.windows(4).any(|window| window == END_OF_DIRECTORY.to_le_bytes()),
            "the bundle says where the directory ends"
        );
        assert!(
            contains(&zip, b"MPCraft 0.1.0"),
            "the bundle holds what was put in it"
        );
        assert!(
            contains(&zip, b"version.txt"),
            "the bundle names the files it holds"
        );
    }

    /// Whether a piece of bytes is in a piece of bytes.
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}
