use wasmtime::{Caller, Linker};

use crate::core::plugins::capabilities::{PluginPermission, PluginStoreState};

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_write_performance_log",
            |caller: Caller<'_, PluginStoreState>| -> i32 {
                if !caller.data().allow_host_effects
                    || !caller
                        .data()
                        .permissions
                        .contains(&PluginPermission::WritePerformanceLog)
                {
                    return 1;
                }

                match (caller.data().host_context.write_performance_log)() {
                    Ok(()) => 0,
                    Err(_) => 2,
                }
            },
        )
        .map_err(|err| format!("Could not define host_write_performance_log: {err}"))?;

    Ok(())
}
