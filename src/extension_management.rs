use std::{collections::HashSet, path::PathBuf};

use crate::{
    core::{
        extensions::{
            catalog::ExtensionKind,
            discovery::plugin_manifest_paths_in_root,
            extensions::load_config,
            install::{
                load_installed_state, set_bundled_extension_enabled,
                set_installed_extension_enabled, uninstall_installed_extension, BundledExtension,
                InstalledState, BUNDLED_SOURCE_ID,
            },
            settings::{extension_settings_key, load_static_extension_settings_schema},
        },
        plugins::manifest::PluginManifest,
    },
    domain::action::Os,
    runtime_state::OmniRuntimeState,
};

#[derive(Debug, Clone)]
pub struct ExtensionManagementSnapshot {
    pub install_root: Option<PathBuf>,
    pub install_root_error: Option<String>,
    pub bundled_extensions: Vec<BundledExtension>,
    pub installed_state: InstalledState,
    pub extension_settings_available: HashSet<String>,
}

pub fn extension_management_snapshot(runtime: &OmniRuntimeState) -> ExtensionManagementSnapshot {
    let install_root = runtime.user_extensions_root();
    let (installed_state, install_root_error) = match install_root.as_deref() {
        Some(root) => match load_installed_state(root) {
            Ok(state) => (state, None),
            Err(err) => (InstalledState::default(), Some(err.to_string())),
        },
        None => (
            InstalledState::default(),
            Some("APPDATA is not set, so Omni Palette cannot manage user extensions.".to_string()),
        ),
    };
    let bundled_extensions = bundled_extensions(
        &runtime.bundled_extensions_root(),
        runtime.current_os(),
        &installed_state,
    );
    let extension_settings_available = extension_settings_available(
        &bundled_extensions,
        &installed_state,
        install_root.as_deref(),
        runtime.current_os(),
    );

    ExtensionManagementSnapshot {
        install_root,
        install_root_error,
        bundled_extensions,
        installed_state,
        extension_settings_available,
    }
}

pub fn set_extension_enabled(
    runtime: &OmniRuntimeState,
    extension_id: &str,
    source_id: &str,
    enabled: bool,
) -> Result<(ExtensionManagementSnapshot, String), String> {
    let install_root = runtime.user_extensions_root().ok_or_else(|| {
        "APPDATA is not set, so Omni Palette cannot manage user extensions.".to_string()
    })?;
    let before = extension_management_snapshot(runtime);
    let message = if source_id == BUNDLED_SOURCE_ID {
        let extension = before
            .bundled_extensions
            .iter()
            .find(|extension| extension.id == extension_id)
            .ok_or_else(|| format!("Bundled extension not found: {extension_id}"))?;
        set_bundled_extension_enabled(&install_root, extension, enabled)
            .map_err(|err| err.to_string())?;
        extension_enabled_message(&extension.name, enabled)
    } else {
        set_installed_extension_enabled(&install_root, extension_id, source_id, enabled)
            .map_err(|err| err.to_string())?;
        extension_enabled_message(extension_id, enabled)
    };

    runtime.reload_extensions()?;
    Ok((extension_management_snapshot(runtime), message))
}

pub fn uninstall_extension(
    runtime: &OmniRuntimeState,
    extension_id: &str,
    source_id: &str,
) -> Result<(ExtensionManagementSnapshot, String), String> {
    if source_id == BUNDLED_SOURCE_ID {
        return Err("Bundled extensions can be disabled, but not uninstalled.".to_string());
    }

    let install_root = runtime.user_extensions_root().ok_or_else(|| {
        "APPDATA is not set, so Omni Palette cannot manage user extensions.".to_string()
    })?;
    uninstall_installed_extension(&install_root, extension_id, source_id)
        .map_err(|err| err.to_string())?;
    runtime.reload_extensions()?;
    Ok((
        extension_management_snapshot(runtime),
        format!("Uninstalled {extension_id}"),
    ))
}

pub fn bundled_extensions(
    bundled_root: &std::path::Path,
    current_os: Os,
    installed_state: &InstalledState,
) -> Vec<BundledExtension> {
    let mut extensions = bundled_static_extensions(bundled_root, current_os, installed_state);
    extensions.extend(bundled_plugin_extensions(
        bundled_root,
        current_os,
        installed_state,
    ));
    extensions.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| bundled_kind_sort_key(left.kind).cmp(&bundled_kind_sort_key(right.kind)))
            .then_with(|| left.id.cmp(&right.id))
    });
    extensions
}

fn bundled_static_extensions(
    bundled_root: &std::path::Path,
    current_os: Os,
    installed_state: &InstalledState,
) -> Vec<BundledExtension> {
    let static_root = bundled_root.join("static");
    let entries = match std::fs::read_dir(&static_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            log::warn!(
                "Could not scan bundled static extensions at {:?}: {}",
                static_root,
                err
            );
            return Vec::new();
        }
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                return None;
            }

            let config = match load_config(&path) {
                Ok(config) => config,
                Err(err) => {
                    log::warn!("Could not load bundled extension {:?}: {}", path, err);
                    return None;
                }
            };
            if config.platform != current_os {
                return None;
            }

            let enabled = installed_state
                .enabled_for(&config.app.id, BUNDLED_SOURCE_ID)
                .unwrap_or(true);

            Some(BundledExtension {
                id: config.app.id,
                name: config.app.name,
                version: format!("schema {}", config.version),
                platform: config.platform,
                kind: ExtensionKind::Static,
                installed_path: path,
                enabled,
            })
        })
        .collect()
}

fn bundled_plugin_extensions(
    bundled_root: &std::path::Path,
    current_os: Os,
    installed_state: &InstalledState,
) -> Vec<BundledExtension> {
    plugin_manifest_paths_in_root(bundled_root)
        .into_iter()
        .filter_map(|manifest_path| {
            let manifest = match PluginManifest::load(&manifest_path) {
                Ok(manifest) => manifest,
                Err(err) => {
                    log::warn!(
                        "Could not load bundled plugin manifest {:?}: {}",
                        manifest_path,
                        err
                    );
                    return None;
                }
            };
            if manifest.platform != current_os {
                return None;
            }

            let enabled = installed_state
                .enabled_for(&manifest.id, BUNDLED_SOURCE_ID)
                .unwrap_or(true);

            Some(BundledExtension {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                platform: manifest.platform,
                kind: ExtensionKind::WasmPlugin,
                installed_path: manifest_path,
                enabled,
            })
        })
        .collect()
}

fn bundled_kind_sort_key(kind: ExtensionKind) -> u8 {
    match kind {
        ExtensionKind::Static => 0,
        ExtensionKind::WasmPlugin => 1,
    }
}

fn extension_settings_available(
    bundled_extensions: &[BundledExtension],
    installed_state: &InstalledState,
    install_root: Option<&std::path::Path>,
    current_os: Os,
) -> HashSet<String> {
    let mut available = HashSet::new();

    for extension in bundled_extensions {
        if extension_exposes_settings(extension.kind, &extension.installed_path, current_os) {
            available.insert(extension_settings_key(&extension.id, BUNDLED_SOURCE_ID));
        }
    }

    for extension in installed_state
        .extensions
        .iter()
        .filter(|extension| extension.source_id != BUNDLED_SOURCE_ID)
    {
        let installed_path =
            resolve_extension_settings_path(install_root, &extension.installed_path);
        if extension_exposes_settings(extension.kind, &installed_path, current_os) {
            available.insert(extension_settings_key(&extension.id, &extension.source_id));
        }
    }

    available
}

fn extension_exposes_settings(kind: ExtensionKind, path: &std::path::Path, current_os: Os) -> bool {
    match kind {
        ExtensionKind::Static => load_static_extension_settings_schema(path)
            .map(|schema| schema.is_some_and(|schema| schema.has_items()))
            .unwrap_or_else(|err| {
                log::warn!(
                    "Could not load static extension settings schema at {:?}: {}",
                    path,
                    err
                );
                false
            }),
        ExtensionKind::WasmPlugin => PluginManifest::load(path)
            .map(|manifest| manifest.platform == current_os && manifest.settings.is_some())
            .unwrap_or_else(|err| {
                log::warn!(
                    "Could not load plugin manifest for extension settings at {:?}: {}",
                    path,
                    err
                );
                false
            }),
    }
}

fn resolve_extension_settings_path(
    install_root: Option<&std::path::Path>,
    installed_path: &std::path::Path,
) -> PathBuf {
    if installed_path.is_absolute() {
        installed_path.to_path_buf()
    } else if let Some(install_root) = install_root {
        install_root.join(installed_path)
    } else {
        installed_path.to_path_buf()
    }
}

fn extension_enabled_message(name: &str, enabled: bool) -> String {
    if enabled {
        format!("Enabled {name}")
    } else {
        format!("Disabled {name}")
    }
}
