use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

use log::warn;

use crate::core::plugins::{
    command::PluginApplication,
    runtime::{LoadedPlugin, TypeTextFn},
};
use crate::domain::action::Os;

const PLUGIN_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Arc<HashMap<String, Arc<LoadedPlugin>>>,
    applications: Arc<Vec<PluginApplication>>,
    executor_tx: mpsc::Sender<PluginRequest>,
    stats: Arc<PluginExecutionStats>,
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
        let type_text: TypeTextFn = Arc::new(|_| {});
        let plugins = Arc::new(HashMap::new());
        let executor_tx = spawn_plugin_executor(Arc::clone(&plugins), Arc::clone(&type_text));
        Self {
            plugins,
            applications: Arc::new(Vec::new()),
            executor_tx,
            stats: Arc::new(PluginExecutionStats::default()),
        }
    }
}

impl PluginRegistry {
    pub fn load(
        manifest_paths: impl IntoIterator<Item = PathBuf>,
        current_os: Os,
        type_text: TypeTextFn,
    ) -> Self {
        let mut plugins = HashMap::new();
        let mut applications = Vec::new();

        for manifest_path in manifest_paths {
            match LoadedPlugin::load(&manifest_path, current_os, Arc::clone(&type_text)) {
                Ok(plugin) => {
                    applications.push(plugin.application());
                    plugins.insert(plugin.id().to_string(), Arc::new(plugin));
                }
                Err(err) => warn!("Failed to load WASM plugin at {:?}: {}", manifest_path, err),
            }
        }
        let plugins = Arc::new(plugins);
        let executor_tx = spawn_plugin_executor(Arc::clone(&plugins), Arc::clone(&type_text));

        Self {
            plugins,
            applications: Arc::new(applications),
            executor_tx,
            stats: Arc::new(PluginExecutionStats::default()),
        }
    }

    #[cfg(test)]
    pub fn load_with_type_text_recorder(
        manifest_paths: impl IntoIterator<Item = PathBuf>,
        current_os: Os,
        typed_text: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> Self {
        Self::load(
            manifest_paths,
            current_os,
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
        self.stats.started.fetch_add(1, Ordering::Relaxed);
        log::debug!("Starting WASM plugin command: {plugin_id}:{command_id}");

        let (tx, rx) = mpsc::channel();
        let request = PluginRequest {
            plugin_id: plugin_id.to_string(),
            command_id: command_id.to_string(),
            response_tx: tx,
        };

        if self.executor_tx.send(request).is_err() {
            self.stats.failed.fetch_add(1, Ordering::Relaxed);
            return Err("WASM plugin executor is unavailable".to_string());
        }

        match rx.recv_timeout(PLUGIN_TIMEOUT) {
            Ok(Ok(())) => {
                self.stats.completed.fetch_add(1, Ordering::Relaxed);
                log::debug!("Completed WASM plugin command: {plugin_id}:{command_id}");
                Ok(())
            }
            Ok(Err(err)) => {
                self.stats.failed.fetch_add(1, Ordering::Relaxed);
                warn!("WASM plugin command failed: {plugin_id}:{command_id}: {err}");
                Err(err)
            }
            Err(_) => {
                self.stats.timed_out.fetch_add(1, Ordering::Relaxed);
                warn!("WASM plugin command timed out: {plugin_id}:{command_id}");
                Err(format!(
                    "WASM plugin command timed out: {plugin_id}:{command_id}"
                ))
            }
        }
    }

    pub fn execution_snapshot(&self) -> PluginExecutionSnapshot {
        PluginExecutionSnapshot {
            loaded_plugins: self.plugins.len(),
            registered_applications: self.applications.len(),
            started: self.stats.started.load(Ordering::Relaxed),
            completed: self.stats.completed.load(Ordering::Relaxed),
            failed: self.stats.failed.load(Ordering::Relaxed),
            timed_out: self.stats.timed_out.load(Ordering::Relaxed),
        }
    }
}

struct PluginRequest {
    plugin_id: String,
    command_id: String,
    response_tx: mpsc::Sender<Result<(), String>>,
}

#[derive(Default)]
struct PluginExecutionStats {
    started: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    timed_out: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginExecutionSnapshot {
    pub loaded_plugins: usize,
    pub registered_applications: usize,
    pub started: u64,
    pub completed: u64,
    pub failed: u64,
    pub timed_out: u64,
}

fn spawn_plugin_executor(
    plugins: Arc<HashMap<String, Arc<LoadedPlugin>>>,
    type_text: TypeTextFn,
) -> mpsc::Sender<PluginRequest> {
    let (tx, rx) = mpsc::channel::<PluginRequest>();

    thread::Builder::new()
        .name("plugin-executor".to_string())
        .spawn(move || {
            while let Ok(request) = rx.recv() {
                let result = plugins
                    .get(&request.plugin_id)
                    .cloned()
                    .ok_or_else(|| format!("Unknown WASM plugin: {}", request.plugin_id))
                    .and_then(|plugin| {
                        plugin.execute_sync(&request.command_id, Arc::clone(&type_text))
                    });
                let _ = request.response_tx.send(result);
            }
        })
        .expect("plugin executor thread should start");

    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    use crate::core::extensions::discovery::ExtensionDiscovery;

    fn real_plugin_manifests() -> Vec<PathBuf> {
        ExtensionDiscovery::new("./extensions/bundled").plugin_manifest_paths()
    }

    fn sample_plugin_wasm_path() -> PathBuf {
        Path::new("extensions")
            .join("bundled")
            .join("plugins")
            .join("auto_typer")
            .join("plugin.wasm")
    }

    #[test]
    fn loads_auto_typer_plugin_and_registers_command() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(
            real_plugin_manifests(),
            Os::Windows,
            typed,
        );
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
            real_plugin_manifests(),
            Os::Windows,
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
        let plugin_dir = root.join("plugins").join("no_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(sample_plugin_wasm_path(), plugin_dir.join("plugin.wasm"))
            .expect("should copy sample plugin wasm");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "no_permission"
name = "No Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = []

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            Arc::clone(&typed),
        );

        let err = registry
            .execute("no_permission", "type_hello_world")
            .expect_err("type_text should require permission");

        assert!(err.contains("non-zero exit code"));
        assert!(typed.lock().expect("typed text lock poisoned").is_empty());
    }

    #[test]
    fn skips_plugin_with_unknown_permission() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("unknown-permission");
        let plugin_dir = root.join("plugins").join("unknown_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(sample_plugin_wasm_path(), plugin_dir.join("plugin.wasm"))
            .expect("should copy sample plugin wasm");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "unknown_permission"
name = "Unknown Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = ["type_txt"]

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            typed,
        );

        assert!(registry.applications().is_empty());
    }

    #[test]
    fn skips_plugin_for_other_platform() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("wrong-platform");
        let plugin_dir = root.join("plugins").join("wrong_platform");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(sample_plugin_wasm_path(), plugin_dir.join("plugin.wasm"))
            .expect("should copy sample plugin wasm");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "wrong_platform"
name = "Wrong Platform"
platform = "macos"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = ["type_text"]

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_type_text_recorder(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            typed,
        );

        assert!(registry.applications().is_empty());
    }
}
