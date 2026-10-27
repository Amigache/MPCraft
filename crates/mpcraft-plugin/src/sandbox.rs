//! The sandbox: a module, the capabilities it was given, and the limits it runs inside.
//!
//! Two things make a plugin safe to run at all.
//!
//! The first is that a capability is not checked at call time — it is what the module can *import*.
//! A plugin that was not given commands never gets a function for commands, so loading a module that
//! asks for one fails before any of its code runs. A check somebody could forget is not a control.
//!
//! The second is that a plugin cannot take as much of the application as it likes. It gets a number
//! of steps per call, which is how a loop that never ends is stopped without stopping anything else,
//! a ceiling on the memory it may hold, and a ceiling on the stack its calls go through. A plugin that
//! runs out of either is an error with a code, not a hung window.
//!
//! The stages are called with nothing in and a number out: zero means it worked. That is the whole
//! ABI, and it is deliberately small — the smaller the surface, the less there is to trust.

use std::collections::HashSet;
use std::sync::Arc;

use wasmtime::{
    Caller, Config, Engine, Extern, Instance, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder,
};

use crate::api::{CommandIntent, HostApi, ParamIntent, TelemetryField};
use crate::capability::Capability;
use crate::error::PluginError;
use crate::lifecycle::Stage;
use crate::manifest::PluginManifest;

/// What the sandbox is willing to give a plugin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    /// How many steps one call may use. A plugin that needs more is doing work it should hand back.
    pub fuel: u64,

    /// How much memory a module may hold.
    pub max_memory_bytes: usize,

    /// How much stack a plugin may use, which is what a recursion that never returns is stopped by.
    pub max_wasm_stack_bytes: usize,

    /// How much of the module's memory one string may be read from, which is what stops a plugin
    /// from naming a length the size of its whole memory.
    pub max_text_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 1_000_000,
            max_memory_bytes: 1024 * 1024,
            max_wasm_stack_bytes: 64 * 1024,
            max_text_bytes: 512,
        }
    }
}

/// What a host function is given when it is called.
#[derive(Debug)]
struct HostData {
    plugin: String,
    api: Arc<dyn HostApi>,
    capabilities: HashSet<Capability>,
    max_text_bytes: usize,
    limits: StoreLimits,
}

/// The sandbox that runs plugins.
pub struct Sandbox {
    engine: Engine,
    limits: Limits,
}

impl Sandbox {
    /// Builds a runtime with the limits a plugin is run inside.
    pub fn new(limits: Limits) -> Result<Self, PluginError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.max_wasm_stack(limits.max_wasm_stack_bytes);

        let engine = match Engine::new(&config) {
            Ok(engine) => engine,
            Err(source) => {
                return Err(PluginError::Module {
                    plugin: "the sandbox".to_string(),
                    message: describe(&source),
                })
            }
        };

        Ok(Self { engine, limits })
    }

    /// Reads a module and puts it inside a store that holds only the functions the manifest was
    /// given.
    pub fn load(
        &self,
        manifest: &PluginManifest,
        bytes: &[u8],
        api: Arc<dyn HostApi>,
    ) -> Result<Plugin, PluginError> {
        let module = match Module::new(&self.engine, bytes) {
            Ok(module) => module,
            Err(source) => {
                return Err(PluginError::Module {
                    plugin: manifest.name.clone(),
                    message: describe(&source),
                })
            }
        };

        let data = HostData {
            plugin: manifest.name.clone(),
            api,
            capabilities: manifest.capabilities.iter().copied().collect(),
            max_text_bytes: self.limits.max_text_bytes,
            limits: StoreLimitsBuilder::new()
                .memory_size(self.limits.max_memory_bytes)
                .build(),
        };

        let mut store = Store::new(&self.engine, data);
        store.limiter(|data| &mut data.limits);

        if let Err(source) = store.set_fuel(self.limits.fuel) {
            return Err(PluginError::Instantiate {
                plugin: manifest.name.clone(),
                message: describe(&source),
            });
        }

        let linker = self.linker(&manifest.name, &manifest.capabilities)?;

        let instance = match linker.instantiate(&mut store, &module) {
            Ok(instance) => instance,
            Err(source) => {
                let message = describe(&source);

                // An import the manifest did not grant is named by what it was, so the operator can
                // see which capability the plugin asked for rather than reading a runtime message.
                if message.contains("unknown import") {
                    // The longer name is read first: a message about `set_param` contains `param`, and
                    // reading it as the read would name the wrong capability to the operator.
                    for name in [
                        "telemetry",
                        "command",
                        "mission_count",
                        "mission_current",
                        "note",
                        "set_param",
                        "param",
                    ] {
                        if message.contains(name) {
                            return Err(PluginError::NotGranted {
                                plugin: manifest.name.clone(),
                                capability: capability_of(name),
                            });
                        }
                    }
                }

                if message.contains("memory")
                    && (message.contains("denied") || message.contains("exceeds"))
                {
                    return Err(PluginError::Denied {
                        plugin: manifest.name.clone(),
                        resource: "memory",
                        max: self.limits.max_memory_bytes,
                    });
                }

                return Err(PluginError::Instantiate {
                    plugin: manifest.name.clone(),
                    message,
                });
            }
        };

        Ok(Plugin {
            manifest: manifest.clone(),
            store,
            instance,
            fuel: self.limits.fuel,
            max_wasm_stack_bytes: self.limits.max_wasm_stack_bytes,
        })
    }

    /// The functions a plugin may call: only the ones it was given.
    fn linker(
        &self,
        plugin: &str,
        capabilities: &[Capability],
    ) -> Result<Linker<HostData>, PluginError> {
        let granted: HashSet<Capability> = capabilities.iter().copied().collect();
        let mut linker = Linker::new(&self.engine);

        // A runtime failure here is not something a manifest could have gotten wrong, so it is
        // reported as the runtime's own words with the name of the plugin the operator can see.
        let error = |source: wasmtime::Error| PluginError::Instantiate {
            plugin: plugin.to_string(),
            message: describe(&source),
        };

        if granted.contains(&Capability::ReadTelemetry) {
            linker
                .func_wrap(
                    "host",
                    "telemetry",
                    |caller: Caller<'_, HostData>, field: i32| -> f64 {
                        let data = caller.data();

                        match TelemetryField::from_index(field) {
                            Some(known) => data.api.telemetry(known).unwrap_or(f64::NAN),
                            None => f64::NAN,
                        }
                    },
                )
                .map_err(error)?;
        }

        if granted.contains(&Capability::SendCommands) {
            linker
                .func_wrap(
                    "host",
                    "command",
                    |caller: Caller<'_, HostData>,
                     command: i32,
                     param: f64,
                     x: i32,
                     y: i32|
                     -> i32 {
                        let data = caller.data();

                        let intent = CommandIntent {
                            command: u32::try_from(command.max(0)).unwrap_or(0),
                            params: [param, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                            x,
                            y,
                            z: 0.0,
                        };

                        // The plugin is not allowed to know *why* the application said no, only
                        // that it did: a plugin that could read the reason could act on it.
                        match data.api.ask(intent) {
                            Ok(()) => 0,
                            Err(_) => 1,
                        }
                    },
                )
                .map_err(error)?;
        }

        if granted.contains(&Capability::MissionAccess) {
            linker
                .func_wrap(
                    "host",
                    "mission_count",
                    |caller: Caller<'_, HostData>| -> i32 {
                        caller.data().api.mission_count() as i32
                    },
                )
                .map_err(error)?;

            // A plan the vehicle has not reported is not a plan with nothing in it: the plugin sees the
            // number that says "nothing was said", which is not a place in the plan.
            linker
                .func_wrap(
                    "host",
                    "mission_current",
                    |caller: Caller<'_, HostData>| -> i32 {
                        match caller.data().api.mission_current() {
                            Some(sequence) => i32::try_from(sequence).unwrap_or(-1),
                            None => -1,
                        }
                    },
                )
                .map_err(error)?;
        }

        if granted.contains(&Capability::ReadParams) {
            linker
                .func_wrap(
                    "host",
                    "param",
                    |mut caller: Caller<'_, HostData>, offset: i32, length: i32| -> f64 {
                        let name = match read_text(&mut caller, offset, length) {
                            Some(name) => name,
                            None => return f64::NAN,
                        };

                        // A parameter the vehicle has not reported is not guessed at: the plugin sees
                        // the same nothing the window shows.
                        caller.data().api.param(&name).unwrap_or(f64::NAN)
                    },
                )
                .map_err(error)?;
        }

        if granted.contains(&Capability::WriteParams) {
            linker
                .func_wrap(
                    "host",
                    "set_param",
                    |mut caller: Caller<'_, HostData>,
                     offset: i32,
                     length: i32,
                     value: f64|
                     -> i32 {
                        let name = match read_text(&mut caller, offset, length) {
                            Some(name) => name,
                            None => return 1,
                        };

                        let intent = ParamIntent { name, value };

                        // The same rule as a command: the asking is queued, and the plugin cannot know
                        // why the application said no, only that it did.
                        match caller.data().api.ask_param(intent) {
                            Ok(()) => 0,
                            Err(_) => 1,
                        }
                    },
                )
                .map_err(error)?;
        }

        if granted.contains(&Capability::Notify) {
            linker
                .func_wrap(
                    "host",
                    "note",
                    |mut caller: Caller<'_, HostData>, offset: i32, length: i32| -> i32 {
                        let text = match read_text(&mut caller, offset, length) {
                            Some(text) => text,
                            None => return 1,
                        };

                        match caller.data().api.note(&text) {
                            Ok(()) => 0,
                            Err(_) => 1,
                        }
                    },
                )
                .map_err(error)?;
        }

        Ok(linker)
    }
}

/// A plugin that has been read and can be run through its stages.
#[derive(Debug)]
pub struct Plugin {
    manifest: PluginManifest,
    store: Store<HostData>,
    instance: Instance,
    /// The steps the plugin was given when it was loaded, which is what an operator needs to know it
    /// ran out of.
    fuel: u64,

    /// The stack the plugin was given, which is what a recursion that never returns ran out of.
    max_wasm_stack_bytes: usize,
}

impl Plugin {
    /// The manifest the plugin was read from.
    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// What the plugin was given, which is what the management sheet shows and what a change here
    /// would have to be checked against before a plugin is run again.
    #[must_use]
    pub fn capabilities(&self) -> &HashSet<Capability> {
        &self.store.data().capabilities
    }

    /// Runs one stage. A stage that is not there is skipped unless it is the one a plugin cannot be
    /// run without.
    pub fn run(&mut self, stage: Stage) -> Result<(), PluginError> {
        if self
            .instance
            .get_export(&mut self.store, stage.export())
            .is_none()
        {
            if stage.is_required() {
                return Err(PluginError::MissingExport {
                    plugin: self.plugin(),
                    export: stage.export().to_string(),
                });
            }

            return Ok(());
        }

        // Every stage takes nothing and returns a number, so a module that exports something of
        // another shape is not a plugin, and saying so here is better than letting the runtime call it
        // a type error.
        let func = match self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, stage.export())
        {
            Ok(func) => func,
            Err(source) => {
                return Err(PluginError::Instantiate {
                    plugin: self.plugin(),
                    message: describe(&source),
                })
            }
        };

        let code = match func.call(&mut self.store, ()) {
            Ok(code) => code,
            Err(source) => {
                // A plugin that used up its steps and a plugin that went deeper than the stack it was
                // given are two different things, and the runtime says which one happened, so the two
                // are told apart by what it says rather than by reading its words.
                match source.downcast_ref::<wasmtime::Trap>() {
                    Some(wasmtime::Trap::OutOfFuel) => {
                        return Err(PluginError::OutOfFuel {
                            plugin: self.plugin(),
                            fuel: self.fuel,
                        })
                    }
                    Some(wasmtime::Trap::StackOverflow) => {
                        return Err(PluginError::TooDeep {
                            plugin: self.plugin(),
                            max_wasm_stack_bytes: self.max_wasm_stack_bytes,
                        })
                    }
                    _ => {}
                }

                return Err(PluginError::Trap {
                    plugin: self.plugin(),
                    message: describe(&source),
                });
            }
        };

        if code != 0 {
            return Err(PluginError::StageFailed {
                plugin: self.plugin(),
                stage: stage.name(),
                code,
            });
        }

        Ok(())
    }

    fn plugin(&self) -> String {
        self.store.data().plugin.clone()
    }
}

/// A string the plugin supplies as bytes in its own memory, read with the ceiling one string has.
///
/// The ceiling is taken from the store before the bytes are read, because the same caller cannot be
/// borrowed for the export and for the memory at the same time. A length the module supplies is capped
/// before anything is read from it, because a plugin that names a length the size of its whole memory
/// would otherwise read the whole of it.
fn read_text(caller: &mut Caller<'_, HostData>, offset: i32, length: i32) -> Option<String> {
    let memory = caller.get_export("memory").and_then(Extern::into_memory)?;
    let cap = caller.data().max_text_bytes;

    // A length the module supplies is capped before anything is read from it.
    let wanted = cap.min(usize::try_from(length.max(0)).unwrap_or(cap));
    let start = usize::try_from(offset.max(0)).unwrap_or(0);

    let bytes = memory.data(caller).get(start..start + wanted)?.to_vec();

    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// The capability a host function belongs to, which is how an import that failed is named.
fn capability_of(name: &str) -> Capability {
    match name {
        "telemetry" => Capability::ReadTelemetry,
        "command" => Capability::SendCommands,
        "mission_count" => Capability::MissionAccess,
        "mission_current" => Capability::MissionAccess,
        "set_param" => Capability::WriteParams,
        "param" => Capability::ReadParams,
        _ => Capability::Notify,
    }
}

fn describe(source: &wasmtime::Error) -> String {
    // The runtime puts what happened under a heading, so the words underneath are what tell an
    // operator which plugin and which instruction the trouble is in.
    let mut text = source.to_string();
    let mut cause = source.source();

    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// An application surface that keeps what a plugin asked for.
    #[derive(Debug)]
    struct FakeApi {
        telemetry: Mutex<Vec<TelemetryField>>,
        commands: Mutex<Vec<u32>>,

        /// What the plugin read, in the order it read it.
        reads: Mutex<Vec<String>>,

        /// What the plugin asked to change.
        writes: Mutex<Vec<ParamIntent>>,
        notes: Mutex<Vec<String>>,

        /// What the vehicle said about a parameter, which is what a read answers with.
        values: HashMap<String, f64>,

        /// Where the vehicle says it is in the plan, which is what the plan shape answers with.
        current: Option<u32>,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                telemetry: Mutex::new(Vec::new()),
                commands: Mutex::new(Vec::new()),
                reads: Mutex::new(Vec::new()),
                writes: Mutex::new(Vec::new()),
                notes: Mutex::new(Vec::new()),
                values: HashMap::from([("FENCE_ALT_MAX".to_string(), 61.5)]),
                current: Some(12),
            }
        }
    }

    impl HostApi for FakeApi {
        fn telemetry(&self, field: TelemetryField) -> Option<f64> {
            self.telemetry.lock().push(field);
            Some(120.0)
        }

        fn ask(&self, intent: CommandIntent) -> Result<(), PluginError> {
            self.commands.lock().push(intent.command);
            Ok(())
        }

        fn mission_count(&self) -> u32 {
            4
        }

        fn mission_current(&self) -> Option<u32> {
            self.current
        }

        fn param(&self, name: &str) -> Option<f64> {
            self.reads.lock().push(name.to_string());
            self.values.get(name).copied()
        }

        fn ask_param(&self, intent: ParamIntent) -> Result<(), PluginError> {
            self.writes.lock().push(intent);
            Ok(())
        }

        fn note(&self, text: &str) -> Result<(), PluginError> {
            self.notes.lock().push(text.to_string());
            Ok(())
        }
    }

    fn manifest(capabilities: &[Capability]) -> PluginManifest {
        PluginManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            author: String::new(),
            module: "test.wasm".into(),
            capabilities: capabilities.to_vec(),
            loop_hz: 1.0,
        }
    }

    #[test]
    fn a_granted_call_reaches_the_surface_it_was_given() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        let wat = r#"
            (module
                (import "host" "telemetry" (func $telemetry (param i32) (result f64)))
                (memory (export "memory") 1)
                (func (export "init") (result i32)
                    i32.const 0
                    call $telemetry
                    drop
                    i32.const 0)
            )
        "#;

        let mut plugin = sandbox
            .load(
                &manifest(&[Capability::ReadTelemetry]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap();
        plugin.run(Stage::Init).unwrap();

        assert_eq!(*api.telemetry.lock(), vec![TelemetryField::Altitude]);
    }

    #[test]
    fn a_capability_that_was_not_granted_fails_before_any_code_runs() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        // The module asks for commands, which this manifest does not grant.
        let wat = r#"
            (module
                (import "host" "command" (func $command (param i32 f64 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (func (export "init") (result i32)
                    i32.const 208
                    f64.const 1
                    i32.const 0
                    i32.const 0
                    call $command
                    drop
                    i32.const 0)
            )
        "#;

        let error = sandbox
            .load(
                &manifest(&[Capability::ReadTelemetry]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "plugin_not_granted", "{error}");
        assert!(api.commands.lock().is_empty());
    }

    #[test]
    fn a_granted_parameter_read_reaches_the_values_the_vehicle_reported() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        let wat = r#"
            (module
                (import "host" "param" (func $param (param i32 i32) (result f64)))
                (memory (export "memory") 1)
                (data (i32.const 0) "FENCE_ALT_MAX")
                (func (export "init") (result i32)
                    i32.const 0
                    i32.const 13
                    call $param
                    drop
                    i32.const 0)
            )
        "#;

        let mut plugin = sandbox
            .load(
                &manifest(&[Capability::ReadParams]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap();
        plugin.run(Stage::Init).unwrap();

        assert_eq!(
            *api.reads.lock(),
            vec!["FENCE_ALT_MAX".to_string()],
            "the name the plugin wrote in its memory is what the surface was asked for"
        );
    }

    #[test]
    fn a_plugin_given_the_read_of_a_parameter_is_not_a_plugin_given_the_change_of_one() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        // The manifest grants the read, and the module asks for the change. Nothing of the module runs,
        // because the function it imports was never given to it.
        let wat = r#"
            (module
                (import "host" "set_param" (func $set_param (param i32 i32 f64) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "FENCE_ALT_MAX")
                (func (export "init") (result i32)
                    i32.const 0
                    i32.const 13
                    f64.const 61.5
                    call $set_param
                    drop
                    i32.const 0)
            )
        "#;

        let error = sandbox
            .load(
                &manifest(&[Capability::ReadParams]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "plugin_not_granted", "{error}");
        assert!(
            error.to_string().contains("write_params"),
            "the failure names the capability that was not given: {error}"
        );
        assert!(api.writes.lock().is_empty(), "nothing ran");
    }

    #[test]
    fn a_granted_parameter_write_reaches_the_surface_as_an_asking() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        let wat = r#"
            (module
                (import "host" "set_param" (func $set_param (param i32 i32 f64) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "FENCE_ALT_MAX")
                (func (export "init") (result i32)
                    i32.const 0
                    i32.const 13
                    f64.const 61.5
                    call $set_param
                    drop
                    i32.const 0)
            )
        "#;

        let mut plugin = sandbox
            .load(
                &manifest(&[Capability::WriteParams]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap();
        plugin.run(Stage::Init).unwrap();

        assert_eq!(
            *api.writes.lock(),
            vec![ParamIntent {
                name: "FENCE_ALT_MAX".to_string(),
                value: 61.5,
            }],
            "the name and the number the plugin supplied are what the surface holds"
        );
    }

    #[test]
    fn the_plan_shape_answers_with_the_item_the_vehicle_says_it_is_on() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        // The stage gives back what it read, so the number that comes through the stage failing is the
        // number the surface answered with — which is what proves the value crossed the boundary rather
        // than that a call was made.
        let wat = r#"
            (module
                (import "host" "mission_current" (func $current (result i32)))
                (func (export "init") (result i32)
                    call $current)
            )
        "#;

        let mut plugin = sandbox
            .load(
                &manifest(&[Capability::MissionAccess]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap();

        let error = plugin
            .run(Stage::Init)
            .expect_err("the stage returns what it read");

        assert!(
            matches!(error, PluginError::StageFailed { code: 12, .. }),
            "{error}"
        );
    }

    #[test]
    fn a_plan_shape_that_was_not_granted_fails_before_any_code_runs() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        // The manifest gives the plugin a way to say something, not the plan. The import it was not given
        // is named by the capability it belongs to, which is what the operator sees.
        let wat = r#"
            (module
                (import "host" "mission_current" (func $current (result i32)))
                (func (export "init") (result i32)
                    call $current
                    drop
                    i32.const 0)
            )
        "#;

        let error = sandbox
            .load(
                &manifest(&[Capability::Notify]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "plugin_not_granted", "{error}");
        assert!(error.to_string().contains("mission_access"), "{error}");
    }

    #[test]
    fn a_loop_that_never_ends_runs_out_of_the_work_it_was_given() {
        let limits = Limits {
            fuel: 200,
            ..Limits::default()
        };

        let sandbox = Sandbox::new(limits).unwrap();
        let api = Arc::new(FakeApi::new());

        // A loop that would run a million times: it stops on its own only if the sandbox lets it, so
        // what the test sees is the fuel running out, not the count being reached.
        let wat = r#"
            (module
                (func (export "loop") (result i32)
                    (local $count i32)
                    i32.const 0
                    local.set $count
                    (block $stop
                        (loop $again
                            local.get $count
                            i32.const 1
                            i32.add
                            local.set $count
                            local.get $count
                            i32.const 1000000
                            i32.lt_s
                            br_if $again))
                    local.get $count)
            )
        "#;

        let mut plugin = sandbox.load(&manifest(&[]), wat.as_bytes(), api).unwrap();
        let error = plugin.run(Stage::Loop).unwrap_err();

        assert_eq!(error.code(), "plugin_fuel", "{error}");
    }

    #[test]
    fn a_module_that_asks_for_more_memory_than_the_sandbox_gives_is_refused() {
        let limits = Limits {
            max_memory_bytes: 64 * 1024,
            ..Limits::default()
        };

        let sandbox = Sandbox::new(limits).unwrap();
        let api = Arc::new(FakeApi::new());

        // Six pages is 384 KiB, which is more than the 64 KiB this sandbox gives.
        let wat = r#"
            (module
                (memory (export "memory") 6)
                (func (export "init") (result i32) i32.const 0)
            )
        "#;

        let error = sandbox
            .load(&manifest(&[]), wat.as_bytes(), api)
            .unwrap_err();

        assert_eq!(error.code(), "plugin_limit", "{error}");
    }

    #[test]
    fn a_recursion_that_never_returns_is_stopped_by_the_stack_it_was_given() {
        let limits = Limits {
            max_wasm_stack_bytes: 16 * 1024,
            ..Limits::default()
        };

        let sandbox = Sandbox::new(limits).unwrap();
        let api = Arc::new(FakeApi::new());

        let wat = r#"
            (module
                (func $again (result i32)
                    call $again)
                (func (export "init") (result i32)
                    call $again)
            )
        "#;

        let mut plugin = sandbox.load(&manifest(&[]), wat.as_bytes(), api).unwrap();
        let error = plugin.run(Stage::Init).unwrap_err();

        assert_eq!(error.code(), "plugin_depth", "{error}");
    }

    #[test]
    fn the_first_stage_is_required_and_the_others_are_not() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        // A plugin that only has a tick: it can be run on the tick but not before it.
        let wat = r#"
            (module
                (func (export "loop") (result i32) i32.const 0)
            )
        "#;

        let mut plugin = sandbox.load(&manifest(&[]), wat.as_bytes(), api).unwrap();

        let missing = plugin.run(Stage::Init).unwrap_err();
        assert_eq!(missing.code(), "plugin_export", "{missing}");

        plugin.run(Stage::Loop).unwrap();
        plugin.run(Stage::Exit).unwrap();
    }

    #[test]
    fn a_stage_that_says_it_did_not_work_is_an_error() {
        let sandbox = Sandbox::new(Limits::default()).unwrap();
        let api = Arc::new(FakeApi::new());

        let wat = r#"
            (module
                (func (export "init") (result i32) i32.const 3)
            )
        "#;

        let mut plugin = sandbox.load(&manifest(&[]), wat.as_bytes(), api).unwrap();
        let error = plugin.run(Stage::Init).unwrap_err();

        assert_eq!(error.code(), "plugin_stage", "{error}");
        assert_eq!(
            error.to_string(),
            "test said the stage Init failed (code 3)"
        );
    }

    #[test]
    fn text_is_read_with_the_ceiling_the_sandbox_has() {
        let limits = Limits {
            max_text_bytes: 8,
            ..Limits::default()
        };

        let sandbox = Sandbox::new(limits).unwrap();
        let api = Arc::new(FakeApi::new());

        // The module writes a long word into its memory and asks for more than the sandbox will read.
        let wat = r#"
            (module
                (import "host" "note" (func $note (param i32 i32) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "hello there, everybody")
                (func (export "init") (result i32)
                    i32.const 0
                    i32.const 64
                    call $note
                    drop
                    i32.const 0)
            )
        "#;

        let mut plugin = sandbox
            .load(
                &manifest(&[Capability::Notify]),
                wat.as_bytes(),
                api.clone(),
            )
            .unwrap();
        plugin.run(Stage::Init).unwrap();

        assert_eq!(*api.notes.lock(), vec!["hello th".to_string()]);
    }
}
