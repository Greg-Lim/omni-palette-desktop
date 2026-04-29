use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use wasmtime::Linker;

#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;

mod read;
mod write;

pub(crate) type WriteTextFn = Arc<dyn Fn(&str) + Send + Sync>;
pub(crate) type ReadTimeTextFn = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;
pub(crate) type ReadAhkSnapshotsJsonFn = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PluginPermission {
    WriteText,
    ReadTime,
    ReadAhkSnapshots,
    #[cfg(debug_assertions)]
    WritePerformanceLog,
}

#[derive(Clone)]
pub(crate) struct PluginHostContext {
    pub(crate) write_text: WriteTextFn,
    pub(crate) read_time_text: ReadTimeTextFn,
    pub(crate) read_ahk_snapshots_json: ReadAhkSnapshotsJsonFn,
    #[cfg(debug_assertions)]
    pub(crate) write_performance_log: LogPerformanceSnapshotFn,
}

pub(crate) struct PluginStoreState {
    pub(crate) permissions: HashSet<PluginPermission>,
    pub(crate) host_context: PluginHostContext,
    pub(crate) allow_host_reads: bool,
    pub(crate) allow_host_effects: bool,
}

pub(crate) fn register_capabilities(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    read::register(linker)?;
    write::register(linker)?;
    Ok(())
}
