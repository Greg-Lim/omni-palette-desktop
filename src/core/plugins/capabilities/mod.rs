use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use wasmtime::Linker;

#[cfg(debug_assertions)]
use crate::core::performance::LogPerformanceSnapshotFn;

#[cfg(debug_assertions)]
mod performance_metrics;
mod type_text;

pub(crate) type TypeTextFn = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PluginPermission {
    TypeText,
    #[cfg(debug_assertions)]
    PerformanceMetrics,
}

#[derive(Clone)]
pub(crate) struct PluginHostContext {
    pub(crate) type_text: TypeTextFn,
    #[cfg(debug_assertions)]
    pub(crate) log_performance_snapshot: LogPerformanceSnapshotFn,
}

pub(crate) struct PluginStoreState {
    pub(crate) permissions: HashSet<PluginPermission>,
    pub(crate) host_context: PluginHostContext,
    pub(crate) allow_host_effects: bool,
}

pub(crate) fn register_capabilities(
    linker: &mut Linker<PluginStoreState>,
) -> Result<(), String> {
    type_text::register(linker)?;
    #[cfg(debug_assertions)]
    performance_metrics::register(linker)?;
    Ok(())
}
