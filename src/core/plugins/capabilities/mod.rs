use std::{collections::HashSet, path::PathBuf, sync::Arc};

use serde::Deserialize;
use wasmtime::Linker;

#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;
use crate::domain::action::InteractionContext;

mod read;
mod write;

pub(crate) type TextEffectFn = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;
pub(crate) type TypeTextFn = TextEffectFn;
pub(crate) type InsertTextFn = TextEffectFn;
pub(crate) type ReadTimeJsonFn = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;
pub(crate) type ResolvePluginStorageRootFn =
    Arc<dyn Fn(&str) -> Result<PathBuf, String> + Send + Sync>;
pub(crate) type ReadSettingsTextFn = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PluginPermission {
    TypeText,
    InsertText,
    ReadTime,
    ReadStorage,
    ReadSettings,
    ReadContext,
    #[cfg(debug_assertions)]
    WritePerformanceLog,
}

#[derive(Clone)]
pub(crate) struct PluginHostContext {
    pub(crate) type_text: TypeTextFn,
    pub(crate) insert_text: InsertTextFn,
    pub(crate) read_time_json: ReadTimeJsonFn,
    pub(crate) resolve_storage_root: ResolvePluginStorageRootFn,
    pub(crate) read_settings_text: ReadSettingsTextFn,
    #[cfg(debug_assertions)]
    pub(crate) write_performance_log: LogPerformanceSnapshotFn,
}

pub(crate) struct PluginStoreState {
    pub(crate) plugin_id: String,
    pub(crate) permissions: HashSet<PluginPermission>,
    pub(crate) host_context: PluginHostContext,
    pub(crate) active_interaction: InteractionContext,
    pub(crate) allow_host_reads: bool,
    pub(crate) allow_host_effects: bool,
}

pub(crate) fn register_capabilities(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    read::register(linker)?;
    write::register(linker)?;
    Ok(())
}
