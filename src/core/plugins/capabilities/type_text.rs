use wasmtime::{Caller, Linker};

use super::{PluginPermission, PluginStoreState};

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_type_text",
            |mut caller: Caller<'_, PluginStoreState>, ptr: i32, len: i32| -> i32 {
                if !caller.data().allow_host_effects
                    || !caller
                        .data()
                        .permissions
                        .contains(&PluginPermission::TypeText)
                {
                    return 1;
                }

                let Some(memory) = caller
                    .get_export("memory")
                    .and_then(|item| item.into_memory())
                else {
                    return 2;
                };
                let data = memory.data(&caller);
                let start = ptr.max(0) as usize;
                let end = start.saturating_add(len.max(0) as usize);
                let Some(bytes) = data.get(start..end) else {
                    return 3;
                };
                let Ok(text) = std::str::from_utf8(bytes) else {
                    return 4;
                };

                (caller.data().host_context.type_text)(text);
                0
            },
        )
        .map_err(|err| format!("Could not define host_type_text: {err}"))?;

    Ok(())
}

