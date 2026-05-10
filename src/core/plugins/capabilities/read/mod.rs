use wasmtime::Linker;

use super::PluginStoreState;

mod context;
mod settings;
mod storage_entries;
mod storage_text;
mod time;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    context::register(linker)?;
    storage_entries::register(linker)?;
    settings::register(linker)?;
    storage_text::register(linker)?;
    time::register(linker)?;
    Ok(())
}
