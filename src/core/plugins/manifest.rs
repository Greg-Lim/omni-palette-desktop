use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::core::plugins::capabilities::PluginPermission;
use crate::domain::action::{FocusState, Os};

#[derive(Debug, Deserialize)]
pub(crate) struct PluginManifest {
    pub id: String,
    pub name: String,
    pub platform: Os,
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
        let manifest: Self = toml::from_str(&manifest_content)
            .map_err(|err| format!("Could not parse plugin manifest: {err}"))?;
        semver::Version::parse(&manifest.version)
            .map_err(|err| format!("Invalid plugin version {}: {err}", manifest.version))?;
        Ok(manifest)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PluginAppConfig {
    pub default_focus_state: Option<FocusState>,
    pub default_tags: Option<Vec<String>>,
}
