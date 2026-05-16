use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::{
        ignore::{load_ignored_process_names, normalize_process_name},
        runtime::{CommandBehavior, RuntimeConfig, RuntimeConfigLoad, RuntimePaths},
    },
    core::{
        extensions::discovery::{user_extensions_root, ExtensionDiscovery},
        registry::registry::MasterRegistry,
    },
    domain::action::Os,
};

pub type SharedRegistry = Arc<RwLock<MasterRegistry>>;
pub type SharedIgnoredProcessNames = Arc<RwLock<HashSet<String>>>;

#[derive(Debug, Clone)]
pub struct RuntimeStateLoadOptions {
    pub bundled_extensions_root: PathBuf,
    pub user_extensions_root: Option<PathBuf>,
    pub dev_config_path: PathBuf,
    pub runtime_paths: RuntimePaths,
    pub current_os: Os,
}

impl RuntimeStateLoadOptions {
    pub fn from_environment(bundled_extensions_root: impl AsRef<Path>, current_os: Os) -> Self {
        let bundled_extensions_root = bundled_extensions_root.as_ref().to_path_buf();
        Self {
            dev_config_path: dev_config_path_for_bundled_root(&bundled_extensions_root),
            user_extensions_root: user_extensions_root(),
            bundled_extensions_root,
            runtime_paths: RuntimePaths::from_environment(),
            current_os,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OmniRuntimeState {
    registry: SharedRegistry,
    ignored_process_names: SharedIgnoredProcessNames,
    runtime_config_load: Arc<RwLock<RuntimeConfigLoad>>,
    runtime_paths: RuntimePaths,
    bundled_extensions_root: PathBuf,
    user_extensions_root: Option<PathBuf>,
    current_os: Os,
}

impl OmniRuntimeState {
    pub fn load(options: RuntimeStateLoadOptions) -> Self {
        let discovery = extension_discovery(
            &options.bundled_extensions_root,
            options.user_extensions_root.as_deref(),
        );
        let registry = MasterRegistry::build(&discovery, options.current_os);
        let ignored_process_names =
            load_ignored_process_names(&discovery.ignore_file_path(), options.current_os);
        let runtime_config_load = RuntimeConfig::load_with_diagnostics(
            options.runtime_paths.config_path.as_deref(),
            &options.dev_config_path,
        );

        Self {
            registry: Arc::new(RwLock::new(registry)),
            ignored_process_names: Arc::new(RwLock::new(ignored_process_names)),
            runtime_config_load: Arc::new(RwLock::new(runtime_config_load)),
            runtime_paths: options.runtime_paths,
            bundled_extensions_root: options.bundled_extensions_root,
            user_extensions_root: options.user_extensions_root,
            current_os: options.current_os,
        }
    }

    pub fn registry(&self) -> SharedRegistry {
        Arc::clone(&self.registry)
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config_load().config
    }

    pub fn config_load(&self) -> RuntimeConfigLoad {
        self.runtime_config_load
            .read()
            .map(|load| load.clone())
            .unwrap_or_else(|err| RuntimeConfigLoad {
                config: RuntimeConfig::default(),
                user_config_error: Some(format!("Runtime config lock poisoned: {err}")),
            })
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        self.runtime_paths.config_path.clone()
    }

    pub fn save_runtime_config(&self, config: RuntimeConfig) -> Result<String, String> {
        let path = self.runtime_paths.config_path.as_ref().ok_or_else(|| {
            "APPDATA is not set, so Omni Palette cannot save user settings.".to_string()
        })?;

        config.save_user_config(path)?;

        let mut runtime_config_load = self
            .runtime_config_load
            .write()
            .map_err(|err| format!("Runtime config lock poisoned: {err}"))?;
        *runtime_config_load = RuntimeConfigLoad {
            config,
            user_config_error: None,
        };

        Ok("Settings saved".to_string())
    }

    pub fn is_ignored_process_name(&self, process_name: &str) -> bool {
        let Some(process_name) = normalize_process_name(process_name) else {
            return false;
        };

        self.ignored_process_names
            .read()
            .map(|ignored| ignored.contains(&process_name))
            .unwrap_or(false)
    }

    pub fn status(&self) -> RuntimeStatusDto {
        let runtime_config_load = self.config_load();
        let (application_count, plugin_count, plugin_application_count) = self
            .registry
            .read()
            .map(|registry| {
                let plugin_snapshot = registry.plugin_registry().execution_snapshot();
                (
                    registry.application_registry.len(),
                    plugin_snapshot.loaded_plugins,
                    plugin_snapshot.registered_applications,
                )
            })
            .unwrap_or((0, 0, 0));

        let ignored_process_count = self
            .ignored_process_names
            .read()
            .map(|ignored| ignored.len())
            .unwrap_or(0);

        RuntimeStatusDto {
            config_path: self
                .runtime_paths
                .config_path
                .as_ref()
                .map(|path| path.display().to_string()),
            config_error: runtime_config_load.user_config_error,
            activation_hint: runtime_config_load.config.activation.to_string(),
            command_behavior: runtime_config_load.config.command_behavior,
            application_count,
            ignored_process_count,
            plugin_count,
            plugin_application_count,
        }
    }

    pub fn reload_extensions(&self) -> Result<ReloadReport, String> {
        let discovery = extension_discovery(
            &self.bundled_extensions_root,
            self.user_extensions_root.as_deref(),
        );
        let new_registry = MasterRegistry::build_strict(&discovery, self.current_os)
            .map_err(|err| err.to_string())?;
        let application_count = new_registry.application_registry.len();
        let plugin_snapshot = new_registry.plugin_registry().execution_snapshot();
        let new_ignored_process_names =
            load_ignored_process_names(&discovery.ignore_file_path(), self.current_os);
        let ignored_process_count = new_ignored_process_names.len();

        {
            let mut registry = self
                .registry
                .write()
                .map_err(|err| format!("Extension registry lock poisoned: {err}"))?;
            *registry = new_registry;
        }

        {
            let mut ignored_process_names = self
                .ignored_process_names
                .write()
                .map_err(|err| format!("Ignored process registry lock poisoned: {err}"))?;
            *ignored_process_names = new_ignored_process_names;
        }

        Ok(ReloadReport {
            application_count,
            ignored_process_count,
            plugin_count: plugin_snapshot.loaded_plugins,
            plugin_application_count: plugin_snapshot.registered_applications,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatusDto {
    pub config_path: Option<String>,
    pub config_error: Option<String>,
    pub activation_hint: String,
    pub command_behavior: CommandBehavior,
    pub application_count: usize,
    pub ignored_process_count: usize,
    pub plugin_count: usize,
    pub plugin_application_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReloadReport {
    pub application_count: usize,
    pub ignored_process_count: usize,
    pub plugin_count: usize,
    pub plugin_application_count: usize,
}

fn extension_discovery(
    bundled_extensions_root: &Path,
    user_extensions_root: Option<&Path>,
) -> ExtensionDiscovery {
    let mut roots = vec![bundled_extensions_root.to_path_buf()];
    if let Some(user_root) = user_extensions_root {
        roots.push(user_root.to_path_buf());
    }
    ExtensionDiscovery::with_roots(roots)
}

fn dev_config_path_for_bundled_root(bundled_extensions_root: &Path) -> PathBuf {
    bundled_extensions_root
        .parent()
        .and_then(Path::parent)
        .map(|repo_root| repo_root.join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::runtime::{CommandBehavior, GitHubExtensionSource, RuntimePaths, ThemeMode},
        domain::action::Os,
    };

    #[test]
    fn ignored_process_lookup_normalizes_configured_names() {
        let root = runtime_test_root("ignored-process-lookup");
        std::fs::write(root.join("ignore.toml"), "windows = [\"Code.exe\"]")
            .expect("ignore config should be written");

        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("config.toml"),
            runtime_paths: RuntimePaths {
                config_path: None,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });

        assert!(runtime.is_ignored_process_name("CODE.EXE"));
        assert!(!runtime.is_ignored_process_name("notepad.exe"));
        assert!(!runtime.is_ignored_process_name(""));
    }

    #[test]
    fn saving_runtime_config_updates_shared_snapshot_and_status() {
        let root = runtime_test_root("save-runtime-config");
        let config_path = root.join("config.toml");
        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path: Some(config_path.clone()),
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });
        let mut next_config = runtime.config();
        next_config.command_behavior = CommandBehavior::Guide;
        next_config.appearance.theme = ThemeMode::Light;
        next_config.github = GitHubExtensionSource {
            owner: "Example".to_string(),
            repo: "omni-extensions".to_string(),
            branch: "stable".to_string(),
            catalog_path: "catalog.json".to_string(),
            enabled: true,
        };

        let message = runtime
            .save_runtime_config(next_config.clone())
            .expect("config save should succeed");

        assert_eq!(message, "Settings saved");
        assert_eq!(runtime.config(), next_config);
        assert_eq!(runtime.status().command_behavior, CommandBehavior::Guide);
        let saved = std::fs::read_to_string(config_path).expect("config should be written");
        assert!(saved.contains("behavior = \"guide\""));
        assert!(saved.contains("theme = \"light\""));
        assert!(saved.contains("owner = \"Example\""));
    }

    #[test]
    fn saving_runtime_config_without_config_path_fails_without_updating_snapshot() {
        let root = runtime_test_root("save-runtime-config-missing-path");
        let runtime = OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path: None,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        });
        let original = runtime.config();
        let mut next_config = original.clone();
        next_config.command_behavior = CommandBehavior::Guide;

        let err = runtime
            .save_runtime_config(next_config)
            .expect_err("missing path should fail");

        assert_eq!(
            err,
            "APPDATA is not set, so Omni Palette cannot save user settings."
        );
        assert_eq!(runtime.config(), original);
    }

    fn runtime_test_root(name: &str) -> PathBuf {
        let root = PathBuf::from("target")
            .join("runtime-state-tests")
            .join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("runtime test root should reset");
        }
        std::fs::create_dir_all(root.join("static")).expect("static dir should be created");
        root
    }
}
