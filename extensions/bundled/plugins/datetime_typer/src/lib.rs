use std::{collections::BTreeMap, sync::Mutex};

use serde::{Deserialize, Serialize};

const INITIAL_HOST_BUFFER_CAPACITY: usize = 16 * 1024;
const MAX_HOST_BUFFER_CAPACITY: usize = 1024 * 1024;
const HOST_BUFFER_TOO_SMALL_CODE: i32 = -4;
const SETTINGS_KEY: &str = "datetime_typer.entries";
const SHORTCUTS_CATEGORY_KEY: &str = "shortcuts";

static RESPONSE_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(target_arch = "wasm32")]
mod host {
    unsafe extern "C" {
        fn host_read_settings_json(ptr: i32, capacity: i32) -> i32;
        fn host_read_time_json(ptr: i32, capacity: i32) -> i32;
        fn host_write_text(ptr: i32, len: i32) -> i32;
    }

    pub(crate) fn read_settings_json(buffer: &mut [u8]) -> i32 {
        unsafe { host_read_settings_json(buffer.as_mut_ptr() as i32, buffer.len() as i32) }
    }

    pub(crate) fn read_time_json(buffer: &mut [u8]) -> i32 {
        unsafe { host_read_time_json(buffer.as_mut_ptr() as i32, buffer.len() as i32) }
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

    pub(crate) fn read_time_json(_buffer: &mut [u8]) -> i32 {
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
    default_entries: Vec<DateTimeEntry>,
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
    lists: BTreeMap<String, Vec<DateTimeEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DateTimeEntry {
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct TimeSnapshot {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    weekday: u8,
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
    let snapshot = match time_snapshot_from_host() {
        Ok(snapshot) => snapshot,
        Err(_) => return 4,
    };
    let text = format_datetime(&entry.format, &snapshot);
    if host::write_text(&text) == 0 {
        0
    } else {
        5
    }
}

fn settings_schema() -> SettingsSchemaDescriptor {
    SettingsSchemaDescriptor {
        categories: vec![SettingsSchemaCategoryDescriptor {
            key: SHORTCUTS_CATEGORY_KEY.to_string(),
            label: "Shortcuts".to_string(),
            description: None,
            toggle_key: None,
            default_collapsed: false,
        }],
        items: vec![SettingsSchemaItemDescriptor {
            key: SETTINGS_KEY.to_string(),
            label: "Date and time entries".to_string(),
            description: Some(
                "Each enabled entry becomes a command that types the formatted current local time."
                    .to_string(),
            ),
            category: Some(SHORTCUTS_CATEGORY_KEY.to_string()),
            kind: SettingsSchemaItemType::EntryList,
            default: false,
            default_entries: default_entries(),
        }],
    }
}

fn default_entries() -> Vec<DateTimeEntry> {
    vec![
        DateTimeEntry {
            id: "print_date_short".to_string(),
            name: "Print date short".to_string(),
            format: "{D} {MMM}".to_string(),
            enabled: true,
        },
        DateTimeEntry {
            id: "print_date_long".to_string(),
            name: "Print date long".to_string(),
            format: "{D} {MMMM} {YYYY}".to_string(),
            enabled: true,
        },
        DateTimeEntry {
            id: "print_date_time".to_string(),
            name: "Print date time".to_string(),
            format: "{D} {MMM} {YYYY} {HH}:{mm}".to_string(),
            enabled: true,
        },
    ]
}

fn configured_entries_from_host() -> Vec<DateTimeEntry> {
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

fn configured_entries_from_settings_json(json: &str) -> Vec<DateTimeEntry> {
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

fn command_descriptors(entries: &[DateTimeEntry]) -> Vec<CommandDescriptor> {
    entries
        .iter()
        .map(|entry| CommandDescriptor {
            id: command_id_for_entry(entry),
            name: entry.name.trim().to_string(),
            priority: "medium".to_string(),
            focus_state: "global".to_string(),
            tags: vec!["date".to_string(), "time".to_string(), "typing".to_string()],
            shortcut_text: entry.format.clone(),
        })
        .collect()
}

fn command_id_for_entry(entry: &DateTimeEntry) -> String {
    format!("datetime_typer_{}", normalize_id(&entry.id))
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

fn time_snapshot_from_host() -> Result<TimeSnapshot, String> {
    let json = read_host_text_with_retry(
        "host_read_time_json",
        INITIAL_HOST_BUFFER_CAPACITY,
        MAX_HOST_BUFFER_CAPACITY,
        host::read_time_json,
    )?;
    serde_json::from_str(&json).map_err(|err| format!("Could not parse time JSON: {err}"))
}

fn format_datetime(format: &str, snapshot: &TimeSnapshot) -> String {
    let mut output = String::new();
    let mut rest = format;

    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start..];
        let Some(end) = after_start.find('}') else {
            output.push_str(after_start);
            return output;
        };
        let token = &after_start[..=end];
        if let Some(value) = format_token(token, snapshot) {
            output.push_str(&value);
        } else {
            output.push_str(token);
        }
        rest = &after_start[end + 1..];
    }

    output.push_str(rest);
    output
}

fn format_token(token: &str, snapshot: &TimeSnapshot) -> Option<String> {
    match token {
        "{D}" => Some(snapshot.day.to_string()),
        "{DD}" => Some(format!("{:02}", snapshot.day)),
        "{M}" => Some(snapshot.month.to_string()),
        "{MM}" => Some(format!("{:02}", snapshot.month)),
        "{MMM}" => month_name(snapshot.month, MonthNameLength::Short),
        "{MMMM}" => month_name(snapshot.month, MonthNameLength::Long),
        "{ddd}" => weekday_name(snapshot.weekday, WeekdayNameLength::Short),
        "{dddd}" => weekday_name(snapshot.weekday, WeekdayNameLength::Long),
        "{YY}" => Some(format!("{:02}", snapshot.year % 100)),
        "{YYYY}" => Some(snapshot.year.to_string()),
        "{H}" => Some(snapshot.hour.to_string()),
        "{HH}" => Some(format!("{:02}", snapshot.hour)),
        "{m}" => Some(snapshot.minute.to_string()),
        "{mm}" => Some(format!("{:02}", snapshot.minute)),
        "{s}" => Some(snapshot.second.to_string()),
        "{ss}" => Some(format!("{:02}", snapshot.second)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonthNameLength {
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeekdayNameLength {
    Short,
    Long,
}

fn month_name(month: u8, length: MonthNameLength) -> Option<String> {
    const MONTHS_SHORT: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const MONTHS_LONG: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    let index = usize::from(month.checked_sub(1)?);
    let name = match length {
        MonthNameLength::Short => MONTHS_SHORT.get(index)?,
        MonthNameLength::Long => MONTHS_LONG.get(index)?,
    };
    Some((*name).to_string())
}

fn weekday_name(weekday: u8, length: WeekdayNameLength) -> Option<String> {
    const WEEKDAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const WEEKDAYS_LONG: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];

    let index = usize::from(weekday);
    let name = match length {
        WeekdayNameLength::Short => WEEKDAYS_SHORT.get(index)?,
        WeekdayNameLength::Long => WEEKDAYS_LONG.get(index)?,
    };
    Some((*name).to_string())
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
    buffer.push(0);
    buffer.as_ptr() as i32
}

fn read_guest_string(ptr: i32, len: i32) -> Result<String, i32> {
    if ptr < 0 || len < 0 {
        return Err(1);
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|_| 2)
}

fn read_host_text_with_retry(
    capability_name: &str,
    initial_capacity: usize,
    max_capacity: usize,
    mut reader: impl FnMut(&mut [u8]) -> i32,
) -> Result<String, String> {
    let mut capacity = initial_capacity.max(1);

    loop {
        let mut buffer = vec![0_u8; capacity];
        let len = reader(&mut buffer);
        if len == HOST_BUFFER_TOO_SMALL_CODE {
            if capacity >= max_capacity {
                return Err(format!(
                    "{capability_name} exceeded max buffer capacity of {max_capacity} bytes"
                ));
            }
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

    fn sample_time() -> TimeSnapshot {
        TimeSnapshot {
            year: 2026,
            month: 5,
            day: 8,
            hour: 7,
            minute: 6,
            second: 5,
            weekday: 5,
        }
    }

    #[test]
    fn formats_supported_tokens_and_leaves_unknown_tokens_literal() {
        let formatted = format_datetime(
            "{D}/{DD} {M}/{MM} {MMM}/{MMMM} {ddd}/{dddd} {YY}/{YYYY} {H}/{HH}:{m}/{mm}:{s}/{ss} {NOPE}",
            &sample_time(),
        );

        assert_eq!(
            formatted,
            "8/08 5/05 May/May Fri/Friday 26/2026 7/07:6/06:5/05 {NOPE}"
        );
    }

    #[test]
    fn settings_schema_exposes_entry_list_defaults() {
        let schema = settings_schema();

        assert_eq!(schema.categories.len(), 1);
        assert_eq!(schema.categories[0].key, SHORTCUTS_CATEGORY_KEY);
        assert_eq!(schema.categories[0].label, "Shortcuts");
        assert_eq!(schema.items.len(), 1);
        assert_eq!(schema.items[0].key, SETTINGS_KEY);
        assert_eq!(
            schema.items[0].category.as_deref(),
            Some(SHORTCUTS_CATEGORY_KEY)
        );
        assert_eq!(schema.items[0].kind, SettingsSchemaItemType::EntryList);
        assert_eq!(schema.items[0].default_entries.len(), 3);
    }

    #[test]
    fn builds_commands_from_enabled_non_empty_settings_entries() {
        let json = serde_json::json!({
            "lists": {
                SETTINGS_KEY: [
                    {"id":"short","name":"Short","format":"{D} {MMM}","enabled":true},
                    {"id":"disabled","name":"Disabled","format":"{YYYY}","enabled":false},
                    {"id":"empty-name","name":" ","format":"{YYYY}","enabled":true}
                ]
            }
        })
        .to_string();
        let entries = configured_entries_from_settings_json(&json);
        let commands = command_descriptors(&entries);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "datetime_typer_short");
        assert_eq!(commands[0].name, "Short");
    }

    #[test]
    fn falls_back_to_default_entries_when_settings_are_empty() {
        let entries = configured_entries_from_settings_json("{}");

        assert_eq!(entries, default_entries());
    }
}
