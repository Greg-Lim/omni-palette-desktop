use std::{collections::BTreeMap, sync::Mutex};

use serde::{Deserialize, Serialize};

const INITIAL_HOST_BUFFER_CAPACITY: usize = 16 * 1024;
const MAX_HOST_BUFFER_CAPACITY: usize = 4 * 1024 * 1024;
const HOST_BUFFER_TOO_SMALL_CODE: i32 = -4;
const PREVIEW_CHAR_LIMIT: usize = 24;
const SNAPSHOT_DIRECTORY_PREFIX: &str = "scripts/";
const SNAPSHOT_EXTENSION: &str = ".json";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

static RESPONSE_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(target_arch = "wasm32")]
mod host {
    unsafe extern "C" {
        fn host_list_storage_entries_json(ptr: i32, capacity: i32) -> i32;
        fn host_read_storage_text(path_ptr: i32, path_len: i32, ptr: i32, capacity: i32) -> i32;
        fn host_read_settings_json(ptr: i32, capacity: i32) -> i32;
        fn host_write_text(ptr: i32, len: i32) -> i32;
    }

    pub(crate) fn list_storage_entries_json(buffer: &mut [u8]) -> i32 {
        unsafe { host_list_storage_entries_json(buffer.as_mut_ptr() as i32, buffer.len() as i32) }
    }

    pub(crate) fn read_storage_text(path: &str, buffer: &mut [u8]) -> i32 {
        unsafe {
            host_read_storage_text(
                path.as_ptr() as i32,
                path.len() as i32,
                buffer.as_mut_ptr() as i32,
                buffer.len() as i32,
            )
        }
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
    pub(crate) fn list_storage_entries_json(_buffer: &mut [u8]) -> i32 {
        -100
    }

    pub(crate) fn read_storage_text(_path: &str, _buffer: &mut [u8]) -> i32 {
        -100
    }

    pub(crate) fn read_settings_json(_buffer: &mut [u8]) -> i32 {
        -100
    }

    pub(crate) fn write_text(_text: &str) -> i32 {
        100
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct SnapshotRecord {
    script_path: String,
    script_text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct JsonShortcutBinding {
    mods: Vec<String>,
    key: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CommandDescriptor {
    id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    shortcut_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cmd: Option<JsonShortcutBinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SettingsSchemaItemType {
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredCommand {
    descriptor: CommandDescriptor,
    trigger_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredScript {
    script_path: String,
    script_name: String,
    category_key: String,
    toggle_key: String,
    commands: Vec<DiscoveredCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredCommand {
    registered: RegisteredCommand,
    toggle_key: String,
    toggle_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHotkey {
    binding: JsonShortcutBinding,
    normalized_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHotstring {
    trigger: String,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedKey {
    binding_key: String,
    display_text: String,
}

type PersistedSettings = BTreeMap<String, bool>;

#[no_mangle]
pub extern "C" fn register_commands_json() -> i32 {
    let json = build_commands_json_from_host().unwrap_or_else(|_| "[]".to_string());
    store_response_string(&json)
}

#[no_mangle]
pub extern "C" fn settings_schema_json() -> i32 {
    let json = build_settings_schema_json_from_host()
        .unwrap_or_else(|_| "{\"categories\":[],\"items\":[]}".to_string());
    store_response_string(&json)
}

#[no_mangle]
pub extern "C" fn execute(command_id_ptr: i32, command_id_len: i32) -> i32 {
    let command_id = match read_guest_string(command_id_ptr, command_id_len) {
        Ok(command_id) => command_id,
        Err(code) => return code,
    };

    let commands = match build_registered_commands_from_host() {
        Ok(commands) => commands,
        Err(_) => return 4,
    };

    match execute_registered_command(&commands, &command_id, write_text_to_host) {
        Ok(()) => 0,
        Err(_) => 5,
    }
}

fn build_commands_json_from_host() -> Result<String, String> {
    let commands = build_registered_commands_from_host()?;
    let descriptors: Vec<&CommandDescriptor> =
        commands.iter().map(|command| &command.descriptor).collect();
    serde_json::to_string(&descriptors)
        .map_err(|err| format!("Could not serialize commands: {err}"))
}

fn build_settings_schema_json_from_host() -> Result<String, String> {
    let storage_entries = read_storage_entries_from_host()?;
    let snapshots = load_snapshot_records(&storage_entries, read_storage_text_from_host);
    let discovered_scripts = discover_scripts(&snapshots);
    let schema = build_settings_schema(&discovered_scripts);
    serde_json::to_string(&schema)
        .map_err(|err| format!("Could not serialize settings schema: {err}"))
}

fn build_registered_commands_from_host() -> Result<Vec<RegisteredCommand>, String> {
    let storage_entries = read_storage_entries_from_host()?;
    let snapshots = load_snapshot_records(&storage_entries, read_storage_text_from_host);
    let settings = read_settings_from_host().unwrap_or_default();
    let discovered_scripts = discover_scripts(&snapshots);
    Ok(build_registered_commands(&discovered_scripts, &settings))
}

fn discover_scripts(snapshots: &[SnapshotRecord]) -> Vec<DiscoveredScript> {
    let mut scripts = Vec::new();

    for snapshot in snapshots {
        let script_name = script_name_from_path(&snapshot.script_path);
        let script_tag = normalize_tag(&script_name);
        let script_category_key = stable_settings_key(&[&snapshot.script_path, "category"]);
        let script_toggle_key = stable_settings_key(&[&snapshot.script_path, "script"]);
        let mut commands = Vec::new();

        for hotkey in parse_hotkeys(&snapshot.script_text) {
            let id = stable_command_id(&[&snapshot.script_path, "hotkey", &hotkey.normalized_text]);
            let toggle_label = hotkey.normalized_text.clone();
            commands.push(DiscoveredCommand {
                toggle_key: stable_settings_key(&[&snapshot.script_path, "command", &id]),
                toggle_label,
                registered: RegisteredCommand {
                    descriptor: CommandDescriptor {
                        id,
                        name: format!("AHK: {script_name} : {}", hotkey.normalized_text),
                        tags: command_tags(&script_tag),
                        shortcut_text: hotkey.normalized_text,
                        cmd: Some(hotkey.binding),
                    },
                    trigger_text: None,
                },
            });
        }

        for hotstring in parse_hotstrings(&snapshot.script_text) {
            let id = stable_command_id(&[
                &snapshot.script_path,
                "hotstring",
                &hotstring.trigger,
                &hotstring.replacement,
            ]);
            let preview = replacement_preview(&hotstring.replacement);
            commands.push(DiscoveredCommand {
                toggle_key: stable_settings_key(&[&snapshot.script_path, "command", &id]),
                toggle_label: format!("{} -> {}", hotstring.trigger, preview),
                registered: RegisteredCommand {
                    descriptor: CommandDescriptor {
                        id,
                        name: format!("AHK: {script_name} : {} -> {}", hotstring.trigger, preview),
                        tags: command_tags(&script_tag),
                        shortcut_text: String::new(),
                        cmd: None,
                    },
                    trigger_text: Some(hotstring.trigger),
                },
            });
        }

        scripts.push(DiscoveredScript {
            script_path: snapshot.script_path.clone(),
            script_name,
            category_key: script_category_key,
            toggle_key: script_toggle_key,
            commands,
        });
    }

    scripts
}

fn build_registered_commands(
    discovered_scripts: &[DiscoveredScript],
    settings: &PersistedSettings,
) -> Vec<RegisteredCommand> {
    let mut commands = Vec::new();

    for script in discovered_scripts {
        if !setting_enabled(settings, &script.toggle_key, true) {
            continue;
        }

        for command in &script.commands {
            if setting_enabled(settings, &command.toggle_key, true) {
                commands.push(command.registered.clone());
            }
        }
    }

    commands
}

fn build_settings_schema(discovered_scripts: &[DiscoveredScript]) -> SettingsSchemaDescriptor {
    let mut categories = Vec::new();
    let mut items = Vec::new();

    for script in discovered_scripts {
        categories.push(SettingsSchemaCategoryDescriptor {
            key: script.category_key.clone(),
            label: script.script_name.clone(),
            description: Some(script.script_path.clone()),
            toggle_key: Some(script.toggle_key.clone()),
            default_collapsed: true,
        });
        items.push(SettingsSchemaItemDescriptor {
            key: script.toggle_key.clone(),
            label: "Enabled".to_string(),
            description: None,
            category: Some(script.category_key.clone()),
            kind: SettingsSchemaItemType::Toggle,
            default: true,
        });

        for command in &script.commands {
            items.push(SettingsSchemaItemDescriptor {
                key: command.toggle_key.clone(),
                label: command.toggle_label.clone(),
                description: None,
                category: Some(script.category_key.clone()),
                kind: SettingsSchemaItemType::Toggle,
                default: true,
            });
        }
    }

    SettingsSchemaDescriptor { categories, items }
}

fn load_snapshot_records(
    storage_entries: &[String],
    mut reader: impl FnMut(&str) -> Result<String, String>,
) -> Vec<SnapshotRecord> {
    let mut snapshots = Vec::new();

    for storage_entry in storage_entries {
        if !is_snapshot_storage_entry(storage_entry) {
            continue;
        }

        let raw = match reader(storage_entry) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let raw = raw.trim_start_matches('\u{feff}');
        let snapshot = match serde_json::from_str::<SnapshotRecord>(raw) {
            Ok(snapshot) => snapshot,
            Err(_) => continue,
        };
        snapshots.push(snapshot);
    }

    snapshots.sort_by(|left, right| left.script_path.cmp(&right.script_path));
    snapshots
}

fn is_snapshot_storage_entry(storage_entry: &str) -> bool {
    storage_entry.starts_with(SNAPSHOT_DIRECTORY_PREFIX)
        && storage_entry.ends_with(SNAPSHOT_EXTENSION)
}

fn read_storage_entries_from_host() -> Result<Vec<String>, String> {
    let json = read_host_text_with_retry(
        "host_list_storage_entries_json",
        INITIAL_HOST_BUFFER_CAPACITY,
        MAX_HOST_BUFFER_CAPACITY,
        host::list_storage_entries_json,
    )?;
    serde_json::from_str(&json).map_err(|err| format!("Could not parse storage entry JSON: {err}"))
}

fn read_storage_text_from_host(path: &str) -> Result<String, String> {
    read_host_text_with_retry(
        "host_read_storage_text",
        INITIAL_HOST_BUFFER_CAPACITY,
        MAX_HOST_BUFFER_CAPACITY,
        |buffer| host::read_storage_text(path, buffer),
    )
}

fn read_settings_from_host() -> Result<PersistedSettings, String> {
    let json = read_host_text_with_retry(
        "host_read_settings_json",
        INITIAL_HOST_BUFFER_CAPACITY,
        MAX_HOST_BUFFER_CAPACITY,
        host::read_settings_json,
    )?;
    serde_json::from_str(&json).map_err(|err| format!("Could not parse settings JSON: {err}"))
}

fn execute_registered_command(
    commands: &[RegisteredCommand],
    command_id: &str,
    mut writer: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let command = commands
        .iter()
        .find(|command| command.descriptor.id == command_id)
        .ok_or_else(|| format!("Unknown AHK command id: {command_id}"))?;

    let trigger_text = command
        .trigger_text
        .as_deref()
        .ok_or_else(|| format!("AHK command {command_id} is shortcut-backed"))?;

    writer(trigger_text)
}

fn setting_enabled(settings: &PersistedSettings, key: &str, default: bool) -> bool {
    settings.get(key).copied().unwrap_or(default)
}

fn parse_hotkeys(script_text: &str) -> Vec<ParsedHotkey> {
    let mut hotkeys = Vec::new();

    for line in collect_plain_script_lines(script_text) {
        let Some((lhs, _rhs)) = line.split_once("::") else {
            continue;
        };

        let candidate = lhs.trim();
        if candidate.is_empty() || candidate.contains(':') || candidate.contains('&') {
            continue;
        }

        if let Some(hotkey) = parse_hotkey_candidate(candidate) {
            hotkeys.push(hotkey);
        }
    }

    hotkeys
}

fn parse_hotstrings(script_text: &str) -> Vec<ParsedHotstring> {
    let mut hotstrings = Vec::new();

    for line in collect_plain_script_lines(script_text) {
        let Some((descriptor, replacement)) = line.split_once("::") else {
            continue;
        };

        let descriptor = descriptor.trim();
        if !descriptor.starts_with(':') {
            continue;
        }

        let Some((options, trigger)) = parse_hotstring_descriptor(descriptor) else {
            continue;
        };
        if !options.contains('*') {
            continue;
        }

        let replacement = replacement.trim_end();
        if replacement.is_empty() {
            continue;
        }

        hotstrings.push(ParsedHotstring {
            trigger: trigger.to_string(),
            replacement: replacement.to_string(),
        });
    }

    hotstrings
}

fn collect_plain_script_lines(script_text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut in_conditional_hotkeys = false;

    for raw_line in script_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some(is_conditional) = parse_hotif_directive(line) {
            in_conditional_hotkeys = is_conditional;
            continue;
        }

        if in_conditional_hotkeys {
            continue;
        }

        lines.push(line);
    }

    lines
}

fn parse_hotif_directive(line: &str) -> Option<bool> {
    if let Some(rest) = line.strip_prefix("#HotIf") {
        return Some(!rest.trim().is_empty());
    }
    if let Some(rest) = line.strip_prefix("#If") {
        return Some(!rest.trim().is_empty());
    }
    None
}

fn parse_hotkey_candidate(candidate: &str) -> Option<ParsedHotkey> {
    let candidate = trim_hotkey_prefixes(candidate).trim();
    if candidate.is_empty() || candidate.contains(char::is_whitespace) {
        return None;
    }

    let mut mods = Vec::new();
    let mut chars = candidate.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        let modifier = match ch {
            '^' => Some("ctrl"),
            '+' => Some("shift"),
            '!' => Some("alt"),
            '#' => Some("win"),
            _ => None,
        };

        let Some(modifier) = modifier else {
            break;
        };
        chars.next();
        if !mods.iter().any(|existing| *existing == modifier) {
            mods.push(modifier);
        }
    }

    let key_token: String = chars.collect();
    let key_token = key_token.trim();
    if key_token.is_empty() {
        return None;
    }

    let key = parse_key_token(key_token)?;
    let ordered_mods = ordered_modifiers(&mods);
    let normalized_text = shortcut_display_text(&ordered_mods, &key.display_text);

    Some(ParsedHotkey {
        binding: JsonShortcutBinding {
            mods: ordered_mods.into_iter().map(str::to_string).collect(),
            key: key.binding_key,
        },
        normalized_text,
    })
}

fn parse_hotstring_descriptor(descriptor: &str) -> Option<(&str, &str)> {
    let remainder = descriptor.strip_prefix(':')?;
    let split_index = remainder.find(':')?;
    let options = &remainder[..split_index];
    let trigger = remainder[split_index + 1..].trim();
    if trigger.is_empty() {
        return None;
    }

    Some((options, trigger))
}

fn parse_key_token(token: &str) -> Option<ParsedKey> {
    if token.len() == 1 {
        let ch = token.chars().next()?;
        return parse_single_key(ch);
    }

    let upper = token.to_ascii_uppercase();
    let (binding_key, display_text) = match upper.as_str() {
        "ENTER" => ("Enter", "Enter"),
        "SPACE" => ("Space", "Space"),
        "TAB" => ("Tab", "Tab"),
        "ESC" | "ESCAPE" => ("Escape", "Esc"),
        "DEL" | "DELETE" => ("Delete", "Del"),
        "BACKSPACE" | "BS" => ("BackSpace", "Backspace"),
        "HOME" => ("Home", "Home"),
        "END" => ("End", "End"),
        "PGUP" | "PAGEUP" => ("PageUp", "PgUp"),
        "PGDN" | "PAGEDOWN" => ("PageDown", "PgDn"),
        "INS" | "INSERT" => ("Insert", "Ins"),
        "PRINTSCREEN" | "PRTSC" => ("PrintScreen", "PrtSc"),
        "SCROLLLOCK" | "SCRLK" => ("ScrollLock", "ScrLk"),
        "PAUSE" => ("Pause", "Pause"),
        "LEFT" => ("LeftArrow", "Left"),
        "RIGHT" => ("RightArrow", "Right"),
        "UP" => ("UpArrow", "Up"),
        "DOWN" => ("DownArrow", "Down"),
        _ => {
            if let Some(function_key) = parse_function_key(&upper) {
                return Some(function_key);
            }
            return None;
        }
    };

    Some(ParsedKey {
        binding_key: binding_key.to_string(),
        display_text: display_text.to_string(),
    })
}

fn parse_single_key(ch: char) -> Option<ParsedKey> {
    if ch.is_ascii_alphabetic() {
        let upper = ch.to_ascii_uppercase();
        return Some(ParsedKey {
            binding_key: format!("Key{upper}"),
            display_text: upper.to_string(),
        });
    }

    if ch.is_ascii_digit() {
        return Some(ParsedKey {
            binding_key: format!("Key{ch}"),
            display_text: ch.to_string(),
        });
    }

    let (binding_key, display_text) = match ch {
        ';' | ':' => ("Semicolon", ";"),
        '=' | '+' => ("Equal", "="),
        ',' | '<' => ("Comma", ","),
        '-' | '_' => ("Minus", "-"),
        '.' | '>' => ("Period", "."),
        '/' | '?' => ("Slash", "/"),
        '`' | '~' => ("Grave", "`"),
        '[' | '{' => ("LeftBracket", "["),
        '\\' | '|' => ("Backslash", "\\"),
        ']' | '}' => ("RightBracket", "]"),
        '\'' | '"' => ("Apostrophe", "'"),
        _ => return None,
    };

    Some(ParsedKey {
        binding_key: binding_key.to_string(),
        display_text: display_text.to_string(),
    })
}

fn parse_function_key(token: &str) -> Option<ParsedKey> {
    let number = token.strip_prefix('F')?.parse::<u8>().ok()?;
    if !(1..=12).contains(&number) {
        return None;
    }

    Some(ParsedKey {
        binding_key: format!("F{number}"),
        display_text: format!("F{number}"),
    })
}

fn trim_hotkey_prefixes(mut candidate: &str) -> &str {
    while let Some(first) = candidate.chars().next() {
        match first {
            '~' | '$' | '*' => candidate = &candidate[first.len_utf8()..],
            _ => break,
        }
    }

    candidate
}

fn ordered_modifiers<'a>(mods: &'a [&'a str]) -> Vec<&'a str> {
    ["ctrl", "shift", "alt", "win"]
        .into_iter()
        .filter(|modifier| mods.contains(modifier))
        .collect()
}

fn shortcut_display_text(mods: &[&str], key_display_text: &str) -> String {
    let mut parts = Vec::with_capacity(mods.len() + 1);
    for modifier in mods {
        let label = match *modifier {
            "ctrl" => "Ctrl",
            "shift" => "Shift",
            "alt" => "Alt",
            "win" => "Win",
            _ => continue,
        };
        parts.push(label);
    }
    parts.push(key_display_text);
    parts.join("+")
}

fn command_tags(script_tag: &str) -> Vec<String> {
    let mut tags = vec!["ahk".to_string()];
    if !script_tag.is_empty() {
        tags.push(script_tag.to_string());
    }
    tags
}

fn replacement_preview(replacement: &str) -> String {
    let collapsed = replacement.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let preview: String = chars.by_ref().take(PREVIEW_CHAR_LIMIT).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        collapsed
    }
}

fn script_name_from_path(script_path: &str) -> String {
    let file_name = script_path
        .rsplit('\\')
        .next()
        .unwrap_or(script_path)
        .rsplit('/')
        .next()
        .unwrap_or(script_path);
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    stem.trim().to_string()
}

fn normalize_tag(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('-');
            last_was_separator = true;
        }
    }

    normalized.trim_matches('-').to_string()
}

fn stable_command_id(parts: &[&str]) -> String {
    format!("ahk_{}", stable_hash_suffix(parts))
}

fn stable_settings_key(parts: &[&str]) -> String {
    format!("ahk_setting_{}", stable_hash_suffix(parts))
}

fn stable_hash_suffix(parts: &[&str]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for part in parts {
        hash = fnv1a_update(hash, part.as_bytes());
        hash = fnv1a_update(hash, &[0xff]);
    }
    format!("{hash:016X}")
}

fn fnv1a_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn write_text_to_host(text: &str) -> Result<(), String> {
    let exit_code = host::write_text(text);
    if exit_code == 0 {
        Ok(())
    } else {
        Err(format!("host_write_text failed with code {exit_code}"))
    }
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

    const UP_ARROW: &str = "\u{2B06}\u{FE0F}";
    const DOWN_ARROW: &str = "\u{2B07}\u{FE0F}";
    const QUESTION_MARK: &str = "\u{2753}";
    const EXCLAMATION_MARK: &str = "\u{2757}";
    const POUND_SIGN: &str = "\u{00A3}";

    fn snapshot_record(script_text: &str) -> SnapshotRecord {
        SnapshotRecord {
            script_path: "C:\\Scripts\\Demo.ahk".to_string(),
            script_text: script_text.to_string(),
        }
    }

    #[test]
    fn parses_plain_hotkeys() {
        let hotkeys =
            parse_hotkeys("^h::MsgBox \"hi\"\n^;::SendText \"today\"\n!Left::Send \"{Left}\"");

        assert_eq!(hotkeys.len(), 3);
        assert_eq!(hotkeys[0].normalized_text, "Ctrl+H");
        assert_eq!(hotkeys[0].binding.key, "KeyH");
        assert_eq!(hotkeys[1].normalized_text, "Ctrl+;");
        assert_eq!(hotkeys[1].binding.key, "Semicolon");
        assert_eq!(hotkeys[2].normalized_text, "Alt+Left");
        assert_eq!(hotkeys[2].binding.key, "LeftArrow");
    }

    #[test]
    fn parses_hotkeys_with_ahk_prefix_markers() {
        let hotkeys = parse_hotkeys("~^h::MsgBox \"hi\"\n*$!j::Send \"{Left}\"");

        assert_eq!(hotkeys.len(), 2);
        assert_eq!(hotkeys[0].normalized_text, "Ctrl+H");
        assert_eq!(hotkeys[1].normalized_text, "Alt+J");
    }

    #[test]
    fn ignores_hotstrings_when_parsing_hotkeys() {
        let hotkeys = parse_hotkeys(&format!(":?*:up;::{UP_ARROW}\n^h::MsgBox \"hi\""));

        assert_eq!(hotkeys.len(), 1);
        assert_eq!(hotkeys[0].normalized_text, "Ctrl+H");
    }

    #[test]
    fn parses_one_line_hotstrings_with_unicode_replacements() {
        let hotstrings = parse_hotstrings(&format!(":?*:up;::{UP_ARROW}\n:?*C:gbp;::{POUND_SIGN}"));

        assert_eq!(
            hotstrings,
            vec![
                ParsedHotstring {
                    trigger: "up;".to_string(),
                    replacement: UP_ARROW.to_string(),
                },
                ParsedHotstring {
                    trigger: "gbp;".to_string(),
                    replacement: POUND_SIGN.to_string(),
                },
            ]
        );
    }

    #[test]
    fn parses_punctuation_hotstring_triggers() {
        let hotstrings = parse_hotstrings(&format!(
            ":?*:?;::{QUESTION_MARK}\n:?*:!;::{EXCLAMATION_MARK}"
        ));

        assert_eq!(hotstrings.len(), 2);
        assert_eq!(hotstrings[0].trigger, "?;");
        assert_eq!(hotstrings[1].trigger, "!;");
    }

    #[test]
    fn skips_hotstrings_without_immediate_expansion() {
        let hotstrings = parse_hotstrings(&format!(":?:todo;::[](#todo)\n:?*:up;::{UP_ARROW}"));

        assert_eq!(hotstrings.len(), 1);
        assert_eq!(hotstrings[0].trigger, "up;");
    }

    #[test]
    fn skips_body_style_hotstrings_and_function_created_hotstrings() {
        let hotstrings = parse_hotstrings(&format!(
            "Hotstring(\"EndChars\", \" \")\n:?*:today;::\n    SendText FormatTime(A_Now, \"dd MMM\")\nreturn\n:?*:up;::{UP_ARROW}"
        ));

        assert_eq!(hotstrings.len(), 1);
        assert_eq!(hotstrings[0].trigger, "up;");
    }

    #[test]
    fn ignores_hotkeys_inside_hotif_blocks() {
        let hotkeys = parse_hotkeys(
            "#HotIf WinActive(\"ahk_exe code.exe\")\n^h::MsgBox \"skip\"\n#HotIf\n^j::MsgBox \"keep\"",
        );

        assert_eq!(hotkeys.len(), 1);
        assert_eq!(hotkeys[0].normalized_text, "Ctrl+J");
    }

    #[test]
    fn loads_snapshot_records_from_storage_entries() {
        let storage_entries = vec![
            "scripts/demo.json".to_string(),
            "notes.txt".to_string(),
            "scripts/broken.json".to_string(),
            "scripts/bom.json".to_string(),
        ];
        let snapshots = load_snapshot_records(&storage_entries, |path| {
            match path {
            "scripts/demo.json" => Ok(
                r#"{"schema_version":1,"script_path":"C:\\Scripts\\Demo.ahk","script_text":"^h::MsgBox \"hi\""}"#
                    .to_string(),
            ),
            "scripts/broken.json" => Ok("{".to_string()),
            "scripts/bom.json" => Ok(
                "\u{feff}{\"schema_version\":1,\"script_path\":\"C:\\\\Scripts\\\\Bom.ahk\",\"script_text\":\":?*:up;::\\u2B06\\uFE0F\"}"
                    .to_string(),
            ),
            _ => Err("missing".to_string()),
        }
        });

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].script_path, "C:\\Scripts\\Bom.ahk");
        assert_eq!(snapshots[1].script_path, "C:\\Scripts\\Demo.ahk");
    }

    #[test]
    fn builds_hotkey_and_hotstring_commands_from_snapshots() {
        let snapshots = vec![snapshot_record(&format!(
            "^h::MsgBox \"hi\"\n:?*:up;::{UP_ARROW}"
        ))];
        let commands =
            build_registered_commands(&discover_scripts(&snapshots), &PersistedSettings::default());

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].descriptor.name, "AHK: Demo : Ctrl+H");
        assert_eq!(commands[0].descriptor.shortcut_text, "Ctrl+H");
        assert!(commands[0].descriptor.cmd.is_some());
        assert!(commands[0].trigger_text.is_none());
        assert_eq!(
            commands[1].descriptor.name,
            format!("AHK: Demo : up; -> {UP_ARROW}")
        );
        assert_eq!(commands[1].descriptor.shortcut_text, "");
        assert!(commands[1].descriptor.cmd.is_none());
        assert_eq!(commands[1].trigger_text.as_deref(), Some("up;"));
    }

    #[test]
    fn replacement_preview_truncates_long_text() {
        let preview =
            replacement_preview("this replacement text is definitely longer than the limit");

        assert_eq!(preview, "this replacement text is...");
    }

    #[test]
    fn executes_hotstring_commands_by_typing_trigger_text() {
        let commands = build_registered_commands(
            &discover_scripts(&[snapshot_record(&format!(":?*:up;::{UP_ARROW}"))]),
            &PersistedSettings::default(),
        );
        let hotstring_id = commands[0].descriptor.id.clone();
        let mut typed = Vec::new();

        execute_registered_command(&commands, &hotstring_id, |text| {
            typed.push(text.to_string());
            Ok(())
        })
        .expect("hotstring command should execute");

        assert_eq!(typed, vec!["up;".to_string()]);
    }

    #[test]
    fn refuses_to_execute_shortcut_backed_commands_through_plugin_path() {
        let commands = build_registered_commands(
            &discover_scripts(&[snapshot_record("^h::MsgBox \"hi\"")]),
            &PersistedSettings::default(),
        );
        let shortcut_id = commands[0].descriptor.id.clone();
        let err = execute_registered_command(&commands, &shortcut_id, |_| Ok(()))
            .expect_err("shortcut-backed command should not execute through plugin path");

        assert!(err.contains("shortcut-backed"));
    }

    #[test]
    fn serializes_hotstring_command_without_cmd_binding() {
        let commands = build_registered_commands(
            &discover_scripts(&[snapshot_record(&format!(":?*:up;::{UP_ARROW}"))]),
            &PersistedSettings::default(),
        );
        let json =
            serde_json::to_string(&commands[0].descriptor).expect("descriptor should serialize");

        assert!(json.contains("\"shortcut_text\":\"\""));
        assert!(!json.contains("\"cmd\""));
    }

    #[test]
    fn builds_commands_from_realistic_hotstring_script() {
        let script_text = format!(
            concat!(
                "#NoEnv\n",
                "#Include \"C:\\Users\\limgr\\Documents\\GitHub\\global_palette\\extensions\\bundled\\plugins\\ahk_agent\\OmniPaletteAgent.ahk\"\n",
                "SendMode Input\n",
                "SetWorkingDir %A_ScriptDir%\n",
                "#SingleInstance Force\n",
                "Hotstring(\"EndChars\", \" \")\n",
                ":?*:up;::{}\n",
                ":?*:down;::{}\n",
                ":?*:?;::{}\n",
            ),
            UP_ARROW, DOWN_ARROW, QUESTION_MARK,
        );
        let commands = build_registered_commands(
            &discover_scripts(&[snapshot_record(&script_text)]),
            &PersistedSettings::default(),
        );

        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands
                .iter()
                .map(|command| command.descriptor.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                format!("AHK: Demo : up; -> {UP_ARROW}").as_str(),
                format!("AHK: Demo : down; -> {DOWN_ARROW}").as_str(),
                format!("AHK: Demo : ?; -> {QUESTION_MARK}").as_str(),
            ]
        );
    }

    #[test]
    fn builds_settings_schema_with_script_and_command_toggles() {
        let script_text = format!("^h::MsgBox \"hi\"\n:?*:up;::{UP_ARROW}");
        let schema = build_settings_schema(&discover_scripts(&[snapshot_record(&script_text)]));

        assert_eq!(schema.categories.len(), 1);
        assert_eq!(schema.categories[0].label, "Demo");
        assert_eq!(
            schema.categories[0].toggle_key.as_deref(),
            Some(schema.items[0].key.as_str())
        );
        assert!(schema.categories[0].default_collapsed);
        assert_eq!(schema.items.len(), 3);
        assert_eq!(schema.items[0].label, "Enabled");
        assert_eq!(
            schema.items[0].category.as_deref(),
            Some(schema.categories[0].key.as_str())
        );
        assert_eq!(schema.items[1].label, "Ctrl+H");
        assert_eq!(schema.items[2].label, format!("up; -> {UP_ARROW}"));
    }

    #[test]
    fn disables_entire_scripts_via_settings() {
        let snapshots = [snapshot_record("^h::MsgBox \"hi\"\n^j::MsgBox \"there\"")];
        let discovered = discover_scripts(&snapshots);
        let mut settings = PersistedSettings::default();
        settings.insert(discovered[0].toggle_key.clone(), false);

        let commands = build_registered_commands(&discovered, &settings);

        assert!(commands.is_empty());
    }

    #[test]
    fn disables_individual_commands_via_settings() {
        let snapshots = [snapshot_record(&format!(
            "^h::MsgBox \"hi\"\n:?*:up;::{UP_ARROW}"
        ))];
        let discovered = discover_scripts(&snapshots);
        let mut settings = PersistedSettings::default();
        settings.insert(discovered[0].commands[1].toggle_key.clone(), false);

        let commands = build_registered_commands(&discovered, &settings);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].descriptor.name, "AHK: Demo : Ctrl+H");
    }

    #[test]
    fn reads_storage_entry_json_from_host() {
        let json = serde_json::to_string(&vec!["scripts/alpha.json", "scripts/beta.json"])
            .expect("json should serialize");
        let entries: Vec<String> =
            serde_json::from_str(&json).expect("json should deserialize into entries");

        assert_eq!(
            entries,
            vec![
                "scripts/alpha.json".to_string(),
                "scripts/beta.json".to_string(),
            ]
        );
    }

    #[test]
    fn read_host_text_with_retry_grows_until_content_fits() {
        let mut calls = 0;
        let text = read_host_text_with_retry("mock_reader", 4, 32, |buffer| {
            calls += 1;
            let payload = b"hello world";
            if buffer.len() < payload.len() {
                return HOST_BUFFER_TOO_SMALL_CODE;
            }

            buffer[..payload.len()].copy_from_slice(payload);
            payload.len() as i32
        })
        .expect("reader should succeed");

        assert_eq!(text, "hello world");
        assert_eq!(calls, 3);
    }

    #[test]
    fn read_host_text_with_retry_reports_capacity_exhaustion() {
        let err =
            read_host_text_with_retry("mock_reader", 4, 8, |_buffer| HOST_BUFFER_TOO_SMALL_CODE)
                .expect_err("reader should report exhausted capacity");

        assert!(err.contains("max buffer capacity"));
    }
}
