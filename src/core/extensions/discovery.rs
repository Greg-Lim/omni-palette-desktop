use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use log::warn;

const IGNORE_FILE_NAME: &str = ".ignore.toml";
const STATIC_DIR_NAME: &str = "static";
const PLUGINS_DIR_NAME: &str = "plugins";
const PLUGIN_MANIFEST_NAME: &str = "plugin.toml";

#[derive(Debug, Clone)]
pub struct ExtensionDiscovery {
    root: PathBuf,
}

impl ExtensionDiscovery {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn ignore_file_path(&self) -> PathBuf {
        self.root.join(IGNORE_FILE_NAME)
    }

    pub fn static_config_paths(&self) -> Vec<PathBuf> {
        let mut paths = toml_files_in(&self.root.join(STATIC_DIR_NAME), false);
        let static_file_names: HashSet<_> = paths
            .iter()
            .filter_map(|path| path.file_name().map(|file_name| file_name.to_os_string()))
            .collect();

        paths.extend(toml_files_in(&self.root, true).into_iter().filter(|path| {
            path.file_name()
                .is_none_or(|file_name| !static_file_names.contains(file_name))
        }));
        paths
    }

    pub fn plugin_manifest_paths(&self) -> Vec<PathBuf> {
        let plugins_root = self.root.join(PLUGINS_DIR_NAME);
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

        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }

                let manifest_path = path.join(PLUGIN_MANIFEST_NAME);
                manifest_path.exists().then_some(manifest_path)
            })
            .collect();
        paths.sort();
        paths
    }
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
            if exclude_ignore_file
                && path.file_name().and_then(|file_name| file_name.to_str())
                    == Some(IGNORE_FILE_NAME)
            {
                return None;
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
        fs::write(root.join(".ignore.toml"), "").expect("ignore config should be written");
        fs::write(root.join("legacy.toml"), "").expect("legacy config should be written");
        fs::write(root.join("chrome.toml"), "").expect("duplicate fallback should be written");
        fs::write(root.join("static").join("chrome.toml"), "")
            .expect("static config should be written");

        let discovery = ExtensionDiscovery::new(&root);
        let paths = discovery.static_config_paths();

        assert!(paths.contains(&root.join("static").join("chrome.toml")));
        assert!(paths.contains(&root.join("legacy.toml")));
        assert!(!paths.contains(&root.join("chrome.toml")));
        assert!(!paths.contains(&root.join(".ignore.toml")));
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
}
