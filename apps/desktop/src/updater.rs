//! The update of the application itself.
//!
//! The release of MPCraft is published on GitHub, and the update is checked against the document
//! the release carries — `latest.json`. That document says the version, the address of the archive
//! for each platform and the signature of that archive. The signature is checked with the public
//! key the configuration carries, so an archive that was not signed by the holder of the private
//! key is refused before anything is installed.
//!
//! Nothing is fetched until the operator asks. The check is a command rather than a task that runs
//! at launch, because a program that reaches the network by itself is a program that says something
//! about the machine it runs on without being asked.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use mpcraft_app::{CommandError, CommandResult};

/// What the release says about an update that is newer than this copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// The version of this copy.
    pub current_version: String,
    /// The version the release announces.
    pub version: String,
    /// What the release says about the update. A release that says nothing leaves this empty.
    pub notes: String,
}

/// Asks the release whether there is a newer version.
///
/// The answer is what the release says, not what this function decides: a release that announces
/// the version this copy already has is not an update, and the updater of the protocol decides it.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the updater is not configured, or when the
/// release cannot be read. A machine that is offline is not an error of the application: it is a
/// check that did not answer.
pub async fn check(app: &AppHandle) -> CommandResult<Option<UpdateInfo>> {
    let updater = app.updater().map_err(|error| {
        CommandError::internal(format!("the updater is not configured: {error}"))
    })?;

    let found = updater.check().await.map_err(|error| {
        CommandError::internal(format!("the release could not be read: {error}"))
    })?;

    Ok(found.map(|update| UpdateInfo {
        current_version: update.current_version,
        version: update.version,
        notes: update.body.unwrap_or_default(),
    }))
}

/// Downloads the update the release announces, installs it and starts the application again.
///
/// The archive is verified against the public key of the configuration before it is installed, so
/// an archive that is not signed by that key stops here.
///
/// # Errors
///
/// Returns a `CommandError` with a stable code when the release cannot be read, when the download
/// fails, or when the update cannot be installed.
pub async fn install(app: &AppHandle) -> CommandResult<()> {
    let updater = app.updater().map_err(|error| {
        CommandError::internal(format!("the updater is not configured: {error}"))
    })?;

    let Some(update) = updater.check().await.map_err(|error| {
        CommandError::internal(format!("the release could not be read: {error}"))
    })?
    else {
        return Ok(());
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| {
            CommandError::internal(format!("the update could not be installed: {error}"))
        })?;

    // The copy that was running is the one the update replaced, so the window has to come from the
    // new one. `restart` does not return.
    app.restart();
}
