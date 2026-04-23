use wasmtime::{Caller, Linker};

use super::{PluginPermission, PluginStoreState};

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_log_performance_snapshot",
            |caller: Caller<'_, PluginStoreState>| -> i32 {
                if !caller.data().allow_host_effects
                    || !caller
                        .data()
                        .permissions
                        .contains(&PluginPermission::PerformanceMetrics)
                {
                    return 1;
                }

                match (caller.data().host_context.log_performance_snapshot)() {
                    Ok(()) => 0,
                    Err(_) => 2,
                }
            },
        )
        .map_err(|err| format!("Could not define host_log_performance_snapshot: {err}"))?;

    Ok(())
}

