use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::domain::action::FocusState;

#[derive(Debug, Deserialize)]
pub(crate) struct PluginManifest {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    pub wasm: PathBuf,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    pub app: Option<PluginAppConfig>,
}

impl PluginManifest {
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        let manifest_content = fs::read_to_string(path)
            .map_err(|err| format!("Could not read plugin manifest: {err}"))?;
        toml::from_str(&manifest_content)
            .map_err(|err| format!("Could not parse plugin manifest: {err}"))
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PluginPermission {
    TypeText,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PluginAppConfig {
    pub default_focus_state: Option<FocusState>,
    pub default_tags: Option<Vec<String>>,
}
