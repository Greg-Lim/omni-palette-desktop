use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use log::{error, warn};
use serde::Deserialize;
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

use crate::domain::action::{CommandPriority, FocusState};

const COMMAND_ID_OFFSET: usize = 4096;
const PLUGIN_TIMEOUT: Duration = Duration::from_millis(750);

type TypeTextFn = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Arc<HashMap<String, Arc<LoadedPlugin>>>,
    applications: Arc<Vec<PluginApplication>>,
    type_text: TypeTextFn,
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("plugin_count", &self.plugins.len())
            .field("applications", &self.applications)
            .finish()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self {
            plugins: Arc::new(HashMap::new()),
            applications: Arc::new(Vec::new()),
            type_text: Arc::new(|_| {}),
        }
    }
}

impl PluginRegistry {
    pub fn load(extensions_folder: &Path, type_text: TypeTextFn) -> Self {
        let mut plugins = HashMap::new();
        let mut applications = Vec::new();

        let entries = match fs::read_dir(extensions_folder) {
            Ok(entries) => entries,
            Err(err) => {
                error!(
                    "Could not scan WASM plugin extensions at {:?}: {}",
                    extensions_folder, err
                );
                return Self {
                    plugins: Arc::new(plugins),
                    applications: Arc::new(applications),
                    type_text,
                };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match LoadedPlugin::load(&manifest_path, Arc::clone(&type_text)) {
                Ok(plugin) => {
                    applications.push(plugin.application());
                    plugins.insert(plugin.id.clone(), Arc::new(plugin));
                }
                Err(err) => warn!("Failed to load WASM plugin at {:?}: {}", manifest_path, err),
            }
        }

        Self {
            plugins: Arc::new(plugins),
            applications: Arc::new(applications),
            type_text,
        }
    }

    #[cfg(test)]
    pub fn load_with_type_text_recorder(
        extensions_folder: &Path,
        typed_text: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> Self {
        Self::load(
            extensions_folder,
            Arc::new(move |text| {
                typed_text
                    .lock()
                    .expect("typed text lock poisoned")
                    .push(text.to_string());
            }),
        )
    }

    pub fn applications(&self) -> &[PluginApplication] {
        &self.applications
    }

    pub fn execute(&self, plugin_id: &str, command_id: &str) -> Result<(), String> {
        let plugin = self
            .plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| format!("Unknown WASM plugin: {plugin_id}"))?;
        let command_id = command_id.to_string();
        let timeout_command_id = command_id.clone();
        let type_text = Arc::clone(&self.type_text);
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = tx.send(plugin.execute_sync(&command_id, type_text));
        });

        rx.recv_timeout(PLUGIN_TIMEOUT).map_err(|_| {
            format!("WASM plugin command timed out: {plugin_id}:{timeout_command_id}")
        })?
    }
}

#[derive(Debug, Clone)]
pub struct PluginApplication {
    pub plugin_id: String,
    pub name: String,
    pub process_name: String,
    pub commands: Vec<PluginCommand>,
}

#[derive(Debug, Clone)]
pub struct PluginCommand {
    pub id: String,
    pub name: String,
    pub priority: CommandPriority,
    pub focus_state: FocusState,
    pub starred: bool,
    pub tags: Vec<String>,
    pub shortcut_text: String,
}

struct LoadedPlugin {
    id: String,
    name: String,
    manifest: PluginManifest,
    engine: Engine,
    module: Module,
    commands: Vec<PluginCommand>,
}

impl LoadedPlugin {
    fn load(manifest_path: &Path, type_text: TypeTextFn) -> Result<Self, String> {
        let manifest_content = fs::read_to_string(manifest_path)
            .map_err(|err| format!("Could not read plugin manifest: {err}"))?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)
            .map_err(|err| format!("Could not parse plugin manifest: {err}"))?;

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
        };
        plugin.commands = plugin.register_commands(type_text)?;
        Ok(plugin)
    }

    fn application(&self) -> PluginApplication {
        PluginApplication {
            plugin_id: self.id.clone(),
            name: self.name.clone(),
            process_name: self.id.clone(),
            commands: self.commands.clone(),
        }
    }

    fn register_commands(&self, type_text: TypeTextFn) -> Result<Vec<PluginCommand>, String> {
        let (mut store, instance) = self.instantiate(type_text, false)?;
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

    fn execute_sync(&self, command_id: &str, type_text: TypeTextFn) -> Result<(), String> {
        let (mut store, instance) = self.instantiate(type_text, true)?;
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

    fn instantiate(
        &self,
        type_text: TypeTextFn,
        allow_host_effects: bool,
    ) -> Result<(Store<PluginStoreState>, wasmtime::Instance), String> {
        let mut store = Store::new(
            &self.engine,
            PluginStoreState {
                permissions: self.manifest.permissions.iter().cloned().collect(),
                type_text,
                allow_host_effects,
            },
        );
        store
            .set_fuel(1_000_000)
            .map_err(|err| format!("Could not set plugin fuel: {err}"))?;

        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                "env",
                "host_type_text",
                |mut caller: Caller<'_, PluginStoreState>, ptr: i32, len: i32| -> i32 {
                    if !caller.data().allow_host_effects
                        || !caller.data().permissions.contains("type_text")
                    {
                        return 1;
                    }

                    let Some(memory) = caller
                        .get_export("memory")
                        .and_then(|item| item.into_memory())
                    else {
                        return 2;
                    };
                    let data = memory.data(&caller);
                    let start = ptr.max(0) as usize;
                    let end = start.saturating_add(len.max(0) as usize);
                    let Some(bytes) = data.get(start..end) else {
                        return 3;
                    };
                    let Ok(text) = std::str::from_utf8(bytes) else {
                        return 4;
                    };

                    (caller.data().type_text)(text);
                    0
                },
            )
            .map_err(|err| format!("Could not define host_type_text: {err}"))?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|err| format!("Could not instantiate plugin: {err}"))?;

        Ok((store, instance))
    }
}

struct PluginStoreState {
    permissions: HashSet<String>,
    type_text: TypeTextFn,
    allow_host_effects: bool,
}

#[derive(Debug, Deserialize)]
struct PluginManifest {
    id: String,
    name: String,
    #[allow(dead_code)]
    version: String,
    wasm: PathBuf,
    #[serde(default)]
    permissions: Vec<String>,
    app: Option<PluginAppConfig>,
}

#[derive(Debug, Deserialize)]
struct PluginAppConfig {
    default_focus_state: Option<FocusState>,
    default_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawCommandDescriptor {
    id: String,
    name: String,
    priority: Option<CommandPriority>,
    focus_state: Option<FocusState>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    starred: bool,
    shortcut_text: Option<String>,
}

impl RawCommandDescriptor {
    fn into_plugin_command(self, app: Option<&PluginAppConfig>) -> PluginCommand {
        let mut tags = app
            .and_then(|app| app.default_tags.clone())
            .unwrap_or_default();
        tags.extend(self.tags);
        tags.sort();
        tags.dedup();

        PluginCommand {
            id: self.id,
            name: self.name,
            priority: self.priority.unwrap_or_default(),
            focus_state: self
                .focus_state
                .or_else(|| app.and_then(|app| app.default_focus_state))
                .unwrap_or(FocusState::Global),
            starred: self.starred,
            tags,
            shortcut_text: self.shortcut_text.unwrap_or_else(|| "WASM".to_string()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_auto_typer_plugin_and_registers_command() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry =
            PluginRegistry::load_with_type_text_recorder(Path::new("./extensions"), typed);
        let app = registry
            .applications()
            .iter()
            .find(|app| app.plugin_id == "auto_typer")
            .expect("auto typer plugin should load");

        assert_eq!(app.name, "Auto Typer");
        assert_eq!(app.commands.len(), 1);
        assert_eq!(app.commands[0].id, "type_hello_world");
        assert_eq!(app.commands[0].name, "Type hello world");
    }

    #[test]
    fn executes_auto_typer_plugin_through_host_type_text() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(
            Path::new("./extensions"),
            Arc::clone(&typed),
        );

        registry
            .execute("auto_typer", "type_hello_world")
            .expect("auto typer command should execute");

        assert_eq!(
            typed.lock().expect("typed text lock poisoned").as_slice(),
            ["hello world"]
        );
    }

    #[test]
    fn rejects_type_text_when_permission_is_missing() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("no-permission");
        let plugin_dir = root.join("no_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(
            Path::new("extensions")
                .join("auto_typer")
                .join("plugin.wasm"),
            plugin_dir.join("plugin.wasm"),
        )
        .expect("should copy sample plugin wasm");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "no_permission"
name = "No Permission"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = []

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(&root, Arc::clone(&typed));

        let err = registry
            .execute("no_permission", "type_hello_world")
            .expect_err("type_text should require permission");

        assert!(err.contains("non-zero exit code"));
        assert!(typed.lock().expect("typed text lock poisoned").is_empty());
    }
}
