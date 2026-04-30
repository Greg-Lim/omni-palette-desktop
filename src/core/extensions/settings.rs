use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::extension::{
        Config, ExtensionSettingCategoryConfig, ExtensionSettingConfig, ExtensionSettingTypeConfig,
    },
    core::extensions::catalog::ExtensionKind,
    core::extensions::extensions::load_config,
};

const SETTINGS_DIRECTORY_NAME: &str = "settings";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSettingsTarget {
    pub extension_id: String,
    pub source_id: String,
    pub display_name: String,
    pub kind: ExtensionKind,
    pub installed_path: PathBuf,
}

impl ExtensionSettingsTarget {
    pub fn key(&self) -> String {
        extension_settings_key(&self.extension_id, &self.source_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedExtensionSettings {
    pub target: ExtensionSettingsTarget,
    pub schema: ExtensionSettingsSchema,
    pub values: ExtensionSettingsValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedExtensionSettings {
    pub target: ExtensionSettingsTarget,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSettingsSchema {
    #[serde(default)]
    pub categories: Vec<ExtensionSettingsCategory>,
    #[serde(default)]
    pub items: Vec<ExtensionSettingItem>,
}

impl ExtensionSettingsSchema {
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSettingsCategory {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_key: Option<String>,
    #[serde(default)]
    pub default_collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSettingItem {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "type")]
    pub kind: ExtensionSettingKind,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSettingKind {
    Toggle,
}

pub type ExtensionSettingsValues = BTreeMap<String, bool>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ExtensionSettingsFile {
    #[serde(default)]
    toggles: ExtensionSettingsValues,
}

pub fn extension_settings_key(extension_id: &str, source_id: &str) -> String {
    format!("{source_id}/{extension_id}")
}

pub fn load_static_extension_settings_schema(
    path: &Path,
) -> Result<Option<ExtensionSettingsSchema>, String> {
    let config = load_config(path)?;
    static_extension_settings_schema(&config)
}

pub fn static_extension_settings_schema(
    config: &Config,
) -> Result<Option<ExtensionSettingsSchema>, String> {
    let schema = ExtensionSettingsSchema {
        categories: config
            .setting_categories
            .iter()
            .map(extension_setting_category_from_config)
            .collect::<Vec<_>>(),
        items: config
            .settings
            .iter()
            .map(extension_setting_item_from_config)
            .collect::<Vec<_>>(),
    };

    if !schema.has_items() {
        return Ok(None);
    }

    Ok(Some(validate_extension_settings_schema(schema)?))
}

pub fn validate_extension_settings_schema(
    schema: ExtensionSettingsSchema,
) -> Result<ExtensionSettingsSchema, String> {
    let mut seen_category_keys = HashSet::new();
    let mut seen_keys = HashSet::new();

    for category in &schema.categories {
        if category.key.trim().is_empty() {
            return Err("Extension setting category key must not be empty".to_string());
        }
        if category.label.trim().is_empty() {
            return Err(format!(
                "Extension setting category label must not be empty for key '{}'",
                category.key
            ));
        }
        if let Some(toggle_key) = &category.toggle_key {
            if toggle_key.trim().is_empty() {
                return Err(format!(
                    "Extension setting category '{}' has an empty toggle_key",
                    category.key
                ));
            }
        }
        if !seen_category_keys.insert(category.key.clone()) {
            return Err(format!(
                "Duplicate extension setting category key '{}'",
                category.key
            ));
        }
    }

    for item in &schema.items {
        if item.key.trim().is_empty() {
            return Err("Extension setting key must not be empty".to_string());
        }
        if item.label.trim().is_empty() {
            return Err(format!(
                "Extension setting label must not be empty for key '{}'",
                item.key
            ));
        }
        if !seen_keys.insert(item.key.clone()) {
            return Err(format!("Duplicate extension setting key '{}'", item.key));
        }
        if let Some(category_key) = &item.category {
            if !seen_category_keys.contains(category_key) {
                return Err(format!(
                    "Extension setting '{}' references unknown category '{}'",
                    item.key, category_key
                ));
            }
        }
    }

    for category in &schema.categories {
        let Some(toggle_key) = &category.toggle_key else {
            continue;
        };
        let item = schema
            .items
            .iter()
            .find(|item| item.key == *toggle_key)
            .ok_or_else(|| {
                format!(
                    "Extension setting category '{}' references missing toggle_key '{}'",
                    category.key, toggle_key
                )
            })?;
        if item.category.as_deref() != Some(category.key.as_str()) {
            return Err(format!(
                "Extension setting category '{}' toggle_key '{}' must reference an item in the same category",
                category.key, toggle_key
            ));
        }
    }

    Ok(schema)
}

pub fn load_extension_settings_values(
    install_root: &Path,
    extension_id: &str,
) -> Result<ExtensionSettingsValues, String> {
    let path = extension_settings_file_path(install_root, extension_id);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExtensionSettingsValues::default());
        }
        Err(err) => {
            return Err(format!(
                "Could not read extension settings {}: {err}",
                path.display()
            ));
        }
    };

    let settings_file: ExtensionSettingsFile = toml::from_str(&content).map_err(|err| {
        format!(
            "Could not parse extension settings {}: {err}",
            path.display()
        )
    })?;
    Ok(settings_file.toggles)
}

pub fn save_extension_settings_values(
    install_root: &Path,
    extension_id: &str,
    values: &ExtensionSettingsValues,
) -> Result<(), String> {
    let settings_root = extension_settings_root(install_root);
    fs::create_dir_all(&settings_root).map_err(|err| {
        format!(
            "Could not create extension settings directory {}: {err}",
            settings_root.display()
        )
    })?;

    let file_path = extension_settings_file_path(install_root, extension_id);
    let staging_path = settings_root.join(format!("{extension_id}.toml.settings"));
    let file = ExtensionSettingsFile {
        toggles: values.clone(),
    };
    let content = toml::to_string_pretty(&file)
        .map_err(|err| format!("Could not serialize extension settings: {err}"))?;

    fs::write(&staging_path, content).map_err(|err| {
        format!(
            "Could not write extension settings {}: {err}",
            staging_path.display()
        )
    })?;
    fs::rename(&staging_path, &file_path).map_err(|err| {
        format!(
            "Could not replace extension settings {}: {err}",
            file_path.display()
        )
    })?;
    Ok(())
}

pub fn resolved_extension_settings_values(
    schema: &ExtensionSettingsSchema,
    stored_values: &ExtensionSettingsValues,
) -> ExtensionSettingsValues {
    schema
        .items
        .iter()
        .map(|item| {
            (
                item.key.clone(),
                stored_values
                    .get(&item.key)
                    .copied()
                    .unwrap_or(item.default),
            )
        })
        .collect()
}

pub fn extension_settings_json(install_root: &Path, extension_id: &str) -> Result<String, String> {
    let values = load_extension_settings_values(install_root, extension_id)?;
    serde_json::to_string(&values)
        .map_err(|err| format!("Could not serialize extension settings as JSON: {err}"))
}

fn extension_setting_item_from_config(config: &ExtensionSettingConfig) -> ExtensionSettingItem {
    ExtensionSettingItem {
        key: config.key.clone(),
        label: config.label.clone(),
        description: config.description.clone(),
        category: config.category.clone(),
        kind: match config.setting_type {
            ExtensionSettingTypeConfig::Toggle => ExtensionSettingKind::Toggle,
        },
        default: config.default,
    }
}

fn extension_setting_category_from_config(
    config: &ExtensionSettingCategoryConfig,
) -> ExtensionSettingsCategory {
    ExtensionSettingsCategory {
        key: config.key.clone(),
        label: config.label.clone(),
        description: config.description.clone(),
        toggle_key: config.toggle_key.clone(),
        default_collapsed: config.default_collapsed,
    }
}

fn extension_settings_root(install_root: &Path) -> PathBuf {
    install_root.join(SETTINGS_DIRECTORY_NAME)
}

fn extension_settings_file_path(install_root: &Path, extension_id: &str) -> PathBuf {
    extension_settings_root(install_root).join(format!("{extension_id}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        config::extension::{ActionConfig, ActionWhenConfig, AppConfig, CommandBinding},
        domain::{action::Os, hotkey::Key},
    };

    fn sample_config(
        setting_categories: Vec<ExtensionSettingCategoryConfig>,
        settings: Vec<ExtensionSettingConfig>,
    ) -> Config {
        Config {
            version: 2,
            platform: Os::Windows,
            app: AppConfig {
                id: "sample".to_string(),
                name: "Sample".to_string(),
                process_name: "sample.exe".to_string(),
                default_focus_state: None,
                default_tags: None,
            },
            actions: [(
                "sample_action".to_string(),
                ActionConfig {
                    name: "Sample Action".to_string(),
                    focus_state: None,
                    when: Some(ActionWhenConfig { any: Vec::new() }),
                    priority: None,
                    tags: None,
                    favorite: None,
                    cmd: CommandBinding::Shortcut(crate::config::extension::KeyChord {
                        mods: Vec::new(),
                        key: Key::KeyA,
                    }),
                },
            )]
            .into_iter()
            .collect(),
            setting_categories,
            settings,
        }
    }

    fn category(
        key: &str,
        label: &str,
        toggle_key: Option<&str>,
    ) -> ExtensionSettingCategoryConfig {
        ExtensionSettingCategoryConfig {
            key: key.to_string(),
            label: label.to_string(),
            description: None,
            toggle_key: toggle_key.map(|value| value.to_string()),
            default_collapsed: false,
        }
    }

    fn toggle_setting(
        key: &str,
        label: &str,
        category: Option<&str>,
        default: bool,
    ) -> ExtensionSettingConfig {
        ExtensionSettingConfig {
            key: key.to_string(),
            label: label.to_string(),
            description: None,
            category: category.map(|value| value.to_string()),
            setting_type: ExtensionSettingTypeConfig::Toggle,
            default,
        }
    }

    #[test]
    fn missing_settings_file_returns_empty_toggle_map() {
        let temp = tempfile::tempdir().expect("temp dir should be created");

        let values = load_extension_settings_values(temp.path(), "ahk_agent")
            .expect("missing settings file should be treated as empty");

        assert!(values.is_empty());
    }

    #[test]
    fn saves_and_loads_extension_settings_values() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let values = ExtensionSettingsValues::from([
            ("script_enabled".to_string(), true),
            ("command_enabled".to_string(), false),
        ]);

        save_extension_settings_values(temp.path(), "ahk_agent", &values)
            .expect("settings should save");
        let reloaded = load_extension_settings_values(temp.path(), "ahk_agent")
            .expect("settings should reload");

        assert_eq!(reloaded, values);
    }

    #[test]
    fn resolves_defaults_and_ignores_orphaned_keys() {
        let schema = ExtensionSettingsSchema {
            categories: vec![],
            items: vec![
                ExtensionSettingItem {
                    key: "alpha".to_string(),
                    label: "Alpha".to_string(),
                    description: None,
                    category: None,
                    kind: ExtensionSettingKind::Toggle,
                    default: true,
                },
                ExtensionSettingItem {
                    key: "beta".to_string(),
                    label: "Beta".to_string(),
                    description: None,
                    category: None,
                    kind: ExtensionSettingKind::Toggle,
                    default: false,
                },
            ],
        };
        let stored = ExtensionSettingsValues::from([
            ("beta".to_string(), true),
            ("orphan".to_string(), false),
        ]);

        let resolved = resolved_extension_settings_values(&schema, &stored);

        assert_eq!(
            resolved,
            ExtensionSettingsValues::from([
                ("alpha".to_string(), true),
                ("beta".to_string(), true),
            ])
        );
    }

    #[test]
    fn validates_duplicate_setting_keys() {
        let err = validate_extension_settings_schema(ExtensionSettingsSchema {
            categories: vec![],
            items: vec![
                ExtensionSettingItem {
                    key: "duplicate".to_string(),
                    label: "One".to_string(),
                    description: None,
                    category: None,
                    kind: ExtensionSettingKind::Toggle,
                    default: true,
                },
                ExtensionSettingItem {
                    key: "duplicate".to_string(),
                    label: "Two".to_string(),
                    description: None,
                    category: None,
                    kind: ExtensionSettingKind::Toggle,
                    default: false,
                },
            ],
        })
        .expect_err("duplicate keys should fail validation");

        assert!(err.contains("Duplicate"));
    }

    #[test]
    fn converts_static_config_settings_into_schema() {
        let schema = static_extension_settings_schema(&sample_config(
            vec![category("general", "General", Some("sample.toggle"))],
            vec![toggle_setting(
                "sample.toggle",
                "Sample toggle",
                Some("general"),
                true,
            )],
        ))
        .expect("settings schema should load")
        .expect("schema should be present");

        assert_eq!(schema.categories.len(), 1);
        assert_eq!(schema.categories[0].key, "general");
        assert_eq!(schema.items.len(), 1);
        assert_eq!(schema.items[0].key, "sample.toggle");
        assert_eq!(schema.items[0].label, "Sample toggle");
        assert!(schema.items[0].default);
    }

    #[test]
    fn validates_duplicate_category_keys() {
        let err = validate_extension_settings_schema(ExtensionSettingsSchema {
            categories: vec![
                ExtensionSettingsCategory {
                    key: "duplicate".to_string(),
                    label: "One".to_string(),
                    description: None,
                    toggle_key: None,
                    default_collapsed: false,
                },
                ExtensionSettingsCategory {
                    key: "duplicate".to_string(),
                    label: "Two".to_string(),
                    description: None,
                    toggle_key: None,
                    default_collapsed: true,
                },
            ],
            items: vec![],
        })
        .expect_err("duplicate category keys should fail validation");

        assert!(err.contains("Duplicate"));
    }

    #[test]
    fn validates_missing_item_category_reference() {
        let err = validate_extension_settings_schema(ExtensionSettingsSchema {
            categories: vec![],
            items: vec![ExtensionSettingItem {
                key: "sample.toggle".to_string(),
                label: "Sample".to_string(),
                description: None,
                category: Some("missing".to_string()),
                kind: ExtensionSettingKind::Toggle,
                default: true,
            }],
        })
        .expect_err("missing category references should fail validation");

        assert!(err.contains("unknown category"));
    }

    #[test]
    fn validates_category_toggle_key_references_item_in_same_category() {
        let err = validate_extension_settings_schema(ExtensionSettingsSchema {
            categories: vec![ExtensionSettingsCategory {
                key: "script".to_string(),
                label: "Script".to_string(),
                description: None,
                toggle_key: Some("script.enabled".to_string()),
                default_collapsed: true,
            }],
            items: vec![ExtensionSettingItem {
                key: "script.enabled".to_string(),
                label: "Enabled".to_string(),
                description: None,
                category: None,
                kind: ExtensionSettingKind::Toggle,
                default: true,
            }],
        })
        .expect_err("category toggle keys outside the category should fail");

        assert!(err.contains("same category"));
    }
}
