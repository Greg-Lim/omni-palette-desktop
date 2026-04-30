// Read extension files and build a resolved runtime config.

use std::{fs, path::Path};

use crate::config::extension::{
    ActionConfig, ActionsMetadataConfig, Config, PackageManifestConfig,
    PlatformImplementationConfig, SkippedImplementation,
};

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, String> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|e| format!("Could not read file: {e}"))?;

    if let Ok(platform_config) = toml::from_str::<PlatformImplementationConfig>(&content) {
        return load_split_config(path, platform_config);
    }

    toml::from_str(&content).map_err(|e| format!("Could not parse config: {e}"))
}

fn load_split_config(
    path: &Path,
    platform_config: PlatformImplementationConfig,
) -> Result<Config, String> {
    if platform_config.version != 3 {
        return Err(format!(
            "Unsupported platform implementation version: {}",
            platform_config.version
        ));
    }

    let extension_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("Could not infer extension id from {}", path.display()))?;
    let (manifest_path, actions_path) = metadata_paths_for_static_config(path, extension_id)
        .ok_or_else(|| {
            format!(
                "Could not find manifest.toml and actions.toml for split extension {}",
                path.display()
            )
        })?;

    let manifest: PackageManifestConfig =
        toml::from_str(&fs::read_to_string(&manifest_path).map_err(|err| {
            format!("Could not read manifest {}: {err}", manifest_path.display())
        })?)
        .map_err(|err| {
            format!(
                "Could not parse manifest {}: {err}",
                manifest_path.display()
            )
        })?;
    let actions_metadata: ActionsMetadataConfig = toml::from_str(
        &fs::read_to_string(&actions_path)
            .map_err(|err| format!("Could not read actions {}: {err}", actions_path.display()))?,
    )
    .map_err(|err| format!("Could not parse actions {}: {err}", actions_path.display()))?;

    resolved_config_from_split(manifest, actions_metadata, platform_config)
}

fn metadata_paths_for_static_config(
    path: &Path,
    extension_id: &str,
) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let static_dir = path.parent()?;
    let platform_root = static_dir.parent()?;
    let extension_root = platform_root.parent()?;

    let source_manifest = extension_root.join("manifest.toml");
    let source_actions = extension_root.join("actions.toml");
    if source_manifest.is_file() && source_actions.is_file() {
        return Some((source_manifest, source_actions));
    }

    let install_root = static_dir.parent()?;
    let metadata_root = install_root.join("metadata").join(extension_id);
    let installed_manifest = metadata_root.join("manifest.toml");
    let installed_actions = metadata_root.join("actions.toml");
    if installed_manifest.is_file() && installed_actions.is_file() {
        return Some((installed_manifest, installed_actions));
    }

    None
}

pub fn resolved_config_from_split(
    manifest: PackageManifestConfig,
    actions_metadata: ActionsMetadataConfig,
    platform_config: PlatformImplementationConfig,
) -> Result<Config, String> {
    if manifest.schema_version != 1 {
        return Err(format!(
            "Unsupported package manifest schema version: {}",
            manifest.schema_version
        ));
    }
    if actions_metadata.schema_version != 1 {
        return Err(format!(
            "Unsupported actions schema version: {}",
            actions_metadata.schema_version
        ));
    }
    if manifest.id.trim().is_empty() {
        return Err("Package manifest id must not be empty".to_string());
    }
    if manifest.kind != "static" {
        return Err(format!("Unsupported extension kind: {}", manifest.kind));
    }

    let mut actions = std::collections::HashMap::new();
    for (action_id, metadata) in &actions_metadata.actions {
        let Some(implementation) = platform_config.actions.get(action_id) else {
            return Err(format!(
                "Action '{action_id}' has no platform implementation or pass entry"
            ));
        };
        if implementation.cmd.is_some() && implementation.implementation.is_some() {
            return Err(format!(
                "Action '{action_id}' must not set both cmd and implementation"
            ));
        }
        if implementation.implementation == Some(SkippedImplementation::Pass) {
            continue;
        }
        let Some(cmd) = implementation.cmd.clone() else {
            return Err(format!(
                "Action '{action_id}' must set cmd or implementation = \"pass\""
            ));
        };

        actions.insert(
            action_id.clone(),
            ActionConfig {
                name: metadata.name.clone(),
                focus_state: metadata.focus_state,
                when: metadata.when.clone(),
                priority: metadata.priority,
                tags: metadata.tags.clone(),
                favorite: metadata.favorite,
                cmd,
            },
        );
    }

    for action_id in platform_config.actions.keys() {
        if !actions_metadata.actions.contains_key(action_id) {
            return Err(format!(
                "Platform implementation references unknown action '{action_id}'"
            ));
        }
    }

    let app_defaults = actions_metadata.app;
    Ok(Config {
        version: 2,
        platform: platform_config.platform,
        app: crate::config::extension::AppConfig {
            id: manifest.id,
            name: manifest.name,
            process_name: platform_config.process_name,
            default_focus_state: app_defaults
                .as_ref()
                .and_then(|app| app.default_focus_state),
            default_tags: app_defaults.and_then(|app| app.default_tags),
        },
        setting_categories: actions_metadata.setting_categories,
        settings: actions_metadata.settings,
        actions,
    })
}

#[test]
fn deserializes_inline_toml() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"
process_name = "chrome.exe"

[[settings]]
key = "chrome.new_ui"
label = "Enable new UI"
type = "toggle"
default = true

[actions]

[actions.new_tab]
name = "New tab"
focus_state = "focused"
cmd = { mods = ["ctrl"], key = "KeyT" }
"#;

    let cfg: Config = toml::from_str(content).expect("should deserialize");
    assert_eq!(cfg.app.id, "chrome");
    assert!(cfg.actions.contains_key("new_tab"));
    // println!("{cfg:?}")
}

#[test]
fn deserializes_context_condition() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.bold]
name = "Bold text"
cmd = { mods = ["ctrl"], key = "KeyB" }

[actions.bold.when]
any = ["ppt.selection.text", "ui.text_input"]
"#;

    let cfg: Config = toml::from_str(content).expect("should deserialize");
    let action = cfg.actions.get("bold").expect("action should exist");
    let when = action.when.as_ref().expect("condition should exist");
    assert_eq!(when.any, vec!["ppt.selection.text", "ui.text_input"]);
}

#[test]
fn deserializes_sequence_command() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.select_draw_pen]
name = "Select drawing pen"
cmd = { sequence = [
    { mods = ["alt"], key = "KeyJ" },
    { key = "KeyI" },
] }
"#;

    let cfg: Config = toml::from_str(content).expect("should deserialize");
    let action = cfg
        .actions
        .get("select_draw_pen")
        .expect("action should exist");
    match &action.cmd {
        crate::config::extension::CommandBinding::Sequence(sequence) => {
            assert_eq!(sequence.sequence.len(), 2);
            assert_eq!(
                sequence.sequence[0].mods,
                vec![crate::config::extension::Modifier::Alt]
            );
        }
        crate::config::extension::CommandBinding::Shortcut(_) => {
            panic!("sequence command should not parse as shortcut")
        }
    }
}

#[test]
fn sequence_keys_use_strict_hotkey_key_names() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.open_dialog]
name = "Open dialog"
cmd = { sequence = [
    { mods = ["alt"], key = "KeyN" },
    { key = "Key2" },
    { key = "Escape" },
] }
"#;

    let cfg: Config = toml::from_str(content).expect("should deserialize");
    let action = cfg.actions.get("open_dialog").expect("action should exist");
    match &action.cmd {
        crate::config::extension::CommandBinding::Sequence(sequence) => {
            assert_eq!(
                sequence.sequence[0].key,
                crate::config::extension::SequenceKeyConfig::Key(crate::domain::hotkey::Key::KeyN)
            );
            assert_eq!(
                sequence.sequence[1].key,
                crate::config::extension::SequenceKeyConfig::Key(crate::domain::hotkey::Key::Key2)
            );
            assert_eq!(
                sequence.sequence[2].key,
                crate::config::extension::SequenceKeyConfig::Key(
                    crate::domain::hotkey::Key::Escape
                )
            );
        }
        crate::config::extension::CommandBinding::Shortcut(_) => {
            panic!("sequence command should not parse as shortcut")
        }
    }
}

#[test]
fn rejects_app_level_priority() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"
process_name = "chrome.exe"
default_focus_state = "focused"
default_priority = "normal"

[actions]
"#;

    let err = toml::from_str::<Config>(content).expect_err("app priority should not deserialize");
    assert!(err.to_string().contains("default_priority"));
}

#[test]
fn rejects_old_combined_os_schema() {
    let content = r#"
version = 1

[app]
id = "chrome"
name = "Chrome"

[app.application_os_name]
windows = "chrome.exe"

[actions.new_tab]
name = "New tab"
cmd.windows = { mods = ["ctrl"], key = "KeyT" }
"#;

    let err = toml::from_str::<Config>(content).expect_err("old schema should not deserialize");
    let message = err.to_string();
    assert!(message.contains("platform") || message.contains("process_name"));
}

#[test]
fn rejects_missing_process_name() {
    let content = r#"
version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"

[actions]
"#;

    let err = toml::from_str::<Config>(content).expect_err("process_name should be required");
    assert!(err.to_string().contains("process_name"));
}

fn split_config_result(manifest: &str, actions: &str, platform: &str) -> Result<Config, String> {
    resolved_config_from_split(
        toml::from_str(manifest).expect("manifest should parse"),
        toml::from_str(actions).expect("actions should parse"),
        toml::from_str(platform).expect("platform should parse"),
    )
}

#[test]
fn split_config_joins_metadata_and_platform_commands() {
    let config = split_config_result(
        r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "1.0.0"
kind = "static"
"#,
        r#"schema_version = 1

[app]
default_focus_state = "focused"
default_tags = ["browser"]

[actions.new_tab]
name = "New tab"
priority = "high"
"#,
        r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
    )
    .expect("split config should resolve");

    assert_eq!(config.app.id, "chrome");
    assert_eq!(config.app.name, "Chrome");
    assert_eq!(config.app.process_name, "chrome.exe");
    assert!(config.actions.contains_key("new_tab"));
    assert_eq!(
        config.actions["new_tab"].priority,
        Some(crate::domain::action::CommandPriority::High)
    );
}

#[test]
fn split_config_accepts_pass_and_skips_runtime_action() {
    let config = split_config_result(
        r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "1.0.0"
kind = "static"
"#,
        r#"schema_version = 1

[actions.new_tab]
name = "New tab"

[actions.macos_only]
name = "macOS only"
"#,
        r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }

[actions.macos_only]
implementation = "pass"
"#,
    )
    .expect("pass should resolve");

    assert!(config.actions.contains_key("new_tab"));
    assert!(!config.actions.contains_key("macos_only"));
}

#[test]
fn split_config_rejects_missing_platform_mapping_without_pass() {
    let err = split_config_result(
        r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "1.0.0"
kind = "static"
"#,
        r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
        r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.other]
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
    )
    .expect_err("missing mapping should fail");

    assert!(err.contains("no platform implementation"));
}

#[test]
fn split_config_rejects_unknown_platform_action() {
    let err = split_config_result(
        r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "1.0.0"
kind = "static"
"#,
        r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
        r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
cmd = { mods = ["ctrl"], key = "KeyT" }

[actions.other]
cmd = { mods = ["ctrl"], key = "KeyO" }
"#,
    )
    .expect_err("unknown action should fail");

    assert!(err.contains("unknown action"));
}

#[test]
fn split_config_rejects_cmd_and_pass_together() {
    let err = split_config_result(
        r#"schema_version = 1
id = "chrome"
name = "Chrome"
version = "1.0.0"
kind = "static"
"#,
        r#"schema_version = 1

[actions.new_tab]
name = "New tab"
"#,
        r#"version = 3
platform = "windows"
process_name = "chrome.exe"

[actions.new_tab]
implementation = "pass"
cmd = { mods = ["ctrl"], key = "KeyT" }
"#,
    )
    .expect_err("cmd plus pass should fail");

    assert!(err.contains("both cmd and implementation"));
}
