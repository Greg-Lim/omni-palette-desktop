use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use log::warn;

use crate::core::extensions::install::{load_installed_state, BUNDLED_SOURCE_ID};

const IGNORE_FILE_NAME: &str = "ignore.toml";
const INSTALLED_FILE_NAME: &str = "installed.toml";
const STATIC_DIR_NAME: &str = "static";
const SOURCES_FILE_NAME: &str = "sources.toml";
const PLUGINS_DIR_NAME: &str = "plugins";
const PLUGIN_MANIFEST_NAME: &str = "plugin.toml";
#[cfg(not(debug_assertions))]
const DEBUG_ONLY_PLUGIN_IDS: &[&str] = &["performance_tracker"];

#[derive(Debug, Clone)]
pub struct ExtensionDiscovery {
    roots: Vec<PathBuf>,
}

impl ExtensionDiscovery {
    #[cfg(test)]
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            roots: vec![root.as_ref().to_path_buf()],
        }
    }

    pub fn bundled_with_user_root(root: impl AsRef<Path>) -> Self {
        let mut roots = vec![root.as_ref().to_path_buf()];
        if let Some(user_root) = user_extensions_root() {
            roots.push(user_root);
        }
        Self { roots }
    }

    #[cfg(test)]
    pub fn with_roots(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
        }
    }

    pub fn ignore_file_path(&self) -> PathBuf {
        self.primary_root().join(IGNORE_FILE_NAME)
    }

    pub fn static_config_paths(&self) -> Vec<PathBuf> {
        let mut merged = BTreeMap::<OsString, PathBuf>::new();
        let bundled_disabled_ids = disabled_bundled_extension_ids_from_user_roots(&self.roots);

        for (root_index, root) in self.roots.iter().enumerate() {
            let static_paths = toml_files_in(&root.join(STATIC_DIR_NAME), false);
            let mut disabled_ids = disabled_installed_extension_ids(root);
            if root_index == 0 {
                disabled_ids.extend(bundled_disabled_ids.iter().cloned());
            }
            let static_file_names: HashSet<_> = static_paths
                .iter()
                .filter_map(|path| path.file_name().map(|file_name| file_name.to_os_string()))
                .collect();

            for path in toml_files_in(root, true)
                .into_iter()
                .filter(|path| is_enabled_static_path(path, &disabled_ids))
                .filter(|path| {
                    path.file_name()
                        .is_none_or(|file_name| !static_file_names.contains(file_name))
                })
            {
                if let Some(file_name) = path.file_name() {
                    merged.insert(file_name.to_os_string(), path);
                }
            }

            for path in static_paths
                .into_iter()
                .filter(|path| is_enabled_static_path(path, &disabled_ids))
            {
                if let Some(file_name) = path.file_name() {
                    merged.insert(file_name.to_os_string(), path);
                }
            }
        }

        merged.into_values().collect()
    }

    pub fn plugin_manifest_paths(&self) -> Vec<PathBuf> {
        let mut merged = BTreeMap::<OsString, PathBuf>::new();
        let bundled_disabled_ids = disabled_bundled_extension_ids_from_user_roots(&self.roots);

        for (root_index, root) in self.roots.iter().enumerate() {
            for manifest_path in plugin_manifest_paths_in_root(root) {
                if let Some(plugin_id) = manifest_path.parent().and_then(|path| path.file_name()) {
                    if root_index == 0
                        && plugin_id
                            .to_str()
                            .is_some_and(|plugin_id| bundled_disabled_ids.contains(plugin_id))
                    {
                        continue;
                    }
                    merged.insert(plugin_id.to_os_string(), manifest_path);
                }
            }
        }

        merged.into_values().collect()
    }

    fn primary_root(&self) -> &Path {
        self.roots
            .first()
            .map(PathBuf::as_path)
            .unwrap_or(Path::new("."))
    }
}

pub fn user_extensions_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|appdata| appdata.join("OmniPalette").join("extensions"))
}

pub(crate) fn plugin_manifest_paths_in_root(root: &Path) -> Vec<PathBuf> {
    let plugins_root = root.join(PLUGINS_DIR_NAME);
    let entries = match fs::read_dir(&plugins_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            warn!(
                "Could not scan plugin directory at {:?}: {}",
                plugins_root, err
            );
            return Vec::new();
        }
    };

    let mut manifest_paths = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let manifest_path = path.join(PLUGIN_MANIFEST_NAME);
            if !manifest_path.exists() {
                return None;
            }

            #[cfg(not(debug_assertions))]
            {
                let plugin_id = path.file_name()?.to_str()?;
                if DEBUG_ONLY_PLUGIN_IDS.contains(&plugin_id) {
                    return None;
                }
            }

            Some(manifest_path)
        })
        .collect::<Vec<_>>();
    manifest_paths.sort();
    manifest_paths
}

fn disabled_installed_extension_ids(root: &Path) -> HashSet<String> {
    load_installed_state(root)
        .map(|state| {
            state
                .extensions
                .into_iter()
                .filter(|extension| extension.source_id != BUNDLED_SOURCE_ID && !extension.enabled)
                .map(|extension| extension.id)
                .collect()
        })
        .unwrap_or_default()
}

fn disabled_bundled_extension_ids_from_user_roots(roots: &[PathBuf]) -> HashSet<String> {
    roots
        .iter()
        .skip(1)
        .filter_map(|root| load_installed_state(root).ok())
        .flat_map(|state| state.disabled_bundled_extension_ids())
        .collect()
}

fn is_enabled_static_path(path: &Path, disabled_ids: &HashSet<String>) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_none_or(|id| !disabled_ids.contains(id))
}

fn toml_files_in(folder: &Path, exclude_ignore_file: bool) -> Vec<PathBuf> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            warn!(
                "Could not scan extension config directory at {:?}: {}",
                folder, err
            );
            return Vec::new();
        }
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if !path.is_file() {
                return None;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                return None;
            }
            if exclude_ignore_file {
                let file_name = path.file_name().and_then(|file_name| file_name.to_str());
                if matches!(
                    file_name,
                    Some(IGNORE_FILE_NAME) | Some(INSTALLED_FILE_NAME) | Some(SOURCES_FILE_NAME)
                ) {
                    return None;
                }
            }

            Some(path)
        })
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_dir(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("test directory should reset");
        }
        fs::create_dir_all(path).expect("test directory should be created");
    }

    #[test]
    fn discovers_static_configs_and_root_fallback_configs() {
        let root = Path::new("target")
            .join("extension-discovery-tests")
            .join("static-configs");
        reset_dir(&root);
        fs::create_dir_all(root.join("static")).expect("static directory should be created");
        fs::write(root.join("ignore.toml"), "").expect("ignore config should be written");
        fs::write(root.join("legacy.toml"), "").expect("legacy config should be written");
        fs::write(root.join("chrome.toml"), "").expect("duplicate fallback should be written");
        fs::write(root.join("static").join("chrome.toml"), "")
            .expect("static config should be written");

        let discovery = ExtensionDiscovery::new(&root);
        let paths = discovery.static_config_paths();

        assert!(paths.contains(&root.join("static").join("chrome.toml")));
        assert!(paths.contains(&root.join("legacy.toml")));
        assert!(!paths.contains(&root.join("chrome.toml")));
        assert!(!paths.contains(&root.join("ignore.toml")));
    }

    #[test]
    fn discovers_plugin_manifests_under_plugins_folder() {
        let root = Path::new("target")
            .join("extension-discovery-tests")
            .join("plugins");
        reset_dir(&root);
        fs::create_dir_all(root.join("plugins").join("auto_typer"))
            .expect("plugin directory should be created");
        fs::write(
            root.join("plugins").join("auto_typer").join("plugin.toml"),
            "",
        )
        .expect("plugin manifest should be written");
        fs::create_dir_all(root.join("legacy_plugin")).expect("legacy folder should be created");
        fs::write(root.join("legacy_plugin").join("plugin.toml"), "")
            .expect("legacy manifest should be written");

        let discovery = ExtensionDiscovery::new(&root);

        assert_eq!(
            discovery.plugin_manifest_paths(),
            vec![root.join("plugins").join("auto_typer").join("plugin.toml")]
        );
    }

    #[test]
    fn disabled_bundled_plugin_in_user_state_suppresses_bundled_plugin() {
        let bundled = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-bundled-plugin-bundled");
        let user = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-bundled-plugin-user");
        reset_dir(&bundled);
        reset_dir(&user);
        fs::create_dir_all(bundled.join("plugins").join("ahk_agent"))
            .expect("bundled plugin dir should be created");
        fs::write(
            bundled
                .join("plugins")
                .join("ahk_agent")
                .join("plugin.toml"),
            "",
        )
        .expect("bundled plugin manifest should be written");
        fs::write(
            user.join("installed.toml"),
            r#"
[[extensions]]
id = "ahk_agent"
version = "0.1.0"
platform = "windows"
kind = "wasm_plugin"
source_id = "bundled"
package_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
enabled = false
installed_path = "plugins/ahk_agent/plugin.toml"
"#,
        )
        .expect("installed state should be written");

        let discovery = ExtensionDiscovery::with_roots(vec![bundled, user]);

        assert!(discovery.plugin_manifest_paths().is_empty());
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn skips_debug_only_plugin_manifests() {
        let root = Path::new("target")
            .join("extension-discovery-tests")
            .join("debug-only-plugins");
        reset_dir(&root);
        fs::create_dir_all(root.join("plugins").join("performance_tracker"))
            .expect("debug plugin directory should be created");
        fs::write(
            root.join("plugins")
                .join("performance_tracker")
                .join("plugin.toml"),
            "",
        )
        .expect("debug plugin manifest should be written");

        let discovery = ExtensionDiscovery::new(&root);

        assert!(discovery.plugin_manifest_paths().is_empty());
    }

    #[test]
    fn later_roots_override_earlier_static_configs_by_file_name() {
        let bundled = Path::new("target")
            .join("extension-discovery-tests")
            .join("bundled-root");
        let user = Path::new("target")
            .join("extension-discovery-tests")
            .join("user-root");
        reset_dir(&bundled);
        reset_dir(&user);
        fs::create_dir_all(bundled.join("static")).expect("bundled static should be created");
        fs::create_dir_all(user.join("static")).expect("user static should be created");
        fs::write(bundled.join("static").join("chrome.toml"), "")
            .expect("bundled chrome should be written");
        fs::write(user.join("static").join("chrome.toml"), "")
            .expect("user chrome should be written");

        let discovery = ExtensionDiscovery::with_roots(vec![bundled, user.clone()]);

        assert_eq!(
            discovery.static_config_paths(),
            vec![user.join("static").join("chrome.toml")]
        );
    }

    #[test]
    fn disabled_user_static_config_does_not_override_bundled_config() {
        let bundled = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-bundled-root");
        let user = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-user-root");
        reset_dir(&bundled);
        reset_dir(&user);
        fs::create_dir_all(bundled.join("static")).expect("bundled static should be created");
        fs::create_dir_all(user.join("static")).expect("user static should be created");
        fs::write(bundled.join("static").join("chrome.toml"), "")
            .expect("bundled chrome should be written");
        fs::write(user.join("static").join("chrome.toml"), "")
            .expect("user chrome should be written");
        fs::write(
            user.join("installed.toml"),
            r#"
[[extensions]]
id = "chrome"
version = "1.0.0"
platform = "windows"
kind = "static"
source_id = "official"
package_sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
enabled = false
installed_path = "static/chrome.toml"
"#,
        )
        .expect("installed state should be written");

        let discovery = ExtensionDiscovery::with_roots(vec![bundled.clone(), user]);

        assert_eq!(
            discovery.static_config_paths(),
            vec![bundled.join("static").join("chrome.toml")]
        );
    }

    #[test]
    fn disabled_bundled_config_in_user_state_suppresses_bundled_config() {
        let bundled = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-bundled-via-user-state-bundled");
        let user = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-bundled-via-user-state-user");
        reset_dir(&bundled);
        reset_dir(&user);
        fs::create_dir_all(bundled.join("static")).expect("bundled static should be created");
        fs::write(bundled.join("static").join("windows.toml"), "")
            .expect("bundled windows should be written");
        fs::write(
            user.join("installed.toml"),
            r#"
[[extensions]]
id = "windows"
version = "0.1.0"
platform = "windows"
kind = "static"
source_id = "bundled"
package_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
enabled = false
installed_path = "static/windows.toml"
"#,
        )
        .expect("installed state should be written");

        let discovery = ExtensionDiscovery::with_roots(vec![bundled, user]);

        assert!(discovery.static_config_paths().is_empty());
    }

    #[test]
    fn disabled_user_static_config_does_not_suppress_bundled_config_with_same_id() {
        let bundled = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-user-same-id-bundled");
        let user = Path::new("target")
            .join("extension-discovery-tests")
            .join("disabled-user-same-id-user");
        reset_dir(&bundled);
        reset_dir(&user);
        fs::create_dir_all(bundled.join("static")).expect("bundled static should be created");
        fs::write(bundled.join("static").join("windows.toml"), "")
            .expect("bundled windows should be written");
        fs::write(
            user.join("installed.toml"),
            r#"
[[extensions]]
id = "windows"
version = "0.1.0"
platform = "windows"
kind = "static"
source_id = "github"
package_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
enabled = false
installed_path = "static/windows.toml"
"#,
        )
        .expect("installed state should be written");

        let discovery = ExtensionDiscovery::with_roots(vec![bundled.clone(), user]);

        assert_eq!(
            discovery.static_config_paths(),
            vec![bundled.join("static").join("windows.toml")]
        );
    }
}
