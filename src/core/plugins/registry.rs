use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc},
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

        Self {
            plugins: Arc::new(plugins),
            applications: Arc::new(applications),
            type_text,
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
