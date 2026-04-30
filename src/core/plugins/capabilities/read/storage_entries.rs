use std::{fs, path::Path};

use wasmtime::{Caller, Linker};

use crate::core::plugins::capabilities::{PluginPermission, PluginStoreState};

const BUFFER_TOO_SMALL_CODE: i32 = -4;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_list_storage_entries_json",
            |mut caller: Caller<'_, PluginStoreState>, ptr: i32, capacity: i32| -> i32 {
                if !caller.data().allow_host_reads
                    || !caller
                        .data()
                        .permissions
                        .contains(&PluginPermission::ReadStorage)
                {
                    return -1;
                }

                let Some(memory) = caller
                    .get_export("memory")
                    .and_then(|item| item.into_memory())
                else {
                    return -2;
                };

                let root = match (caller.data().host_context.resolve_storage_root)(
                    &caller.data().plugin_id,
                ) {
                    Ok(root) => root,
                    Err(_) => return -3,
                };
                let Ok(text) = list_storage_entries_json(&root) else {
                    return -3;
                };

                let bytes = text.as_bytes();
                let start = ptr.max(0) as usize;
                let capacity = capacity.max(0) as usize;
                let end = start.saturating_add(bytes.len());
                if bytes.len() > capacity {
                    return BUFFER_TOO_SMALL_CODE;
                }

                let data = memory.data_mut(&mut caller);
                let Some(buffer) = data.get_mut(start..end) else {
                    return -5;
                };
                buffer.copy_from_slice(bytes);
                bytes.len() as i32
            },
        )
        .map_err(|err| format!("Could not define host_list_storage_entries_json: {err}"))?;

    Ok(())
}

pub(crate) fn list_storage_entries_json(root: &Path) -> Result<String, String> {
    if !root.exists() {
        return Ok("[]".to_string());
    }

    let mut entries = Vec::new();
    collect_storage_entries(root, Path::new(""), &mut entries)?;
    entries.sort();
    serde_json::to_string(&entries)
        .map_err(|err| format!("Could not serialize storage entries: {err}"))
}

fn collect_storage_entries(
    root: &Path,
    relative: &Path,
    entries: &mut Vec<String>,
) -> Result<(), String> {
    let folder = if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    let dir_entries = fs::read_dir(&folder).map_err(|err| {
        format!(
            "Could not read storage directory {}: {err}",
            folder.display()
        )
    })?;

    for entry in dir_entries {
        let entry = entry.map_err(|err| format!("Could not read storage entry: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Could not read storage entry type: {err}"))?;
        let file_name = entry.file_name();
        let next_relative = relative.join(&file_name);

        if file_type.is_dir() {
            collect_storage_entries(root, &next_relative, entries)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(relative_text) = path_to_storage_string(&next_relative) else {
            continue;
        };
        entries.push(relative_text);
    }

    Ok(())
}

fn path_to_storage_string(path: &Path) -> Option<String> {
    let mut pieces = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(segment) = component else {
            return None;
        };
        pieces.push(segment.to_str()?.to_string());
    }

    Some(pieces.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_storage_root_returns_empty_json_array() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let missing_root = temp.path().join("missing");

        let json = list_storage_entries_json(&missing_root).expect("listing should succeed");

        assert_eq!(json, "[]");
    }

    #[test]
    fn lists_files_recursively_as_relative_paths() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        fs::create_dir_all(temp.path().join("scripts").join("nested"))
            .expect("nested dir should be created");
        fs::write(temp.path().join("scripts").join("alpha.json"), "{}")
            .expect("alpha file should be written");
        fs::write(
            temp.path().join("scripts").join("nested").join("beta.json"),
            "{}",
        )
        .expect("beta file should be written");

        let json = list_storage_entries_json(temp.path()).expect("listing should succeed");

        assert_eq!(json, r#"["scripts/alpha.json","scripts/nested/beta.json"]"#);
    }
}
