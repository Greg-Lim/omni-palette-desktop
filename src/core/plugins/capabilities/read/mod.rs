use wasmtime::Linker;

use super::PluginStoreState;

mod storage_entries;
mod storage_text;
mod time;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    storage_entries::register(linker)?;
    storage_text::register(linker)?;
    time::register(linker)?;
    Ok(())
}
