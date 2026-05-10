use wasmtime::{Caller, Linker};

use crate::core::plugins::capabilities::{PluginPermission, PluginStoreState};

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    register_text_import(linker, "host_type_text", PluginPermission::TypeText)?;
    register_text_import(linker, "host_insert_text", PluginPermission::InsertText)?;

    Ok(())
}

fn register_text_import(
    linker: &mut Linker<PluginStoreState>,
    import_name: &'static str,
    permission: PluginPermission,
) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            import_name,
            move |mut caller: Caller<'_, PluginStoreState>, ptr: i32, len: i32| -> i32 {
                if !caller.data().allow_host_effects
                    || !caller.data().permissions.contains(&permission)
                {
                    return 1;
                }

                let Some(memory) = caller
                    .get_export("memory")
                    .and_then(|item| item.into_memory())
                else {
                    return 2;
                };
                if ptr < 0 || len < 0 {
                    return 3;
                }
                let data = memory.data(&caller);
                let start = ptr as usize;
                let end = start.saturating_add(len as usize);
                let Some(bytes) = data.get(start..end) else {
                    return 3;
                };
                let Ok(text) = std::str::from_utf8(bytes) else {
                    return 4;
                };

                let result = match permission {
                    PluginPermission::TypeText => (caller.data().host_context.type_text)(text),
                    PluginPermission::InsertText => (caller.data().host_context.insert_text)(text),
                    _ => return 1,
                };

                match result {
                    Ok(()) => 0,
                    Err(err) => {
                        log::warn!("{import_name} failed: {err}");
                        5
                    }
                }
            },
        )
        .map_err(|err| format!("Could not define {import_name}: {err}"))?;

    Ok(())
}
