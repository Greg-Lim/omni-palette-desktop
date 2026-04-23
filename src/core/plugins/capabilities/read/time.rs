use wasmtime::{Caller, Linker};

use crate::core::plugins::capabilities::{PluginPermission, PluginStoreState};

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_read_time_text",
            |mut caller: Caller<'_, PluginStoreState>, ptr: i32, capacity: i32| -> i32 {
                if !caller.data().allow_host_effects
                    || !caller
                        .data()
                        .permissions
                        .contains(&PluginPermission::ReadTime)
                {
                    return -1;
                }

                let Some(memory) = caller
                    .get_export("memory")
                    .and_then(|item| item.into_memory())
                else {
                    return -2;
                };

                let Ok(text) = (caller.data().host_context.read_time_text)() else {
                    return -3;
                };
                let bytes = text.as_bytes();
                let start = ptr.max(0) as usize;
                let capacity = capacity.max(0) as usize;
                let end = start.saturating_add(bytes.len());
                if bytes.len() > capacity {
                    return -4;
                }

                let data = memory.data_mut(&mut caller);
                let Some(buffer) = data.get_mut(start..end) else {
                    return -5;
                };
                buffer.copy_from_slice(bytes);
                bytes.len() as i32
            },
        )
        .map_err(|err| format!("Could not define host_read_time_text: {err}"))?;

    Ok(())
}
