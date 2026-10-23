//! The plugins the operator chose to run, and the driver that calls their stages.
//!
//! Upstream finds every assembly in its own folder and runs it, which means a plugin is run because
//! it happens to be there. Here the folder is read, but nothing runs until the operator says so: a
//! program that ran whatever it found in a folder would be running code nobody agreed to.
//!
//! The driver is deliberately small. It reads the manifests, resolves what the operator decided
//! against what the manifest asks for, and calls the four stages in order. The sandbox is not the
//! driver's business — it is handed a manifest and gives back a plugin that can be called — so this
//! file holds no runtime knowledge at all.
//!
//! Two things are kept apart on purpose. What a plugin asked for is queued here and run by the
//! application afterwards, because a call inside the sandbox must never wait for the wire: a plugin
//! that held the link while it waited for an answer would hold it for every other consumer. And what
//! a plugin said is kept with a ceiling, like the messages of the vehicle, so that a plugin that
//! talks without stopping cannot grow the state of the application.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use mpcraft_link::SessionState;
use mpcraft_plugin::{
    CommandIntent, HostApi, Limits, Plugin, PluginError, PluginManifest, Sandbox, Stage,
    TelemetryField,
};
use mpcraft_settings::Settings;
use mpcraft_vehicle::Telemetry;

use crate::error::{CommandError, CommandResult};

/// How many lines of what the plugins said are kept.
///
/// The message console shows the last lines, and a plugin that wrote without stopping would otherwise
/// be the thing that grew the memory of the application.
pub const MAX_NOTES: usize = 100;

/// How many commands a plugin may have waiting at one moment.
pub const MAX_PENDING_COMMANDS: usize = 32;

/// Where a plugin stands, which is what the management sheet lists it under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// The plugin is in the folder and nobody chose to run it.
    NotRun,

    /// The plugin was read and its first stage said it worked.
    Ready,

    /// The plugin has been called on a tick.
    Running,

    /// The plugin could not be read, or a stage of it said it did not work.
    Failed,
}

/// One plugin as the driver sees it, which is the shape the management sheet reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    /// The name the manifest says.
    pub name: String,

    /// The version the plugin says it is.
    pub version: String,

    /// Who wrote it, which is what an operator needs before deciding to run it.
    pub author: String,

    /// What the manifest asks for, named the way a manifest writes it.
    pub capabilities: Vec<String>,

    /// How often the plugin is called, once per tick.
    pub loop_hz: f64,

    /// Whether the operator chose to run it.
    pub enabled: bool,

    /// Where the plugin stands.
    pub state: PluginState,

    /// The code of the last failure, when there was one.
    pub error_code: Option<String>,

    /// What the last failure said.
    pub error_message: Option<String>,
}

impl std::fmt::Debug for PluginRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The runtime itself is not something a person can read out of a log line: what matters is the
        // folder and what the driver sees in it.
        f.debug_struct("PluginRuntime")
            .field("folder", &self.folder)
            .field("plugins", &self.info())
            .finish()
    }
}

/// What the vehicle last said about one thing, in the units the field names.
#[must_use]
pub fn value_of(telemetry: &Telemetry, field: TelemetryField) -> Option<f64> {
    match field {
        TelemetryField::Altitude => telemetry.position.map(|position| position.alt),
        TelemetryField::GroundSpeed => telemetry.velocity.map(|velocity| velocity.ground_speed),
        TelemetryField::Heading => telemetry.attitude.map(|attitude| attitude.yaw),
        TelemetryField::BatteryVolts => telemetry.battery.map(|battery| battery.voltage),
        TelemetryField::BatteryRemaining => telemetry.battery.map(|battery| battery.remaining),
        TelemetryField::DistanceToHome => telemetry.distance_to_home,
        TelemetryField::Satellites => telemetry.gps.map(|gps| f64::from(gps.satellites)),
        // A field a plugin might be given later is not guessed at now.
        _ => None,
    }
}

#[derive(Debug, Default)]
struct Inner {
    /// What the vehicle last said.
    telemetry: Telemetry,

    /// How many items the plan on the vehicle holds.
    mission_count: u32,

    /// What the plugins asked for, which is run afterwards, outside the sandbox.
    pending: VecDeque<CommandIntent>,

    /// What the plugins said, most recent last.
    console: VecDeque<String>,
}

/// The surface a plugin is plugged into, backed by the state the application keeps.
///
/// The values are read from what the link published, so that the sandbox never has to reach the
/// session itself: the plugin crate is below the application, and this is the shape it is allowed to
/// see.
#[derive(Debug, Default)]
pub struct PluginSurface {
    inner: Arc<Mutex<Inner>>,
}

impl PluginSurface {
    /// A surface with nothing in it yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what the link published, so that a plugin reads the same numbers the window shows.
    pub fn record(&self, session: &SessionState) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.telemetry = session.telemetry.clone();

            // The plan the vehicle holds is counted from what the vehicle reported.
            inner.mission_count = session
                .telemetry
                .mission
                .map(|progress| u32::from(progress.total))
                .unwrap_or(0);
        }
    }

    /// Takes the commands the plugins asked for, which are run outside the sandbox.
    pub fn take_pending(&self) -> Vec<CommandIntent> {
        match self.inner.lock() {
            Ok(mut inner) => inner.pending.drain(..).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// What the plugins said, most recent last.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        match self.inner.lock() {
            Ok(inner) => inner.console.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl HostApi for PluginSurface {
    fn telemetry(&self, field: TelemetryField) -> Option<f64> {
        let inner = self.inner.lock().ok()?;

        value_of(&inner.telemetry, field)
    }

    fn ask(&self, intent: CommandIntent) -> Result<(), PluginError> {
        let mut inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) => {
                return Err(PluginError::Instantiate {
                    plugin: String::new(),
                    message: "the surface of the plugins is locked".to_string(),
                })
            }
        };

        if inner.pending.len() >= MAX_PENDING_COMMANDS {
            return Err(PluginError::Denied {
                plugin: String::new(),
                resource: "commands",
                max: MAX_PENDING_COMMANDS,
            });
        }

        inner.pending.push_back(intent);

        Ok(())
    }

    fn mission_count(&self) -> u32 {
        match self.inner.lock() {
            Ok(inner) => inner.mission_count,
            Err(_) => 0,
        }
    }

    fn note(&self, text: &str) -> Result<(), PluginError> {
        let mut inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) => {
                return Err(PluginError::Instantiate {
                    plugin: String::new(),
                    message: "the surface of the plugins is locked".to_string(),
                })
            }
        };

        while inner.console.len() >= MAX_NOTES {
            inner.console.pop_front();
        }

        inner.console.push_back(text.to_string());

        Ok(())
    }
}

/// One plugin as the driver holds it.
#[derive(Debug)]
struct Entry {
    /// What the manifest said, when the manifest was one. A plugin whose manifest could not be read
    /// still has a name, because that is what the sheet has to show beside the failure.
    manifest: Option<PluginManifest>,

    name: String,
    enabled: bool,
    state: PluginState,
    error: Option<CommandError>,
    plugin: Option<Plugin>,

    /// The instant the next tick of this plugin is due at, which is what a plugin called at two times
    /// a second is not called at twenty.
    next: Instant,
}

/// The driver: the folder, the manifests, and the plugins the operator chose.
pub struct PluginRuntime {
    folder: PathBuf,
    sandbox: Option<Sandbox>,
    surface: Arc<PluginSurface>,
    entries: BTreeMap<String, Entry>,

    /// What the runtime said when it was built. A runtime that cannot build its engine still reads the
    /// folder — a person needs to see which plugin is in it — but it cannot run one.
    startup_error: Option<CommandError>,
}

impl PluginRuntime {
    /// A driver for a folder of plugins.
    #[must_use]
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        let folder = folder.into();

        // The engine of the sandbox is not something a manifest could have gotten wrong, so a failure
        // here is kept as it came and reported when a plugin is asked to run.
        match Sandbox::new(Limits::default()) {
            Ok(sandbox) => Self {
                folder,
                sandbox: Some(sandbox),
                surface: Arc::new(PluginSurface::new()),
                entries: BTreeMap::new(),
                startup_error: None,
            },
            Err(error) => Self {
                folder,
                sandbox: None,
                surface: Arc::new(PluginSurface::new()),
                entries: BTreeMap::new(),
                startup_error: Some(CommandError::from(error)),
            },
        }
    }

    /// The folder the plugins are read from.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The surface the plugins are plugged into.
    #[must_use]
    pub fn surface(&self) -> &Arc<PluginSurface> {
        &self.surface
    }

    /// Reads the folder, and says what is in it and what the operator chose.
    ///
    /// # Errors
    ///
    /// Returns an error when the folder itself cannot be read. A manifest that is not one is not a
    /// failure of the whole folder: it is a plugin that stands failed, with the code that names what
    /// went wrong.
    pub fn scan(&mut self, settings: &Settings) -> CommandResult<Vec<PluginInfo>> {
        self.entries.clear();

        let items = match std::fs::read_dir(&self.folder) {
            Ok(items) => items,
            Err(source) => {
                return Err(CommandError::internal(format!(
                    "cannot read the folder of plugins: {source}"
                )));
            }
        };

        let mut paths = Vec::new();

        for item in items {
            let path = match item {
                Ok(item) => item.path(),
                Err(_) => continue,
            };

            if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
                paths.push(path);
            }
        }

        // The folder is listed in the order the names fall in, so that the sheet does not change shape
        // between two reads.
        paths.sort();

        for path in paths {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_default();

            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(source) => {
                    let error = CommandError::new(
                        "plugin_parse",
                        format!(
                            "cannot read the plugin manifest {}: {source}",
                            path.display()
                        ),
                    );

                    self.entries.insert(
                        name.clone(),
                        Entry {
                            manifest: None,
                            name,
                            enabled: false,
                            state: PluginState::Failed,
                            error: Some(error),
                            plugin: None,
                            next: Instant::now(),
                        },
                    );

                    continue;
                }
            };

            match PluginManifest::from_text(&text, path.clone()) {
                Ok(manifest) => {
                    let enabled = settings
                        .plugins
                        .iter()
                        .any(|entry| entry.name == manifest.name && entry.enabled);

                    let name = manifest.name.clone();

                    self.entries.insert(
                        name.clone(),
                        Entry {
                            manifest: Some(manifest),
                            name,
                            enabled,
                            state: PluginState::NotRun,
                            error: None,
                            plugin: None,
                            next: Instant::now(),
                        },
                    );
                }
                Err(error) => {
                    // A manifest that is not one still has a name, and the name is what the sheet has
                    // to show beside the failure.
                    self.entries.insert(
                        name.clone(),
                        Entry {
                            manifest: None,
                            name,
                            enabled: false,
                            state: PluginState::Failed,
                            error: Some(CommandError::from(error)),
                            plugin: None,
                            next: Instant::now(),
                        },
                    );
                }
            }
        }

        Ok(self.info())
    }

    /// Runs the first stage of every plugin the operator chose.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime itself cannot run anything. A plugin that fails is not a
    /// failure of the driver: it stands failed with its own code, which is what the sheet shows.
    pub fn open(&mut self) -> CommandResult<Vec<PluginInfo>> {
        if let Some(error) = &self.startup_error {
            return Err(error.clone());
        }

        // The modules are read before anything is written back, because reading one is not something
        // the list of plugins can be doing at the same time.
        let chosen: Vec<(String, Result<Plugin, CommandError>)> = self
            .entries
            .values()
            .filter(|entry| entry.enabled && entry.plugin.is_none())
            .map(|entry| (entry.name.clone(), self.read(entry)))
            .collect();

        for (name, outcome) in chosen {
            let entry = self
                .entries
                .get_mut(&name)
                .expect("the name came from the entries");

            match outcome {
                Ok(plugin) => {
                    entry.plugin = Some(plugin);
                    entry.state = PluginState::Ready;
                    entry.error = None;
                }
                Err(error) => {
                    entry.state = PluginState::Failed;
                    entry.error = Some(error);
                }
            }
        }

        Ok(self.info())
    }

    /// Tells every plugin that is running that the link is open.
    pub fn link_opened(&mut self) -> Vec<PluginInfo> {
        for entry in self.entries.values_mut() {
            let Some(plugin) = entry.plugin.as_mut() else {
                continue;
            };

            if let Err(error) = plugin.run(Stage::Loaded) {
                entry.state = PluginState::Failed;
                entry.error = Some(CommandError::from(error));
            }
        }

        self.info()
    }

    /// Calls the tick of each plugin whose own rate is due.
    ///
    /// A plugin that asked for two times a second is not called twenty times a second: the driver is
    /// called on the tick of the telemetry and hands the call to a plugin only when that plugin's own
    /// rate has come round.
    pub fn tick(&mut self, now: Instant) -> usize {
        let mut called = 0;

        for entry in self.entries.values_mut() {
            let Some(plugin) = entry.plugin.as_mut() else {
                continue;
            };

            if entry.state == PluginState::Failed || now < entry.next {
                continue;
            }

            match plugin.run(Stage::Loop) {
                Ok(()) => {
                    called += 1;
                    entry.state = PluginState::Running;
                    entry.next = now
                        + rate(
                            entry
                                .manifest
                                .as_ref()
                                .map_or(1.0, |manifest| manifest.loop_hz),
                        );
                }
                Err(error) => {
                    // A plugin that stopped once is not run again: the operator has to see that it
                    // stopped, and a plugin called again after a trap is a plugin that may trap again
                    // on every tick.
                    entry.state = PluginState::Failed;
                    entry.error = Some(CommandError::from(error));
                }
            }
        }

        called
    }

    /// Tells every plugin that is running that the application is closing.
    pub fn close(&mut self) -> Vec<PluginInfo> {
        for entry in self.entries.values_mut() {
            let Some(plugin) = entry.plugin.as_mut() else {
                continue;
            };

            if let Err(error) = plugin.run(Stage::Exit) {
                entry.error = Some(CommandError::from(error));
            }
        }

        self.info()
    }

    /// Chooses whether a plugin is run, and runs or leaves alone the one that was chosen.
    ///
    /// # Errors
    ///
    /// Returns an error when the plugin was not seen, or when the one that was chosen could not be
    /// read.
    pub fn set_enabled(&mut self, name: &str, chosen: bool) -> CommandResult<()> {
        if !self.entries.contains_key(name) {
            return Err(CommandError::invalid_request(format!(
                "there is no plugin called {name} in the folder"
            )));
        }

        // The module is read before the list is written back, like everywhere else in the driver.
        let outcome = if chosen {
            Some(
                self.read(
                    self.entries
                        .get(name)
                        .expect("the name was seen just before"),
                ),
            )
        } else {
            None
        };

        let entry = self
            .entries
            .get_mut(name)
            .expect("the name was seen just before");

        entry.enabled = chosen;

        if let Some(outcome) = outcome {
            match outcome {
                Ok(plugin) => {
                    entry.plugin = Some(plugin);
                    entry.state = PluginState::Ready;
                    entry.error = None;
                }
                Err(error) => {
                    entry.state = PluginState::Failed;
                    entry.error = Some(error);
                }
            }
        } else {
            if let Some(plugin) = entry.plugin.as_mut() {
                let _ = plugin.run(Stage::Exit);
            }

            entry.plugin = None;
            entry.state = PluginState::NotRun;
            entry.error = None;
        }

        Ok(())
    }

    /// What the driver sees, which is what the sheet lists.
    #[must_use]
    pub fn info(&self) -> Vec<PluginInfo> {
        self.entries
            .values()
            .map(|entry| PluginInfo {
                name: entry.name.clone(),
                version: entry
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.version.clone())
                    .unwrap_or_default(),
                author: entry
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.author.clone())
                    .unwrap_or_default(),
                capabilities: entry
                    .manifest
                    .as_ref()
                    .map(|manifest| {
                        manifest
                            .capabilities
                            .iter()
                            .map(|capability| capability.name().to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                loop_hz: entry
                    .manifest
                    .as_ref()
                    .map_or(0.0, |manifest| manifest.loop_hz),
                enabled: entry.enabled,
                state: entry.state,
                error_code: entry.error.as_ref().map(|error| error.code.clone()),
                error_message: entry.error.as_ref().map(|error| error.message.clone()),
            })
            .collect()
    }

    /// Reads one module and runs the stage a plugin cannot be run without.
    fn read(&self, entry: &Entry) -> Result<Plugin, CommandError> {
        let manifest = entry.manifest.as_ref().ok_or_else(|| {
            CommandError::new(
                "plugin_parse",
                format!("the plugin {} has no manifest", entry.name),
            )
        })?;

        // The module is named relative to the folder of the manifest, and a plugin that names a path
        // outside that folder is not read: `..` in a manifest is a plugin reaching past the folder it
        // was put in.
        let path = module_path(&self.folder, &manifest.module)?;

        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) => {
                return Err(CommandError::new(
                    "plugin_module",
                    format!("cannot read the module of {}: {source}", manifest.name),
                ));
            }
        };

        let sandbox = self.sandbox.as_ref().ok_or_else(|| {
            self.startup_error
                .clone()
                .unwrap_or_else(|| CommandError::internal("the sandbox is not built"))
        })?;

        // The surface is handed to the sandbox as the shape a plugin is allowed to see, which is the
        // whole of the layering: the plugin crate never knows it is talking to the application.
        let api: Arc<dyn HostApi> = self.surface.clone();

        let mut plugin = sandbox
            .load(manifest, &bytes, api)
            .map_err(CommandError::from)?;

        // The first stage is where a plugin decides whether it can work at all, so it is run when the
        // plugin is read rather than on a tick.
        plugin.run(Stage::Init).map_err(CommandError::from)?;

        Ok(plugin)
    }
}

/// The module of a plugin, kept inside the folder it was read from.
fn module_path(folder: &Path, module: &Path) -> CommandResult<PathBuf> {
    if module.is_absolute() || module.components().any(|part| part == Component::ParentDir) {
        return Err(CommandError::new(
            "plugin_module",
            format!(
                "the module `{}` is outside the folder of plugins",
                module.display()
            ),
        ));
    }

    Ok(folder.join(module))
}

/// How long a plugin waits between two calls.
fn rate(hz: f64) -> Duration {
    if hz <= 0.0 || !hz.is_finite() {
        // A plugin with no rate is called on every tick, which is what a plugin that wants to be
        // called continuously means.
        return Duration::ZERO;
    }

    Duration::from_secs_f64(1.0 / hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    use mpcraft_settings::PluginEntry;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A folder of plugins that is removed when the test ends.
    struct PluginFolder {
        path: PathBuf,
    }

    impl PluginFolder {
        fn new(name: &str) -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mpcraft-plugins-{name}-{}-{unique}",
                std::process::id()
            ));

            std::fs::create_dir_all(&path).expect("the folder of plugins should be creatable");

            Self { path }
        }

        /// The folder.
        fn path(&self) -> &Path {
            &self.path
        }

        /// Writes a manifest and the module it names.
        fn write(&self, manifest: &str, wat: &str) {
            let field = |key: &str| {
                manifest
                    .lines()
                    .find(|line| line.starts_with(&format!("{key} = ")))
                    .map(|line| line.replace(&format!("{key} = \""), "").replace('"', ""))
                    .unwrap_or_else(|| "plugin".to_string())
            };

            let name = field("name");
            let module = field("module");

            std::fs::write(self.path.join(format!("{name}.toml")), manifest)
                .expect("the manifest should be writable");

            // The module is written as text, and text is only read as text when it begins with the
            // module, so the spaces are taken off before the file is written.
            std::fs::write(self.path.join(&module), wat.trim())
                .expect("the module should be writable");
        }
    }

    impl Drop for PluginFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn chosen(names: &[&str]) -> Settings {
        Settings {
            plugins: names
                .iter()
                .map(|name| PluginEntry::enabled(*name))
                .collect(),
            ..Settings::default()
        }
    }

    const NOTIFY_MODULE: &str = r#"
        (module
            (import "host" "note" (func $note (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "the pack is low")
            (func (export "init") (result i32)
                i32.const 0
                i32.const 15
                call $note
                drop
                i32.const 0))
    "#;

    #[test]
    fn a_plugin_nobody_chose_is_seen_and_not_run() {
        let folder = PluginFolder::new("chosen");

        folder.write(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\ncapabilities = [\"notify\"]\n",
            NOTIFY_MODULE,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        let info = runtime.scan(&Settings::default()).unwrap();

        assert_eq!(info.len(), 1);
        assert_eq!(info[0].name, "alarm");
        assert!(!info[0].enabled, "a plugin nobody chose is not run");
        assert_eq!(info[0].state, PluginState::NotRun);

        runtime.open().unwrap();

        // Nothing was said, because nothing was run.
        assert!(runtime.surface().notes().is_empty());
    }

    #[test]
    fn a_plugin_the_operator_chose_is_read_and_its_words_reach_the_console() {
        let folder = PluginFolder::new("console");

        folder.write(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\ncapabilities = [\"notify\"]\n",
            NOTIFY_MODULE,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        let info = runtime.scan(&chosen(&["alarm"])).unwrap();

        assert!(info[0].enabled);

        runtime.open().unwrap();

        let info = runtime.info();

        assert_eq!(
            info[0].state,
            PluginState::Ready,
            "{}",
            info[0].error_message.as_deref().unwrap_or_default()
        );
        assert_eq!(
            runtime.surface().notes(),
            vec!["the pack is low".to_string()]
        );
    }

    #[test]
    fn a_command_a_plugin_asks_for_is_queued_rather_than_run_inside_the_sandbox() {
        let folder = PluginFolder::new("command");

        folder.write(
            "name = \"beeper\"\nversion = \"1.0.0\"\nmodule = \"beeper.wasm\"\ncapabilities = [\"send_commands\"]\n",
            r#"
                (module
                    (import "host" "command" (func $command (param i32 f64 i32 i32) (result i32)))
                    (func (export "init") (result i32)
                        i32.const 208
                        f64.const 30
                        i32.const 0
                        i32.const 0
                        call $command
                        drop
                        i32.const 0))
            "#,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        runtime.scan(&chosen(&["beeper"])).unwrap();
        runtime.open().unwrap();

        // The plugin asked, and the driver holds the asking: nothing was sent, because the sandbox is
        // not allowed to reach the wire.
        let pending = runtime.surface().take_pending();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].command, 208);
        assert_eq!(pending[0].params[0], 30.0);
    }

    #[test]
    fn a_plugin_is_called_only_when_its_own_rate_is_due() {
        let folder = PluginFolder::new("rate");

        folder.write(
            "name = \"slow\"\nversion = \"1.0.0\"\nmodule = \"slow.wasm\"\ncapabilities = [\"notify\"]\nloop_hz = 2\n",
            r#"
                (module
                    (import "host" "note" (func $note (param i32 i32) (result i32)))
                    (memory (export "memory") 1)
                    (data (i32.const 0) "tick")
                    (func (export "init") (result i32) i32.const 0)
                    (func (export "loop") (result i32)
                        i32.const 0
                        i32.const 4
                        call $note
                        drop
                        i32.const 0))
            "#,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        runtime.scan(&chosen(&["slow"])).unwrap();
        runtime.open().unwrap();

        let now = Instant::now();

        // The first call is due at once; the second is not due until half a second later, which is what
        // a plugin that asked for two times a second means on a tick of twenty.
        assert_eq!(runtime.tick(now), 1);
        assert_eq!(runtime.tick(now + Duration::from_millis(100)), 0);
        assert_eq!(runtime.tick(now + Duration::from_millis(500)), 1);

        assert_eq!(runtime.surface().notes().len(), 2);
        assert_eq!(runtime.info()[0].state, PluginState::Running);
    }

    #[test]
    fn a_manifest_that_is_not_one_stands_failed_with_its_own_code() {
        let folder = PluginFolder::new("manifest");

        std::fs::write(
            folder.path.join("broken.toml"),
            "name = \"broken\"\nversion = \"1.0.0\"\nmodule = \"broken.wasm\"\ncapabilities = [\"write_parameters\"]\n",
        )
        .unwrap();

        let mut runtime = PluginRuntime::new(folder.path());
        let info = runtime.scan(&chosen(&["broken"])).unwrap();

        assert_eq!(info[0].state, PluginState::Failed);
        assert_eq!(info[0].error_code.as_deref(), Some("plugin_capability"));
    }

    #[test]
    fn a_module_that_names_a_path_outside_the_folder_is_not_read() {
        let error = module_path(Path::new("plugins"), Path::new("../elsewhere.wasm"))
            .expect_err("a manifest reaching past its folder is refused");

        assert_eq!(error.code, "plugin_module");

        // A path that stays inside the folder is read as the manifest says.
        assert_eq!(
            module_path(Path::new("plugins"), Path::new("alarm.wasm")).unwrap(),
            PathBuf::from("plugins").join("alarm.wasm")
        );
    }

    #[test]
    fn a_stage_that_says_it_did_not_work_stands_failed_with_its_code() {
        let folder = PluginFolder::new("stage");

        folder.write(
            "name = \"broken\"\nversion = \"1.0.0\"\nmodule = \"broken.wasm\"\n",
            r#"
                (module
                    (func (export "init") (result i32) i32.const 3))
            "#,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        runtime.scan(&chosen(&["broken"])).unwrap();
        runtime.open().unwrap();

        let info = &runtime.info()[0];

        assert_eq!(info.state, PluginState::Failed);
        assert_eq!(info.error_code.as_deref(), Some("plugin_stage"));
    }

    #[test]
    fn what_a_plugin_says_is_kept_with_the_ceiling_the_console_has() {
        let surface = PluginSurface::new();

        for index in 0..(MAX_NOTES + 10) {
            surface.note(&format!("line {index}")).unwrap();
        }

        let kept = surface.notes();

        assert_eq!(kept.len(), MAX_NOTES);

        // The ten oldest lines are the ones that fell off, which is what a console keeps.
        assert_eq!(kept.first().unwrap(), "line 10");
        assert_eq!(kept.last().unwrap(), "line 109");
    }

    #[test]
    fn the_values_of_the_surface_are_what_the_vehicle_last_said() {
        let telemetry = Telemetry {
            position: Some(mpcraft_vehicle::GeoPoint {
                lat: 41.0,
                lng: 2.0,
                alt: 120.0,
            }),
            velocity: Some(mpcraft_vehicle::Velocity {
                north: 5.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 5.0,
            }),
            attitude: Some(mpcraft_vehicle::Attitude {
                roll: 0.0,
                pitch: 0.0,
                yaw: 90.0,
            }),
            battery: Some(mpcraft_vehicle::Battery {
                voltage: 12.6,
                current: 3.0,
                remaining: 70.0,
            }),
            distance_to_home: Some(250.0),
            gps: Some(mpcraft_vehicle::GpsStatus {
                fix_type: mpcraft_mavlink::MavGpsFixType::GPS_FIX_TYPE_3D_FIX,
                satellites: 11,
                hdop: 1.2,
            }),
            ..Telemetry::default()
        };

        assert_eq!(value_of(&telemetry, TelemetryField::Altitude), Some(120.0));
        assert_eq!(value_of(&telemetry, TelemetryField::Heading), Some(90.0));
        assert_eq!(value_of(&telemetry, TelemetryField::Satellites), Some(11.0));

        // A vehicle that has not said something is not guessed at.
        let empty = Telemetry::default();

        assert_eq!(value_of(&empty, TelemetryField::DistanceToHome), None);
    }

    #[test]
    fn a_plugin_that_was_chosen_and_then_left_alone_is_not_run_again() {
        let folder = PluginFolder::new("left");

        folder.write(
            "name = \"alarm\"\nversion = \"1.0.0\"\nmodule = \"alarm.wasm\"\ncapabilities = [\"notify\"]\n",
            NOTIFY_MODULE,
        );

        let mut runtime = PluginRuntime::new(folder.path());
        runtime.scan(&chosen(&["alarm"])).unwrap();
        runtime.open().unwrap();

        assert_eq!(runtime.info()[0].state, PluginState::Ready);

        runtime.set_enabled("alarm", false).unwrap();

        assert_eq!(runtime.info()[0].state, PluginState::NotRun);

        // The tick calls nothing now: the plugin was left alone.
        assert_eq!(runtime.tick(Instant::now()), 0);
    }
}
