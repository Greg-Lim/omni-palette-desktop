use std::{collections::BTreeMap, sync::Mutex};

use serde::{Deserialize, Serialize};

const INITIAL_HOST_BUFFER_CAPACITY: usize = 16 * 1024;
const MAX_HOST_BUFFER_CAPACITY: usize = 1024 * 1024;
const HOST_BUFFER_TOO_SMALL_CODE: i32 = -4;
const SETTINGS_KEY: &str = "auto_typer.entries";
const TEXT_CATEGORY_KEY: &str = "text";

static RESPONSE_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(target_arch = "wasm32")]
mod host {
    unsafe extern "C" {
        fn host_read_settings_json(ptr: i32, capacity: i32) -> i32;
        fn host_write_text(ptr: i32, len: i32) -> i32;
    }

    pub(crate) fn read_settings_json(buffer: &mut [u8]) -> i32 {
        unsafe { host_read_settings_json(buffer.as_mut_ptr() as i32, buffer.len() as i32) }
    }

    pub(crate) fn write_text(text: &str) -> i32 {
        unsafe { host_write_text(text.as_ptr() as i32, text.len() as i32) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod host {
    pub(crate) fn read_settings_json(_buffer: &mut [u8]) -> i32 {
        -100
    }

    pub(crate) fn write_text(_text: &str) -> i32 {
        0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SettingsSchemaDescriptor {
    #[serde(default)]
    categories: Vec<SettingsSchemaCategoryDescriptor>,
    #[serde(default)]
    items: Vec<SettingsSchemaItemDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SettingsSchemaCategoryDescriptor {
    key: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toggle_key: Option<String>,
    #[serde(default)]
    default_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SettingsSchemaItemDescriptor {
    key: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(rename = "type")]
    kind: SettingsSchemaItemType,
    #[serde(default)]
    default: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    default_entries: Vec<TextEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry_list_format_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry_list_default_format: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SettingsSchemaItemType {
    EntryList,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SettingsValues {
    #[serde(default)]
    toggles: BTreeMap<String, bool>,
    #[serde(default)]
    lists: BTreeMap<String, Vec<TextEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TextEntry {
    id: String,
    name: String,
    format: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CommandDescriptor {
    id: String,
    name: String,
    priority: String,
    focus_state: String,
    tags: Vec<String>,
    shortcut_text: String,
}

#[no_mangle]
pub extern "C" fn register_commands_json() -> i32 {
    let json = serde_json::to_string(&command_descriptors(&configured_entries_from_host()))
        .unwrap_or_else(|_| "[]".to_string());
    store_response_string(&json)
}

#[no_mangle]
pub extern "C" fn settings_schema_json() -> i32 {
    let json = serde_json::to_string(&settings_schema())
        .unwrap_or_else(|_| "{\"categories\":[],\"items\":[]}".to_string());
    store_response_string(&json)
}

#[no_mangle]
pub extern "C" fn execute(command_id_ptr: i32, command_id_len: i32) -> i32 {
    let command_id = match read_guest_string(command_id_ptr, command_id_len) {
        Ok(command_id) => command_id,
        Err(code) => return code,
    };
    let entries = configured_entries_from_host();
    let Some(entry) = entries
        .iter()
        .find(|entry| command_id_for_entry(entry) == command_id)
    else {
        return 3;
    };

    if host::write_text(&entry.format) == 0 {
        0
    } else {
        5
    }
}

fn settings_schema() -> SettingsSchemaDescriptor {
    SettingsSchemaDescriptor {
        categories: vec![SettingsSchemaCategoryDescriptor {
            key: TEXT_CATEGORY_KEY.to_string(),
            label: "Text".to_string(),
            description: None,
            toggle_key: None,
            default_collapsed: false,
        }],
        items: vec![SettingsSchemaItemDescriptor {
            key: SETTINGS_KEY.to_string(),
            label: "Text entries".to_string(),
            description: Some(
                "Each enabled entry becomes a command that types its text.".to_string(),
            ),
            category: Some(TEXT_CATEGORY_KEY.to_string()),
            kind: SettingsSchemaItemType::EntryList,
            default: false,
            default_entries: default_entries(),
            entry_list_format_hint: Some("Text".to_string()),
            entry_list_default_format: Some("Text to type".to_string()),
        }],
    }
}

fn default_entries() -> Vec<TextEntry> {
    vec![TextEntry {
        id: "type_hello_world".to_string(),
        name: "Type hello world".to_string(),
        format: "hello world".to_string(),
        enabled: true,
    }]
}

fn configured_entries_from_host() -> Vec<TextEntry> {
    let Ok(json) = read_host_text_with_retry(
        "host_read_settings_json",
        INITIAL_HOST_BUFFER_CAPACITY,
        MAX_HOST_BUFFER_CAPACITY,
        host::read_settings_json,
    ) else {
        return default_entries();
    };
    configured_entries_from_settings_json(&json)
}

fn configured_entries_from_settings_json(json: &str) -> Vec<TextEntry> {
    if json.trim().is_empty() || json.trim() == "{}" {
        return default_entries();
    }
    let Ok(values) = serde_json::from_str::<SettingsValues>(json) else {
        return default_entries();
    };
    values
        .lists
        .get(SETTINGS_KEY)
        .cloned()
        .unwrap_or_else(default_entries)
        .into_iter()
        .filter(|entry| {
            entry.enabled && !entry.name.trim().is_empty() && !entry.format.trim().is_empty()
        })
        .collect()
}

fn command_descriptors(entries: &[TextEntry]) -> Vec<CommandDescriptor> {
    entries
        .iter()
        .map(|entry| CommandDescriptor {
            id: command_id_for_entry(entry),
            name: entry.name.trim().to_string(),
            priority: "medium".to_string(),
            focus_state: "global".to_string(),
            tags: vec!["text".to_string(), "typing".to_string()],
            shortcut_text: entry.format.clone(),
        })
        .collect()
}

fn command_id_for_entry(entry: &TextEntry) -> String {
    if entry.id == "type_hello_world" {
        return entry.id.clone();
    }
    format!("auto_typer_{}", normalize_id(&entry.id))
}

fn normalize_id(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "entry".to_string()
    } else {
        normalized.to_string()
    }
}

fn default_true() -> bool {
    true
}

fn store_response_string(value: &str) -> i32 {
    let mut buffer = RESPONSE_BUFFER
        .lock()
        .expect("response buffer lock should not poison");
    buffer.clear();
    buffer.extend_from_slice(value.as_bytes());
    buffer.as_ptr() as i32
}

fn read_guest_string(ptr: i32, len: i32) -> Result<String, i32> {
    if ptr < 0 || len < 0 {
        return Err(1);
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    String::from_utf8(bytes.to_vec()).map_err(|_| 1)
}

fn read_host_text_with_retry(
    capability_name: &str,
    initial_capacity: usize,
    max_capacity: usize,
    read: impl Fn(&mut [u8]) -> i32,
) -> Result<String, String> {
    let mut capacity = initial_capacity;
    loop {
        let mut buffer = vec![0; capacity];
        let len = read(&mut buffer);
        if len == HOST_BUFFER_TOO_SMALL_CODE && capacity < max_capacity {
            capacity = capacity.saturating_mul(2).min(max_capacity);
            continue;
        }
        if len < 0 {
            return Err(format!("{capability_name} failed with code {len}"));
        }
        let len = len as usize;
        if len > buffer.len() {
            return Err(format!(
                "{capability_name} returned {len} bytes for a {} byte buffer",
                buffer.len()
            ));
        }
        buffer.truncate(len);
        return String::from_utf8(buffer)
            .map_err(|err| format!("{capability_name} returned invalid UTF-8: {err}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_schema_exposes_text_entry_defaults() {
        let schema = settings_schema();

        assert_eq!(schema.categories.len(), 1);
        assert_eq!(schema.categories[0].label, "Text");
        assert_eq!(schema.items.len(), 1);
        assert_eq!(schema.items[0].key, SETTINGS_KEY);
        assert_eq!(schema.items[0].category.as_deref(), Some(TEXT_CATEGORY_KEY));
        assert_eq!(schema.items[0].kind, SettingsSchemaItemType::EntryList);
        assert_eq!(
            schema.items[0].entry_list_format_hint.as_deref(),
            Some("Text")
        );
        assert_eq!(
            schema.items[0].entry_list_default_format.as_deref(),
            Some("Text to type")
        );
        assert_eq!(schema.items[0].default_entries, default_entries());
    }

    #[test]
    fn falls_back_to_default_entries_when_settings_are_empty() {
        let entries = configured_entries_from_settings_json("{}");

        assert_eq!(entries, default_entries());
    }

    #[test]
    fn builds_commands_from_enabled_non_empty_settings_entries() {
        let json = serde_json::json!({
            "lists": {
                SETTINGS_KEY: [
                    {"id":"signoff","name":"Signoff","format":"Thanks,\nGreg","enabled":true},
                    {"id":"disabled","name":"Disabled","format":"Hidden","enabled":false},
                    {"id":"empty-name","name":" ","format":"Visible","enabled":true},
                    {"id":"empty-text","name":"Empty text","format":" ","enabled":true}
                ]
            }
        })
        .to_string();
        let entries = configured_entries_from_settings_json(&json);
        let commands = command_descriptors(&entries);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "auto_typer_signoff");
        assert_eq!(commands[0].name, "Signoff");
        assert_eq!(commands[0].shortcut_text, "Thanks,\nGreg");
    }
}
