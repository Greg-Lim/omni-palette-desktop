use std::path::Path;

use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::core::extensions::settings::{
    validate_extension_settings_schema, ExtensionSettingsSchema,
};
#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;
use crate::core::plugins::{
    capabilities::{
        register_capabilities, PluginHostContext, PluginStoreState, ReadSettingsTextFn,
        ReadTimeTextFn, ResolvePluginStorageRootFn, WriteTextFn,
    },
    command::{PluginApplication, PluginCommand, RawCommandDescriptor},
    manifest::{PluginManifest, PluginSettingsSource},
};
use crate::domain::action::Os;

const COMMAND_ID_OFFSET: usize = 4096;
const PLUGIN_FUEL_BUDGET: u64 = 10_000_000;

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
        resolve_storage_root: ResolvePluginStorageRootFn,
        read_settings_text: ReadSettingsTextFn,
        #[cfg(debug_assertions)] write_performance_log: LogPerformanceSnapshotFn,
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

        let (engine, module) = load_engine_and_module(&wasm_path)?;
        let host_context = PluginHostContext {
            write_text,
            read_time_text,
            resolve_storage_root,
            read_settings_text,
            #[cfg(debug_assertions)]
            write_performance_log,
        };
        let _settings_schema =
            load_plugin_settings_schema_internal(&manifest, &engine, &module, &host_context)?;

        let mut plugin = Self {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            manifest,
            engine,
            module,
            commands: Vec::new(),
            host_context,
        };
        plugin.commands = plugin.register_commands()?;
        Ok(plugin)
    }

    pub(crate) fn load_settings_schema_from_manifest(
        manifest_path: &Path,
        current_os: Os,
        write_text: WriteTextFn,
        read_time_text: ReadTimeTextFn,
        resolve_storage_root: ResolvePluginStorageRootFn,
        read_settings_text: ReadSettingsTextFn,
        #[cfg(debug_assertions)] write_performance_log: LogPerformanceSnapshotFn,
    ) -> Result<Option<ExtensionSettingsSchema>, String> {
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
        let (engine, module) = load_engine_and_module(&wasm_path)?;
        let host_context = PluginHostContext {
            write_text,
            read_time_text,
            resolve_storage_root,
            read_settings_text,
            #[cfg(debug_assertions)]
            write_performance_log,
        };

        load_plugin_settings_schema_internal(&manifest, &engine, &module, &host_context)
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
        instantiate_plugin(
            &self.engine,
            &self.module,
            PluginStoreState {
                plugin_id: self.id.clone(),
                permissions: self.manifest.permissions.iter().cloned().collect(),
                host_context: self.host_context.clone(),
                allow_host_reads: true,
                allow_host_effects,
            },
        )
    }
}

fn load_engine_and_module(wasm_path: &Path) -> Result<(Engine, Module), String> {
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config).map_err(|err| format!("Could not create engine: {err}"))?;
    let module = Module::from_file(&engine, wasm_path)
        .map_err(|err| format!("Could not load plugin module {:?}: {err}", wasm_path))?;
    Ok((engine, module))
}

fn load_plugin_settings_schema_internal(
    manifest: &PluginManifest,
    engine: &Engine,
    module: &Module,
    host_context: &PluginHostContext,
) -> Result<Option<ExtensionSettingsSchema>, String> {
    let Some(settings) = manifest.settings else {
        return Ok(None);
    };

    match settings.source {
        PluginSettingsSource::Wasm => {
            let (mut store, instance) = instantiate_plugin(
                engine,
                module,
                PluginStoreState {
                    plugin_id: manifest.id.clone(),
                    permissions: manifest.permissions.iter().cloned().collect(),
                    host_context: host_context.clone(),
                    allow_host_reads: true,
                    allow_host_effects: false,
                },
            )?;
            let export = instance
                .get_typed_func::<(), i32>(&mut store, "settings_schema_json")
                .map_err(|err| format!("Missing settings_schema_json export: {err}"))?;
            let ptr = export
                .call(&mut store, ())
                .map_err(|err| format!("settings_schema_json failed: {err}"))?;
            let json = read_guest_c_string(&mut store, &instance, ptr as usize)?;
            let schema: ExtensionSettingsSchema = serde_json::from_str(&json)
                .map_err(|err| format!("Could not parse settings schema JSON: {err}"))?;
            Ok(Some(validate_extension_settings_schema(schema)?))
        }
    }
}

fn instantiate_plugin(
    engine: &Engine,
    module: &Module,
    initial_state: PluginStoreState,
) -> Result<(Store<PluginStoreState>, wasmtime::Instance), String> {
    let mut store = Store::new(engine, initial_state);
    store
        .set_fuel(PLUGIN_FUEL_BUDGET)
        .map_err(|err| format!("Could not set plugin fuel: {err}"))?;

    let mut linker = Linker::new(engine);
    register_capabilities(&mut linker)?;

    let instance = linker
        .instantiate(&mut store, module)
        .map_err(|err| format!("Could not instantiate plugin: {err}"))?;

    Ok((store, instance))
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
