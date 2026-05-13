use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::{
        ignore::load_ignored_process_names,
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
    pub fn from_environment(
        bundled_extensions_root: impl AsRef<Path>,
        current_os: Os,
    ) -> Self {
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
    runtime_config_load: RuntimeConfigLoad,
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
            runtime_config_load,
            runtime_paths: options.runtime_paths,
            bundled_extensions_root: options.bundled_extensions_root,
            user_extensions_root: options.user_extensions_root,
            current_os: options.current_os,
        }
    }

    pub fn registry(&self) -> SharedRegistry {
        Arc::clone(&self.registry)
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.runtime_config_load.config
    }

    pub fn status(&self) -> RuntimeStatusDto {
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
            config_error: self.runtime_config_load.user_config_error.clone(),
            activation_hint: self.runtime_config_load.config.activation.to_string(),
            command_behavior: self.runtime_config_load.config.command_behavior,
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
