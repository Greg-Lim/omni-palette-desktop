use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::extension::Modifier,
    domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
    theme::ThemeMode,
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
pub struct RuntimeConfigLoad {
    pub config: RuntimeConfig,
    pub user_config_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub activation: KeyboardShortcut,
    pub command_behavior: CommandBehavior,
    pub appearance: AppearanceConfig,
    pub startup: StartupConfig,
    pub github: GitHubExtensionSource,
}

impl RuntimeConfig {
    #[allow(dead_code)]
    pub fn load(appdata_config_path: Option<&Path>, dev_config_path: &Path) -> Self {
        Self::load_with_diagnostics(appdata_config_path, dev_config_path).config
    }

    pub fn load_with_diagnostics(
        appdata_config_path: Option<&Path>,
        dev_config_path: &Path,
    ) -> RuntimeConfigLoad {
        let mut user_config_error = None;

        if let Some(path) = appdata_config_path {
            if path.exists() {
                match RuntimeConfigFile::load(path) {
                    Ok(config) => {
                        return RuntimeConfigLoad {
                            config: config.into_runtime_config(),
                            user_config_error,
                        };
                    }
                    Err(err) => user_config_error = Some(err),
                }
            }
        }

        if let Ok(config) = DevConfigFile::load(dev_config_path) {
            return RuntimeConfigLoad {
                config: RuntimeConfig {
                    activation: config.activation.into_shortcut(),
                    ..RuntimeConfig::default()
                },
                user_config_error,
            };
        }

        RuntimeConfigLoad {
            config: RuntimeConfig::default(),
            user_config_error,
        }
    }

    pub fn save_user_config(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("Config path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Could not create config directory {}: {err}",
                parent.display()
            )
        })?;

        let file_config = RuntimeConfigFile::from(self);
        let content = toml::to_string_pretty(&file_config)
            .map_err(|err| format!("Could not serialize config: {err}"))?;
        let mut temp_file = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
            format!(
                "Could not create temporary config file in {}: {err}",
                parent.display()
            )
        })?;
        temp_file
            .write_all(content.as_bytes())
            .map_err(|err| format!("Could not write temporary config file: {err}"))?;
        temp_file
            .flush()
            .map_err(|err| format!("Could not flush temporary config file: {err}"))?;
        temp_file
            .persist(path)
            .map_err(|err| format!("Could not replace config file {}: {err}", path.display()))?;
        Ok(())
    }

    pub fn default_activation_shortcut() -> KeyboardShortcut {
        default_activation_shortcut()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            activation: default_activation_shortcut(),
            command_behavior: CommandBehavior::default(),
            appearance: AppearanceConfig::default(),
            startup: StartupConfig::default(),
            github: GitHubExtensionSource::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandBehavior {
    #[default]
    Execute,
    Guide,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub theme: ThemeMode,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GitHubExtensionSource {
    pub owner: String,
    pub repo: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_catalog_path")]
    pub catalog_path: String,
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
}

impl Default for GitHubExtensionSource {
    fn default() -> Self {
        Self {
            owner: "Greg-Lim".to_string(),
            repo: "omni-palette-desktop".to_string(),
            branch: default_branch(),
            catalog_path: default_catalog_path(),
            enabled: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RuntimeConfigFile {
    #[serde(default = "default_activation_config")]
    activation: ActivationConfig,
    #[serde(default)]
    appearance: AppearanceConfig,
    #[serde(default)]
    commands: RuntimeCommandConfig,
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
            command_behavior: self.commands.behavior,
            appearance: self.appearance,
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

impl From<&RuntimeConfig> for RuntimeConfigFile {
    fn from(config: &RuntimeConfig) -> Self {
        Self {
            activation: ActivationConfig::from_shortcut(config.activation),
            appearance: config.appearance,
            commands: RuntimeCommandConfig {
                behavior: config.command_behavior,
            },
            startup: config.startup,
            extensions: RuntimeExtensionsConfig {
                github: config.github.clone(),
            },
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RuntimeCommandConfig {
    #[serde(default)]
    behavior: CommandBehavior,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RuntimeExtensionsConfig {
    #[serde(default)]
    github: GitHubExtensionSource,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

    fn from_shortcut(shortcut: KeyboardShortcut) -> Self {
        let mut mods = Vec::new();
        if shortcut.modifier.control {
            mods.push(Modifier::Ctrl);
        }
        if shortcut.modifier.shift {
            mods.push(Modifier::Shift);
        }
        if shortcut.modifier.alt {
            mods.push(Modifier::Alt);
        }
        if shortcut.modifier.win {
            mods.push(Modifier::Win);
        }

        Self {
            mods,
            key: shortcut.key,
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
    "master".to_string()
}

fn default_catalog_path() -> String {
    "extensions/registry/catalog.v1.json".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeMode;

    #[test]
    fn default_activation_is_ctrl_shift_p() {
        let config = RuntimeConfig::default();

        assert!(config.activation.modifier.control);
        assert!(config.activation.modifier.shift);
        assert!(!config.activation.modifier.alt);
        assert_eq!(config.activation.key, Key::KeyP);
    }

    #[test]
    fn default_command_behavior_executes_commands() {
        let config = RuntimeConfig::default();

        assert_eq!(config.command_behavior, CommandBehavior::Execute);
    }

    #[test]
    fn default_appearance_uses_system_theme() {
        let config = RuntimeConfig::default();

        assert_eq!(config.appearance.theme, ThemeMode::System);
    }

    #[test]
    fn default_github_catalog_points_to_desktop_registry() {
        let config = RuntimeConfig::default();

        assert_eq!(config.github.owner, "Greg-Lim");
        assert_eq!(config.github.repo, "omni-palette-desktop");
        assert_eq!(config.github.branch, "master");
        assert_eq!(
            config.github.catalog_path,
            "extensions/registry/catalog.v1.json"
        );
        assert_eq!(
            config.github.catalog_url(),
            "https://raw.githubusercontent.com/Greg-Lim/omni-palette-desktop/master/extensions/registry/catalog.v1.json"
        );
        assert!(!config.github.enabled);
    }

    #[test]
    fn parses_appdata_runtime_config() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let path = root.path().join("config.toml");
        fs::write(
            &path,
            r#"
activation = { mods = ["ctrl", "alt"], key = "Space" }

[commands]
behavior = "guide"

[startup]
launch_on_login = true
start_hidden = true

[extensions.github]
owner = "Greg-Lim"
repo = "omni-palette-extensions"
branch = "main"
catalog_path = "dist/catalog.v1.json"
enabled = true
"#,
        )
        .expect("config should be written");

        let config = RuntimeConfig::load(Some(&path), Path::new("missing-dev-config.toml"));

        assert!(config.activation.modifier.control);
        assert!(config.activation.modifier.alt);
        assert_eq!(config.activation.key, Key::Space);
        assert_eq!(config.command_behavior, CommandBehavior::Guide);
        assert_eq!(config.appearance.theme, ThemeMode::System);
        assert!(config.startup.launch_on_login);
        assert_eq!(
            config.github.catalog_url(),
            "https://raw.githubusercontent.com/Greg-Lim/omni-palette-extensions/main/dist/catalog.v1.json"
        );
    }

    #[test]
    fn parses_appearance_theme_modes() {
        for (theme_text, expected) in [
            ("system", ThemeMode::System),
            ("light", ThemeMode::Light),
            ("dark", ThemeMode::Dark),
        ] {
            let root = tempfile::tempdir().expect("temp dir should be created");
            let path = root.path().join("config.toml");
            fs::write(
                &path,
                format!(
                    r#"
activation = {{ mods = ["ctrl", "shift"], key = "KeyP" }}

[appearance]
theme = "{theme_text}"
"#
                ),
            )
            .expect("config should be written");

            let config = RuntimeConfig::load(Some(&path), Path::new("missing-dev-config.toml"));

            assert_eq!(config.appearance.theme, expected);
        }
    }

    #[test]
    fn save_user_config_round_trips_runtime_settings() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let path = root.path().join("OmniPalette").join("config.toml");
        let config = RuntimeConfig {
            activation: KeyboardShortcut {
                modifier: HotkeyModifiers {
                    control: true,
                    shift: false,
                    alt: true,
                    win: false,
                },
                key: Key::Space,
            },
            command_behavior: CommandBehavior::Guide,
            appearance: AppearanceConfig {
                theme: ThemeMode::Light,
            },
            startup: StartupConfig {
                launch_on_login: false,
                start_hidden: true,
            },
            github: GitHubExtensionSource {
                owner: "Greg-Lim".to_string(),
                repo: "omni-palette-extensions".to_string(),
                branch: "main".to_string(),
                catalog_path: "dist/catalog.v1.json".to_string(),
                enabled: true,
            },
        };

        config.save_user_config(&path).expect("config should save");
        let loaded = RuntimeConfig::load(Some(&path), Path::new("missing-dev-config.toml"));

        assert_eq!(loaded, config);
    }

    #[test]
    fn invalid_user_config_reports_diagnostic_and_falls_back() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let path = root.path().join("config.toml");
        fs::write(&path, "activation =").expect("invalid config should be written");

        let loaded =
            RuntimeConfig::load_with_diagnostics(Some(&path), Path::new("missing-dev-config.toml"));

        assert!(loaded.user_config_error.is_some());
        assert_eq!(loaded.config, RuntimeConfig::default());
    }

    #[test]
    fn default_activation_shortcut_restores_ctrl_shift_p() {
        let shortcut = RuntimeConfig::default_activation_shortcut();

        assert!(shortcut.modifier.control);
        assert!(shortcut.modifier.shift);
        assert!(!shortcut.modifier.alt);
        assert_eq!(shortcut.key, Key::KeyP);
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
