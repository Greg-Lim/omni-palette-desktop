use wasmtime::Linker;

use super::PluginStoreState;

mod time;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    time::register(linker)?;
    Ok(())
}
