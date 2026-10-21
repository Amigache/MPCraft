//! The firmware of the vehicles.
//!
//! The firmware of ArduPilot is published in a manifest: a long list of files, each of
//! them with the vehicle it is for, the board it is for, the version it belongs to and
//! where it is. The manifest is the list of what can be installed, and the operator
//! chooses the vehicle, the version and the board from it.
//!
//! This module reads the manifest and says what is in it. The upload itself is not
//! here: the file is written to the board by the bootloader of the board, and that is
//! a matter of its own.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{CommandError, CommandResult};

/// The address the manifest of the firmware is published at.
pub const MANIFEST_URL: &str = "https://firmware.ardupilot.org/manifest.json.gz";

/// The manifest of the firmware, as it is published.
#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    /// The files of the firmware.
    #[serde(default)]
    firmware: Vec<Entry>,
}

/// One file of the manifest.
#[derive(Debug, Clone, Deserialize)]
struct Entry {
    /// The vehicle the file is for: `Copter`, `Plane`, `Rover`, `Sub`.
    #[serde(rename = "vehicletype", default)]
    vehicle: String,
    /// The board the file is for.
    #[serde(default)]
    platform: String,
    /// Where the file is.
    #[serde(default)]
    url: String,
    /// The version the file belongs to: `V4.7.2`.
    #[serde(rename = "mav-firmware-version-str", default)]
    version: String,
    /// The shape of the file: `apj` for the autopilot.
    #[serde(default)]
    format: String,
    /// How big the file is, in bytes.
    #[serde(default)]
    size: Option<u64>,
    /// The hash of the file, so that a file that is not the one can be told.
    #[serde(default)]
    sha256: Option<String>,
}

/// The name the operator knows a vehicle by.
///
/// The manifest names them the way the code does; the interface names them the way
/// upstream does.
fn name_of_vehicle(vehicle: &str) -> String {
    match vehicle {
        "Copter" => "ArduCopter".to_owned(),
        "Plane" => "ArduPlane".to_owned(),
        "Sub" => "ArduSub".to_owned(),
        other => other.to_owned(),
    }
}

/// One vehicle of the manifest, and the versions of its firmware.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareBoard {
    /// The name of the vehicle: `ArduCopter`, `ArduPlane`, `Rover`, `ArduSub`.
    pub name: String,
    /// The versions of the firmware, the newest first.
    pub versions: Vec<FirmwareVersion>,
}

/// One version of the firmware of a vehicle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareVersion {
    /// The name of the version: `4.7.2`.
    pub version: String,
    /// The files of the version, one per board.
    pub files: Vec<FirmwareFile>,
}

/// One file of the firmware: the one of a board.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareFile {
    /// The board the file is for.
    pub platform: String,
    /// Where the file is.
    pub url: String,
    /// The shape of the file.
    pub format: String,
    /// How big the file is, in bytes.
    pub size: u64,
    /// The hash of the file.
    pub sha256: String,
}

/// The manifest of the firmware, read and said in the order the operator reads it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareManifest {
    /// The vehicles, with the versions of their firmware.
    pub boards: Vec<FirmwareBoard>,
}

/// Fetches the manifest of the firmware and reads it.
///
/// The manifest is published compressed, and the answer is uncompressed on the way in.
///
/// # Errors
///
/// Returns an error when the manifest cannot be fetched, or when it is not the document
/// that was expected.
pub async fn download_manifest(url: &str) -> CommandResult<FirmwareManifest> {
    let response = reqwest::get(url)
        .await
        .map_err(|error| {
            CommandError::internal(format!("the manifest could not be fetched: {error}"))
        })?;

    if !response.status().is_success() {
        return Err(CommandError::internal(format!(
            "the manifest answered with {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await.map_err(|error| {
        CommandError::internal(format!("the manifest could not be read: {error}"))
    })?;

    // The manifest is published as a compressed file, and the server says so with the
    // type of the file rather than with the way it is sent: it is unpacked here.
    let bytes = unpack(&bytes)?;

    parse_manifest(&bytes)
}

/// Unpacks a document that is published compressed.
fn unpack(bytes: &[u8]) -> CommandResult<Vec<u8>> {
    // The two first bytes of a compressed document are the ones that say it is one.
    if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        use std::io::Read as _;

        let mut out = Vec::new();

        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|error| {
                CommandError::internal(format!("the manifest could not be unpacked: {error}"))
            })?;

        return Ok(out);
    }

    Ok(bytes.to_vec())
}

/// Reads a manifest that is already in memory.
fn parse_manifest(bytes: &[u8]) -> CommandResult<FirmwareManifest> {
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|error| {
        CommandError::internal(format!("the manifest is not the one expected: {error}"))
    })?;

    // The files are gathered by vehicle and by version, and the boards are kept apart
    // so that the same board does not appear twice.
    let mut vehicles: BTreeMap<String, BTreeMap<String, BTreeMap<String, FirmwareFile>>> =
        BTreeMap::new();

    for entry in manifest.firmware {
        // The file of the autopilot is the one that is written to a board. The other
        // shapes are the bootloader and the firmware of other processors.
        if entry.format != "apj" {
            continue;
        }

        // The peripherals are not vehicles: they are the things that go with one.
        if entry.vehicle == "AP_Periph" {
            continue;
        }

        if entry.vehicle.is_empty() || entry.version.is_empty() || entry.platform.is_empty() {
            continue;
        }

        let version = entry.version.trim_start_matches(['v', 'V']).to_owned();

        vehicles
            .entry(name_of_vehicle(&entry.vehicle))
            .or_default()
            .entry(version)
            .or_default()
            .entry(entry.platform.clone())
            .or_insert_with(|| FirmwareFile {
                platform: entry.platform,
                url: entry.url,
                format: entry.format,
                size: entry.size.unwrap_or_default(),
                sha256: entry.sha256.unwrap_or_default(),
            });
    }

    let mut boards: Vec<FirmwareBoard> = vehicles
        .into_iter()
        .map(|(name, versions)| FirmwareBoard {
            name,
            versions: versions
                .into_iter()
                .map(|(version, files)| FirmwareVersion {
                    version,
                    files: files.into_values().collect(),
                })
                .collect(),
        })
        .collect();

    // The newest version comes first: it is the one that is wanted.
    for board in &mut boards {
        board.versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
        board
            .versions
            .iter_mut()
            .for_each(|version| version.files.sort_by(|a, b| a.platform.cmp(&b.platform)));
    }

    boards.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(FirmwareManifest { boards })
}

/// Fetches the file of a version of the firmware and writes it down.
///
/// The file is written as it comes, and the hash of the manifest is not checked here:
/// the file that is written is the one the operator chose, and the hash is said in the
/// interface so that it can be checked by hand.
///
/// # Errors
///
/// Returns an error when the file cannot be fetched or cannot be written.
pub async fn download_file(url: &str, output: &std::path::Path) -> CommandResult<u64> {
    let response = reqwest::get(url)
        .await
        .map_err(|error| {
            CommandError::internal(format!("the file could not be fetched: {error}"))
        })?;

    if !response.status().is_success() {
        return Err(CommandError::internal(format!(
            "the file answered with {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await.map_err(|error| {
        CommandError::internal(format!("the file could not be read: {error}"))
    })?;

    let length = bytes.len() as u64;

    std::fs::write(output, &bytes).map_err(|error| {
        CommandError::internal(format!(
            "the file could not be written to {}: {error}",
            output.display()
        ))
    })?;

    Ok(length)
}

/// Orders two versions of the firmware, the newest first.
///
/// The versions are named `4.3.7`, and the numbers are compared one by one: a version
/// with a fourth number is newer than one without it.
fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    fn numbers(version: &str) -> Vec<u64> {
        version
            .trim_start_matches(['v', 'V'])
            .split(['.', '-', '_'])
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    }

    let left = numbers(left);
    let right = numbers(right);

    for (a, b) in left.iter().zip(right.iter()) {
        match a.cmp(b) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }

    right.len().cmp(&left.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manifest with two vehicles and two versions each.
    const SAMPLE: &str = r#"{
        "format-version": "1.0.0",
        "firmware": [
            { "vehicletype": "Copter", "platform": "Pixhawk4", "url": "https://example.invalid/a", "mav-firmware-version-str": "V4.3.6", "format": "apj", "size": 10, "sha256": "aa" },
            { "vehicletype": "Copter", "platform": "Pixhawk4", "url": "https://example.invalid/b", "mav-firmware-version-str": "V4.3.7", "format": "apj", "size": 20, "sha256": "bb" },
            { "vehicletype": "Copter", "platform": "BeastH7", "url": "https://example.invalid/c", "mav-firmware-version-str": "V4.3.7", "format": "apj" },
            { "vehicletype": "Copter", "platform": "CubeBlack", "url": "https://example.invalid/d", "mav-firmware-version-str": "V4.3.7", "format": "hex" },
            { "vehicletype": "Plane", "platform": "Pixhawk4", "url": "https://example.invalid/e", "mav-firmware-version-str": "V4.3.7", "format": "apj" }
        ]
    }"#;

    #[test]
    fn the_vehicles_and_their_versions_are_read() {
        let manifest = parse_manifest(SAMPLE.as_bytes()).expect("the manifest is read");

        assert_eq!(manifest.boards.len(), 2);
        assert_eq!(manifest.boards[0].name, "ArduCopter");
        assert_eq!(manifest.boards[0].versions.len(), 2);
        assert_eq!(manifest.boards[0].versions[0].version, "4.3.7");
        assert_eq!(manifest.boards[0].versions[0].files.len(), 2);
        assert_eq!(manifest.boards[0].versions[0].files[0].platform, "BeastH7");
        assert_eq!(manifest.boards[1].name, "ArduPlane");
    }

    #[test]
    fn the_files_that_are_not_the_autopilot_are_left_out() {
        let manifest = parse_manifest(SAMPLE.as_bytes()).expect("the manifest is read");
        let files = &manifest.boards[0].versions[0].files;

        assert!(
            !files.iter().any(|file| file.platform == "CubeBlack"),
            "the file of the bootloader is not the firmware of the autopilot"
        );
    }

    #[test]
    fn the_versions_are_ordered_from_the_newest_to_the_oldest() {
        assert_eq!(
            compare_versions("4.3.7", "4.3.6"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("4.3.7", "4.4.0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("4.3.7", "4.3.7"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn a_manifest_that_is_not_the_one_expected_is_an_error() {
        let error = parse_manifest(b"this is not a manifest").expect_err("it is not a manifest");
        assert_eq!(error.code, CommandError::CODE_INTERNAL);
    }
}
