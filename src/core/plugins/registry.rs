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

#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;
use crate::core::plugins::{
    capabilities::{ReadTimeTextFn, WriteTextFn},
    command::PluginApplication,
    runtime::LoadedPlugin,
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
        let plugins = Arc::new(HashMap::new());
        let executor_tx = spawn_plugin_executor(Arc::clone(&plugins));
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
        write_text: WriteTextFn,
        read_time_text: ReadTimeTextFn,
        #[cfg(debug_assertions)]
        write_performance_log: LogPerformanceSnapshotFn,
    ) -> Self {
        let mut plugins = HashMap::new();
        let mut applications = Vec::new();

        for manifest_path in manifest_paths {
            match LoadedPlugin::load(
                &manifest_path,
                current_os,
                Arc::clone(&write_text),
                Arc::clone(&read_time_text),
                #[cfg(debug_assertions)]
                Arc::clone(&write_performance_log),
            ) {
                Ok(plugin) => {
                    applications.push(plugin.application());
                    plugins.insert(plugin.id().to_string(), Arc::new(plugin));
                }
                Err(err) => warn!("Failed to load WASM plugin at {:?}: {}", manifest_path, err),
            }
        }
        let plugins = Arc::new(plugins);
        let executor_tx = spawn_plugin_executor(Arc::clone(&plugins));

        Self {
            plugins,
            applications: Arc::new(applications),
            executor_tx,
            stats: Arc::new(PluginExecutionStats::default()),
        }
    }

    #[cfg(test)]
    pub fn load_with_write_text_recorder(
        manifest_paths: impl IntoIterator<Item = PathBuf>,
        current_os: Os,
        typed_text: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> Self {
        Self::load_with_host_recorders(
            manifest_paths,
            current_os,
            typed_text,
            Arc::new(std::sync::Mutex::new(Vec::new())),
            Arc::new(std::sync::Mutex::new(Vec::new())),
        )
    }

    #[cfg(test)]
    pub fn load_with_host_recorders(
        manifest_paths: impl IntoIterator<Item = PathBuf>,
        current_os: Os,
        typed_text: Arc<std::sync::Mutex<Vec<String>>>,
        read_time_requests: Arc<std::sync::Mutex<Vec<String>>>,
        #[cfg(debug_assertions)]
        performance_logs: Arc<std::sync::Mutex<Vec<String>>>,
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
            Arc::new(move || {
                read_time_requests
                    .lock()
                    .expect("read time lock poisoned")
                    .push("read_time".to_string());
                Ok("6 Apr".to_string())
            }),
            #[cfg(debug_assertions)]
            Arc::new(move || {
                performance_logs
                    .lock()
                    .expect("performance log lock poisoned")
                    .push("performance snapshot".to_string());
                Ok(())
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
                    .and_then(|plugin| plugin.execute_sync(&request.command_id));
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

    fn sample_auto_typer_plugin_path() -> PathBuf {
        Path::new("extensions")
            .join("bundled")
            .join("plugins")
            .join("auto_typer")
            .join("plugin.wat")
    }

    fn sample_performance_plugin_path() -> PathBuf {
        Path::new("extensions")
            .join("bundled")
            .join("plugins")
            .join("performance_tracker")
            .join("plugin.wat")
    }

    #[test]
    fn loads_auto_typer_plugin_and_registers_command() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_write_text_recorder(
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
        assert_eq!(app.commands.len(), 2);
        assert_eq!(app.commands[0].id, "type_hello_world");
        assert_eq!(app.commands[0].name, "Type hello world");
        assert_eq!(app.commands[1].id, "type_current_date");
        assert_eq!(app.commands[1].name, "Type current date");
    }

    #[test]
    fn executes_auto_typer_plugin_through_host_write_text() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_write_text_recorder(
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
    fn executes_auto_typer_current_date_through_read_and_write_capabilities() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_time_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_host_recorders(
            real_plugin_manifests(),
            Os::Windows,
            Arc::clone(&typed),
            Arc::clone(&read_time_requests),
            Arc::new(std::sync::Mutex::new(Vec::new())),
        );

        registry
            .execute("auto_typer", "type_current_date")
            .expect("auto typer current date command should execute");

        assert_eq!(typed.lock().expect("typed text lock poisoned").as_slice(), ["6 Apr"]);
        assert_eq!(
            read_time_requests
                .lock()
                .expect("read time lock poisoned")
                .as_slice(),
            ["read_time"]
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    fn loads_performance_tracker_plugin_and_registers_command() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_time_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let performance_logs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_host_recorders(
            real_plugin_manifests(),
            Os::Windows,
            typed,
            read_time_requests,
            performance_logs,
        );
        let app = registry
            .applications()
            .iter()
            .find(|app| app.plugin_id == "performance_tracker")
            .expect("performance tracker plugin should load");

        assert_eq!(app.name, "Performance Tracker");
        assert_eq!(app.commands.len(), 1);
        assert_eq!(app.commands[0].id, "log_performance_snapshot");
        assert_eq!(app.commands[0].name, "Log performance snapshot");
    }

    #[test]
    #[cfg(debug_assertions)]
    fn executes_performance_tracker_through_host_logger() {
        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_time_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let performance_logs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_host_recorders(
            real_plugin_manifests(),
            Os::Windows,
            typed,
            read_time_requests,
            Arc::clone(&performance_logs),
        );

        registry
            .execute("performance_tracker", "log_performance_snapshot")
            .expect("performance tracker command should execute");

        assert_eq!(
            performance_logs
                .lock()
                .expect("performance log lock poisoned")
                .as_slice(),
            ["performance snapshot"]
        );
    }

    #[test]
    fn rejects_write_text_when_permission_is_missing() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("no-permission");
        let plugin_dir = root.join("plugins").join("no_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(sample_auto_typer_plugin_path(), plugin_dir.join("plugin.wat"))
            .expect("should copy sample plugin wat");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "no_permission"
name = "No Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
permissions = []

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_write_text_recorder(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            Arc::clone(&typed),
        );

        let err = registry
            .execute("no_permission", "type_hello_world")
            .expect_err("write_text should require permission");

        assert!(err.contains("non-zero exit code"));
        assert!(typed.lock().expect("typed text lock poisoned").is_empty());
    }

    #[test]
    #[cfg(debug_assertions)]
    fn rejects_performance_logging_when_permission_is_missing() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("no-performance-permission");
        let plugin_dir = root.join("plugins").join("no_performance_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(
            sample_performance_plugin_path(),
            plugin_dir.join("plugin.wat"),
        )
        .expect("should copy sample performance plugin");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "no_performance_permission"
name = "No Performance Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
permissions = []

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_time_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let performance_logs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_host_recorders(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            typed,
            read_time_requests,
            Arc::clone(&performance_logs),
        );

        let err = registry
            .execute("no_performance_permission", "log_performance_snapshot")
            .expect_err("performance logging should require permission");

        assert!(err.contains("non-zero exit code"));
        assert!(performance_logs
            .lock()
            .expect("performance log lock poisoned")
            .is_empty());
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
        fs::copy(sample_auto_typer_plugin_path(), plugin_dir.join("plugin.wat"))
            .expect("should copy sample plugin wat");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "unknown_permission"
name = "Unknown Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
permissions = ["type_txt"]

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_write_text_recorder(
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
        fs::copy(sample_auto_typer_plugin_path(), plugin_dir.join("plugin.wat"))
            .expect("should copy sample plugin wat");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "wrong_platform"
name = "Wrong Platform"
platform = "macos"
version = "0.1.0"
wasm = "plugin.wat"
permissions = ["write_text"]

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_write_text_recorder(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            typed,
        );

        assert!(registry.applications().is_empty());
    }

    #[test]
    fn rejects_read_time_when_permission_is_missing() {
        let root = Path::new("target")
            .join("plugin-tests")
            .join("no-read-time-permission");
        let plugin_dir = root.join("plugins").join("no_read_time_permission");
        if root.exists() {
            fs::remove_dir_all(&root).expect("should reset test plugin root");
        }
        fs::create_dir_all(&plugin_dir).expect("should create test plugin folder");
        fs::copy(sample_auto_typer_plugin_path(), plugin_dir.join("plugin.wat"))
            .expect("should copy sample plugin wat");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"id = "no_read_time_permission"
name = "No Read Time Permission"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
permissions = ["write_text"]

[app]
default_focus_state = "global"
"#,
        )
        .expect("should write test manifest");

        let typed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_time_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = PluginRegistry::load_with_host_recorders(
            ExtensionDiscovery::new(&root).plugin_manifest_paths(),
            Os::Windows,
            Arc::clone(&typed),
            Arc::clone(&read_time_requests),
            Arc::new(std::sync::Mutex::new(Vec::new())),
        );

        let err = registry
            .execute("no_read_time_permission", "type_current_date")
            .expect_err("read_time should require permission");

        assert!(err.contains("non-zero exit code"));
        assert!(typed.lock().expect("typed text lock poisoned").is_empty());
        assert!(read_time_requests
            .lock()
            .expect("read time lock poisoned")
            .is_empty());
    }
}
