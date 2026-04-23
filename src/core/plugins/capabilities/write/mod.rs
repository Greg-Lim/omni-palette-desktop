use wasmtime::Linker;

use super::PluginStoreState;

#[cfg(debug_assertions)]
mod performance_log;
mod text;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    text::register(linker)?;
    #[cfg(debug_assertions)]
    performance_log::register(linker)?;
    Ok(())
}
