use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    config::extension::Modifier,
    domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
};

const APP_DIR_NAME: &str = "OmniPalette";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub config_path: Option<PathBuf>,
    pub local_cache_root: Option<PathBuf>,
}

impl RuntimePaths {
    pub fn from_environment() -> Self {
        let config_path = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DIR_NAME).join(CONFIG_FILE_NAME));
        let local_cache_root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DIR_NAME));

        Self {
            config_path,
            local_cache_root,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub activation: KeyboardShortcut,
    pub startup: StartupConfig,
    pub github: GitHubExtensionSource,
}

impl RuntimeConfig {
    pub fn load(appdata_config_path: Option<&Path>, dev_config_path: &Path) -> Self {
        if let Some(path) = appdata_config_path {
            if let Ok(config) = RuntimeConfigFile::load(path) {
                return config.into_runtime_config();
            }
        }

        if let Ok(config) = DevConfigFile::load(dev_config_path) {
            return RuntimeConfig {
                activation: config.activation.into_shortcut(),
                ..RuntimeConfig::default()
            };
        }

        RuntimeConfig::default()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            activation: default_activation_shortcut(),
            startup: StartupConfig::default(),
            github: GitHubExtensionSource::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct StartupConfig {
    #[serde(default)]
    pub launch_on_login: bool,
    #[serde(default = "default_true")]
    pub start_hidden: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            launch_on_login: false,
            start_hidden: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubExtensionSource {
    pub owner: String,
    pub repo: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_catalog_path")]
    pub catalog_path: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl GitHubExtensionSource {
    pub fn catalog_url(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.owner,
            self.repo,
            self.branch,
            self.catalog_path.trim_start_matches('/')
        )
    }

    pub fn signature_url(&self) -> String {
        format!("{}.sig", self.catalog_url())
    }
}

impl Default for GitHubExtensionSource {
    fn default() -> Self {
        Self {
            owner: "Greg-Lim".to_string(),
            repo: "omni-palette-extensions".to_string(),
            branch: default_branch(),
            catalog_path: default_catalog_path(),
            public_key: String::new(),
            enabled: false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RuntimeConfigFile {
    #[serde(default = "default_activation_config")]
    activation: ActivationConfig,
    #[serde(default)]
    startup: StartupConfig,
    #[serde(default)]
    extensions: RuntimeExtensionsConfig,
}

impl RuntimeConfigFile {
    fn load(path: &Path) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|err| format!("Could not read config: {err}"))?;
        toml::from_str(&content).map_err(|err| format!("Could not parse config: {err}"))
    }

    fn into_runtime_config(self) -> RuntimeConfig {
        RuntimeConfig {
            activation: self.activation.into_shortcut(),
            startup: self.startup,
            github: self.extensions.github,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DevConfigFile {
    #[serde(default = "default_activation_config")]
    activation: ActivationConfig,
}

impl DevConfigFile {
    fn load(path: &Path) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|err| format!("Could not read dev config: {err}"))?;
        toml::from_str(&content).map_err(|err| format!("Could not parse dev config: {err}"))
    }
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeExtensionsConfig {
    #[serde(default)]
    github: GitHubExtensionSource,
}

#[derive(Debug, Clone, Deserialize)]
struct ActivationConfig {
    #[serde(default)]
    mods: Vec<Modifier>,
    key: Key,
}

impl ActivationConfig {
    fn into_shortcut(self) -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: self.mods.contains(&Modifier::Ctrl),
                shift: self.mods.contains(&Modifier::Shift),
                alt: self.mods.contains(&Modifier::Alt),
                win: self.mods.contains(&Modifier::Win),
            },
            key: self.key,
        }
    }
}

fn default_activation_config() -> ActivationConfig {
    ActivationConfig {
        mods: vec![Modifier::Ctrl, Modifier::Shift],
        key: Key::KeyP,
    }
}

fn default_activation_shortcut() -> KeyboardShortcut {
    default_activation_config().into_shortcut()
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_catalog_path() -> String {
    "catalog.v1.json".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_activation_is_ctrl_shift_p() {
        let config = RuntimeConfig::default();

        assert!(config.activation.modifier.control);
        assert!(config.activation.modifier.shift);
        assert!(!config.activation.modifier.alt);
        assert_eq!(config.activation.key, Key::KeyP);
    }

    #[test]
    fn parses_appdata_runtime_config() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let path = root.path().join("config.toml");
        fs::write(
            &path,
            r#"
activation = { mods = ["ctrl", "alt"], key = "Space" }

[startup]
launch_on_login = true
start_hidden = true

[extensions.github]
owner = "Greg-Lim"
repo = "omni-palette-extensions"
branch = "main"
catalog_path = "dist/catalog.v1.json"
public_key = "abc"
enabled = true
"#,
        )
        .expect("config should be written");

        let config = RuntimeConfig::load(Some(&path), Path::new("missing-dev-config.toml"));

        assert!(config.activation.modifier.control);
        assert!(config.activation.modifier.alt);
        assert_eq!(config.activation.key, Key::Space);
        assert!(config.startup.launch_on_login);
        assert_eq!(
            config.github.catalog_url(),
            "https://raw.githubusercontent.com/Greg-Lim/omni-palette-extensions/main/dist/catalog.v1.json"
        );
    }

    #[test]
    fn dev_config_can_supply_activation_without_matching_appdata_schema() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let dev_path = root.path().join("config.toml");
        fs::write(
            &dev_path,
            r#"
activation = { mods = ["ctrl", "shift"], key = "Space" }
extensions = ["extensions/chrome.toml"]
"#,
        )
        .expect("dev config should be written");

        let config = RuntimeConfig::load(None, &dev_path);

        assert!(config.activation.modifier.control);
        assert!(config.activation.modifier.shift);
        assert_eq!(config.activation.key, Key::Space);
    }
}
