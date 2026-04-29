use wasmtime::Linker;

use super::PluginStoreState;

mod ahk_snapshots;
mod time;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    ahk_snapshots::register(linker)?;
    time::register(linker)?;
    Ok(())
}
