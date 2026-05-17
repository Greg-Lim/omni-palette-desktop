use std::{path::Path, sync::Arc};

use crate::{core::extensions::settings::ExtensionSettingsSchema, domain::action::Os};

pub(crate) mod capabilities;
pub mod command;
pub(crate) mod manifest;
pub mod registry;
pub(crate) mod runtime;

pub use command::PluginApplication;
pub use registry::PluginRegistry;

#[allow(dead_code)]
pub fn load_plugin_settings_schema_from_manifest(
    manifest_path: &Path,
    current_os: Os,
) -> Result<Option<ExtensionSettingsSchema>, String> {
    runtime::LoadedPlugin::load_settings_schema_from_manifest(
        manifest_path,
        current_os,
        Arc::new(|_text| Ok(())),
        Arc::new(|_text| Ok(())),
        Arc::new(|| Ok(r#"{"unix":0}"#.to_string())),
        Arc::new(|plugin_id| Ok(std::env::temp_dir().join("OmniPalette").join(plugin_id))),
        Arc::new(|_plugin_id| Ok("{}".to_string())),
        #[cfg(debug_assertions)]
        Arc::new(|| Ok(())),
    )
}
