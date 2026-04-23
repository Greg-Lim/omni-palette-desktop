use std::path::Path;

use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::core::plugins::{
    capabilities::{
        register_capabilities, PluginHostContext, PluginStoreState, ReadTimeTextFn, WriteTextFn,
    },
    command::{PluginApplication, PluginCommand, RawCommandDescriptor},
    manifest::PluginManifest,
};
#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;
use crate::domain::action::Os;

const COMMAND_ID_OFFSET: usize = 4096;

pub(crate) struct LoadedPlugin {
    id: String,
    name: String,
    manifest: PluginManifest,
    engine: Engine,
    module: Module,
    commands: Vec<PluginCommand>,
    host_context: PluginHostContext,
}

impl LoadedPlugin {
    pub(crate) fn load(
        manifest_path: &Path,
        current_os: Os,
        write_text: WriteTextFn,
        read_time_text: ReadTimeTextFn,
        #[cfg(debug_assertions)]
        write_performance_log: LogPerformanceSnapshotFn,
    ) -> Result<Self, String> {
        let manifest = PluginManifest::load(manifest_path)?;
        if manifest.platform != current_os {
            return Err(format!(
                "Plugin platform {:?} does not match current OS {:?}",
                manifest.platform, current_os
            ));
        }

        let plugin_dir = manifest_path
            .parent()
            .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
        let wasm_path = plugin_dir.join(&manifest.wasm);

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine =
            Engine::new(&config).map_err(|err| format!("Could not create engine: {err}"))?;
        let module = Module::from_file(&engine, &wasm_path)
            .map_err(|err| format!("Could not load plugin module {:?}: {err}", wasm_path))?;

        let mut plugin = Self {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            manifest,
            engine,
            module,
            commands: Vec::new(),
            host_context: PluginHostContext {
                write_text,
                read_time_text,
                #[cfg(debug_assertions)]
                write_performance_log,
            },
        };
        plugin.commands = plugin.register_commands()?;
        Ok(plugin)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn application(&self) -> PluginApplication {
        PluginApplication {
            plugin_id: self.id.clone(),
            name: self.name.clone(),
            process_name: self.id.clone(),
            commands: self.commands.clone(),
        }
    }

    pub(crate) fn execute_sync(&self, command_id: &str) -> Result<(), String> {
        let (mut store, instance) = self.instantiate(true)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| "Plugin does not export memory".to_string())?;
        let command_id_bytes = command_id.as_bytes();
        let memory_data = memory.data_mut(&mut store);
        let end = COMMAND_ID_OFFSET + command_id_bytes.len();
        if end > memory_data.len() {
            return Err("Plugin memory is too small for command id".to_string());
        }
        memory_data[COMMAND_ID_OFFSET..end].copy_from_slice(command_id_bytes);

        let execute = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "execute")
            .map_err(|err| format!("Missing execute export: {err}"))?;
        let exit_code = execute
            .call(
                &mut store,
                (COMMAND_ID_OFFSET as i32, command_id_bytes.len() as i32),
            )
            .map_err(|err| format!("Plugin execute failed: {err}"))?;

        if exit_code == 0 {
            Ok(())
        } else {
            Err(format!(
                "Plugin execute returned non-zero exit code: {exit_code}"
            ))
        }
    }

    fn register_commands(&self) -> Result<Vec<PluginCommand>, String> {
        let (mut store, instance) = self.instantiate(false)?;
        let register = instance
            .get_typed_func::<(), i32>(&mut store, "register_commands_json")
            .map_err(|err| format!("Missing register_commands_json export: {err}"))?;
        let ptr = register
            .call(&mut store, ())
            .map_err(|err| format!("register_commands_json failed: {err}"))?;
        let json = read_guest_c_string(&mut store, &instance, ptr as usize)?;
        let raw_commands: Vec<RawCommandDescriptor> = serde_json::from_str(&json)
            .map_err(|err| format!("Could not parse command descriptor JSON: {err}"))?;

        Ok(raw_commands
            .into_iter()
            .map(|command| command.into_plugin_command(self.manifest.app.as_ref()))
            .collect())
    }

    fn instantiate(
        &self,
        allow_host_effects: bool,
    ) -> Result<(Store<PluginStoreState>, wasmtime::Instance), String> {
        let mut store = Store::new(
            &self.engine,
            PluginStoreState {
                permissions: self.manifest.permissions.iter().cloned().collect(),
                host_context: self.host_context.clone(),
                allow_host_effects,
            },
        );
        store
            .set_fuel(1_000_000)
            .map_err(|err| format!("Could not set plugin fuel: {err}"))?;

        let mut linker = Linker::new(&self.engine);
        register_capabilities(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|err| format!("Could not instantiate plugin: {err}"))?;

        Ok((store, instance))
    }
}

fn read_guest_c_string(
    store: &mut Store<PluginStoreState>,
    instance: &wasmtime::Instance,
    ptr: usize,
) -> Result<String, String> {
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| "Plugin does not export memory".to_string())?;
    let data = memory.data(&*store);
    let bytes = data
        .get(ptr..)
        .ok_or_else(|| "Plugin returned an invalid string pointer".to_string())?;
    let len = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| "Plugin returned a non-null-terminated string".to_string())?;

    std::str::from_utf8(&bytes[..len])
        .map(str::to_string)
        .map_err(|err| format!("Plugin returned invalid UTF-8: {err}"))
}
