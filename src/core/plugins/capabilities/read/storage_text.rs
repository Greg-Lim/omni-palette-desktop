use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use wasmtime::{Caller, Linker};

use crate::core::plugins::capabilities::{PluginPermission, PluginStoreState};

const BUFFER_TOO_SMALL_CODE: i32 = -4;

pub(crate) fn register(linker: &mut Linker<PluginStoreState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "host_read_storage_text",
            |mut caller: Caller<'_, PluginStoreState>,
             path_ptr: i32,
             path_len: i32,
             ptr: i32,
             capacity: i32|
             -> i32 {
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

                let data = memory.data(&caller);
                let path_start = path_ptr.max(0) as usize;
                let path_end = path_start.saturating_add(path_len.max(0) as usize);
                let Some(path_bytes) = data.get(path_start..path_end) else {
                    return -3;
                };
                let Ok(path_text) = std::str::from_utf8(path_bytes) else {
                    return -3;
                };

                let root = match (caller.data().host_context.resolve_storage_root)(
                    &caller.data().plugin_id,
                ) {
                    Ok(root) => root,
                    Err(_) => return -3,
                };
                let Ok(text) = read_storage_text(&root, path_text) else {
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
        .map_err(|err| format!("Could not define host_read_storage_text: {err}"))?;

    Ok(())
}

pub(crate) fn read_storage_text(root: &Path, relative_path: &str) -> Result<String, String> {
    let relative = normalize_relative_storage_path(relative_path)?;
    let path = root.join(relative);
    fs::read_to_string(&path)
        .map_err(|err| format!("Could not read storage file {}: {err}", path.display()))
}

fn normalize_relative_storage_path(relative_path: &str) -> Result<PathBuf, String> {
    if relative_path.trim().is_empty() {
        return Err("Storage path must not be empty".to_string());
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(format!(
                    "Storage path must stay within the plugin root: {relative_path}"
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err("Storage path must contain at least one segment".to_string());
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_files_from_relative_storage_paths() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        fs::create_dir_all(temp.path().join("scripts")).expect("scripts dir should be created");
        fs::write(
            temp.path().join("scripts").join("alpha.json"),
            "{\"ok\":true}",
        )
        .expect("storage file should be written");

        let text = read_storage_text(temp.path(), "scripts/alpha.json")
            .expect("storage read should succeed");

        assert_eq!(text, "{\"ok\":true}");
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let err = read_storage_text(temp.path(), "../secret.txt")
            .expect_err("parent traversal should be rejected");

        assert!(err.contains("within the plugin root"));
    }

    #[test]
    fn rejects_absolute_storage_paths() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let err = read_storage_text(temp.path(), "C:/secret.txt")
            .expect_err("absolute path should be rejected");

        assert!(err.contains("within the plugin root"));
    }
}
