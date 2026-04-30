// register action is for the user to register new actions given the context and action

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use log::{error, info};

use raw_window_handle::RawWindowHandle;

#[cfg(debug_assertions)]
use crate::core::performance::process_performance_snapshot_logger;

use crate::{
    config::extension::{
        ActionWhenConfig, CommandBinding, Config, KeyChord, KeySequenceStepConfig, Modifier,
        SequenceKeyConfig,
    },
    core::{
        extensions::{
            discovery::ExtensionDiscovery, extensions::load_config,
            settings::extension_settings_json,
        },
        plugins::{PluginApplication, PluginRegistry},
    },
    domain::{
        action::{
            normalize_context_tag, sequence_shortcut_text, Action, ActionContextCondition,
            ActionExecution, ActionId, ActionMetadata, ActionName, AppName, AppProcessName,
            ApplicationID, ContextRoot, FocusState, KeySequenceStep, Os, SequenceKey,
        },
        hotkey::{HotkeyModifiers, KeyboardShortcut},
    },
    platform::platform_interface::RawWindowHandleExt,
    platform::windows::sender::hotkey_sender::send_text,
};

#[derive(Default, Debug)]
pub struct MasterRegistry {
    // represents the global registry to determine all possible commands
    // 2 way: can be lazy generated when the user pulls up the palette or pregenerated.
    pub application_registry: HashMap<ApplicationID, Application>,
    pub application_process_name_id: HashMap<AppProcessName, ApplicationID>,
    pub plugin_registry: Arc<PluginRegistry>,
}

impl MasterRegistry {
    pub fn build(extension_discovery: &ExtensionDiscovery, current_os: Os) -> MasterRegistry {
        Self::build_internal(extension_discovery, current_os, false)
            .expect("best-effort registry build should not fail")
    }

    pub fn build_strict(
        extension_discovery: &ExtensionDiscovery,
        current_os: Os,
    ) -> Result<MasterRegistry, RegistryBuildError> {
        Self::build_internal(extension_discovery, current_os, true)
    }

    fn build_internal(
        extension_discovery: &ExtensionDiscovery,
        current_os: Os,
        fail_on_static_errors: bool,
    ) -> Result<MasterRegistry, RegistryBuildError> {
        let plugin_registry = Arc::new(PluginRegistry::load(
            extension_discovery.plugin_manifest_paths(),
            current_os,
            Arc::new(send_text),
            Arc::new(current_date_text),
            Arc::new(plugin_storage_root),
            Arc::new(plugin_settings_text),
            #[cfg(debug_assertions)]
            process_performance_snapshot_logger(),
        ));
        let mut master_registry = MasterRegistry {
            plugin_registry: Arc::clone(&plugin_registry),
            ..Default::default()
        };
        let mut errors = Vec::new();

        for path in extension_discovery.static_config_paths() {
            match load_config(&path).and_then(|c| Application::new(&c, &current_os)) {
                Ok(app) => {
                    let app_id = master_registry.application_registry.len() as u32;
                    info!("Successfully loaded extension: {:?}", path);
                    master_registry
                        .application_registry
                        .insert(app_id, app.clone());
                    master_registry
                        .application_process_name_id
                        .insert(app.application_process_name.clone(), app_id);
                }
                Err(err) => {
                    error!("Failed to load extension at {:?}: {}", path, err);
                    errors.push(format!("{}: {err}", path.display()));
                }
            }
        }

        for plugin_app in plugin_registry.applications() {
            let app_id = master_registry.application_registry.len() as u32;
            match Application::from_plugin(plugin_app) {
                Ok(app) => {
                    master_registry
                        .application_process_name_id
                        .insert(app.application_process_name.clone(), app_id);
                    master_registry.application_registry.insert(app_id, app);
                }
                Err(err) => error!(
                    "Failed to register WASM plugin application {}: {}",
                    plugin_app.plugin_id, err
                ),
            }
        }

        if fail_on_static_errors && !errors.is_empty() {
            Err(RegistryBuildError { errors })
        } else {
            Ok(master_registry)
        }
    }

    pub fn plugin_registry(&self) -> Arc<PluginRegistry> {
        Arc::clone(&self.plugin_registry)
    }
}

#[derive(Debug, Clone)]
pub struct RegistryBuildError {
    errors: Vec<String>,
}

impl std::fmt::Display for RegistryBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to reload extensions")?;
        for error in &self.errors {
            write!(f, "; {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for RegistryBuildError {}

#[derive(Debug)]
pub struct UnitAction {
    // This struct will be use for search and generating the UI
    pub app_name: AppName,
    pub action_name: ActionName,
    pub focus_state: FocusState,
    pub execution: ActionExecution,
    pub shortcut_text: String,
    pub metadata: ActionMetadata,
    pub target_window: Option<RawWindowHandle>,
}

impl MasterRegistry {
    pub fn get_actions(&self, context: &ContextRoot) -> Vec<UnitAction> {
        let mut all_actions = vec![];

        // Extract background actions
        for bg_context in &context.bg_context {
            let Some(process_name) = bg_context.get_app_process_name() else {
                continue;
            };

            let Some(&app_id) = self.application_process_name_id.get(&process_name) else {
                continue;
            };

            let Some(app) = self.application_registry.get(&app_id) else {
                continue;
            };

            for action in app
                .application_registry
                .values()
                .filter(|a| a.focus_state == FocusState::Background && a.when.any.is_empty())
            {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_name: action.name.clone(),
                    focus_state: FocusState::Background,
                    execution: action.execution.clone(),
                    shortcut_text: action.shortcut_text.clone(),
                    metadata: action.metadata.clone(),
                    target_window: Some(*bg_context),
                });
            }
        }

        // Extract focused actions (only for the single active/foreground window)
        'add_focused_actions: {
            let Some(active) = context.get_active() else {
                break 'add_focused_actions;
            };

            let Some(process_name) = active.get_app_process_name() else {
                break 'add_focused_actions;
            };

            let Some(app_id) = self.application_process_name_id.get(&process_name) else {
                break 'add_focused_actions;
            };

            let Some(app) = self.application_registry.get(app_id) else {
                break 'add_focused_actions;
            };

            for action in app.application_registry.values().filter(|a| {
                a.focus_state == FocusState::Focused && a.when.matches(&context.active_interaction)
            }) {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_name: action.name.clone(),
                    focus_state: FocusState::Focused,
                    execution: action.execution.clone(),
                    shortcut_text: action.shortcut_text.clone(),
                    metadata: action.metadata.clone(),
                    target_window: Some(*active),
                });
            }
        }

        for app in self.application_registry.values() {
            for action in app.application_registry.values().filter(|a| {
                a.focus_state == FocusState::Global && a.when.matches(&context.active_interaction)
            }) {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_name: action.name.clone(),
                    focus_state: FocusState::Global,
                    execution: action.execution.clone(),
                    shortcut_text: action.shortcut_text.clone(),
                    metadata: action.metadata.clone(),
                    target_window: None,
                });
            }
        }

        all_actions
    }
}

#[derive(Debug, Clone)] // Debug is useful for printing
pub struct Application {
    application_name: AppName,
    application_process_name: AppProcessName,
    application_registry: HashMap<ActionId, Action>,
}

impl Application {
    pub fn new(app_config: &Config, current_os: &Os) -> Result<Application, String> {
        if app_config.version != 2 {
            return Err(format!(
                "Unsupported extension config version: {}",
                app_config.version
            ));
        }
        if app_config.platform != *current_os {
            return Err(format!(
                "Extension platform {:?} does not match current OS {:?}",
                app_config.platform, current_os
            ));
        }
        if app_config.app.id.trim().is_empty() {
            return Err("Extension app id must not be empty".to_string());
        }

        let mut application_registry: HashMap<ActionId, Action> = HashMap::new();
        let default_tags = app_config.app.default_tags.clone().unwrap_or_default();

        for (count, (_app_id, config_action)) in (0_u32..).zip(app_config.actions.iter()) {
            let when = action_condition_from_config(config_action.when.as_ref())?;

            let mut tags = default_tags.clone();
            if let Some(action_tags) = &config_action.tags {
                tags.extend(action_tags.iter().cloned());
            }
            tags.sort();
            tags.dedup();

            let (execution, shortcut_text) = action_execution_from_binding(&config_action.cmd)?;

            let app_action: Action = Action {
                name: config_action.name.clone(),
                shortcut_text,
                execution,
                focus_state: config_action.focus_state.unwrap_or(
                    app_config
                        .app
                        .default_focus_state
                        .unwrap_or(FocusState::Focused),
                ),
                when,
                metadata: ActionMetadata {
                    priority: config_action.priority.unwrap_or_default(),
                    favorite: config_action.favorite.unwrap_or(false),
                    tags,
                },
            };
            application_registry.insert(count, app_action);
        }

        Ok(Application {
            application_name: app_config.app.name.clone(),
            application_process_name: app_config.app.process_name.clone(),
            application_registry,
        })
    }

    pub fn from_plugin(plugin: &PluginApplication) -> Result<Application, String> {
        let mut application_registry = HashMap::new();

        for (idx, command) in plugin.commands.iter().enumerate() {
            let (execution, derived_shortcut_text) = match &command.cmd {
                Some(binding) => action_execution_from_binding(binding)?,
                None => (
                    ActionExecution::PluginCommand {
                        plugin_id: plugin.plugin_id.clone(),
                        command_id: command.id.clone(),
                    },
                    "WASM".to_string(),
                ),
            };
            let shortcut_text = command
                .shortcut_text
                .clone()
                .unwrap_or(derived_shortcut_text);

            application_registry.insert(
                idx as u32,
                Action {
                    name: command.name.clone(),
                    shortcut_text,
                    execution,
                    focus_state: command.focus_state,
                    when: ActionContextCondition::default(),
                    metadata: ActionMetadata {
                        priority: command.priority,
                        favorite: command.favorite,
                        tags: command.tags.clone(),
                    },
                },
            );
        }

        Ok(Application {
            application_name: plugin.name.clone(),
            application_process_name: plugin.process_name.clone(),
            application_registry,
        })
    }
}

fn action_condition_from_config(
    when: Option<&ActionWhenConfig>,
) -> Result<ActionContextCondition, String> {
    let Some(when) = when else {
        return Ok(ActionContextCondition::default());
    };
    if when.any.is_empty() {
        return Err("Action context condition 'when.any' must not be empty".to_string());
    }

    let mut any = Vec::with_capacity(when.any.len());
    for raw_tag in &when.any {
        let Some(tag) = normalize_context_tag(raw_tag) else {
            return Err(format!("Invalid action context tag: '{raw_tag}'"));
        };
        any.push(tag);
    }
    any.sort();
    any.dedup();

    Ok(ActionContextCondition { any })
}

const MAX_SHORTCUT_SEQUENCE_STEPS: usize = 5;

fn action_execution_from_binding(
    binding: &CommandBinding,
) -> Result<(ActionExecution, String), String> {
    match binding {
        CommandBinding::Shortcut(chord) => {
            let shortcut = shortcut_from_chord(chord);
            Ok((ActionExecution::Shortcut(shortcut), shortcut.to_string()))
        }
        CommandBinding::Sequence(sequence) => {
            let steps = sequence_steps_from_config(&sequence.sequence)?;
            let shortcut_text = sequence_shortcut_text(&steps);
            Ok((ActionExecution::ShortcutSequence(steps), shortcut_text))
        }
    }
}

fn shortcut_from_chord(chord: &KeyChord) -> KeyboardShortcut {
    KeyboardShortcut {
        modifier: modifiers_from_config(&chord.mods),
        key: chord.key,
    }
}

fn sequence_steps_from_config(
    sequence: &[KeySequenceStepConfig],
) -> Result<Vec<KeySequenceStep>, String> {
    if sequence.is_empty() {
        return Err("Shortcut sequence must contain at least one step".to_string());
    }
    if sequence.len() > MAX_SHORTCUT_SEQUENCE_STEPS {
        return Err(format!(
            "Shortcut sequence must contain at most {MAX_SHORTCUT_SEQUENCE_STEPS} steps"
        ));
    }

    sequence
        .iter()
        .map(sequence_step_from_config)
        .collect::<Result<Vec<_>, _>>()
}

fn sequence_step_from_config(step: &KeySequenceStepConfig) -> Result<KeySequenceStep, String> {
    let modifier = modifiers_from_config(&step.mods);
    if modifier.win {
        return Err("Shortcut sequences cannot use the Win modifier".to_string());
    }

    let key = match step.key {
        SequenceKeyConfig::Ctrl => SequenceKey::Ctrl,
        SequenceKeyConfig::Shift => SequenceKey::Shift,
        SequenceKeyConfig::Alt => SequenceKey::Alt,
        SequenceKeyConfig::Win => {
            return Err("Shortcut sequences cannot use the Win key".to_string());
        }
        SequenceKeyConfig::Key(key) => {
            if key == crate::domain::hotkey::Key::Enter {
                return Err("Shortcut sequences cannot use Enter".to_string());
            }
            SequenceKey::Key(key)
        }
    };

    Ok(KeySequenceStep { modifier, key })
}

fn modifiers_from_config(mods: &[Modifier]) -> HotkeyModifiers {
    HotkeyModifiers {
        control: mods.contains(&Modifier::Ctrl),
        shift: mods.contains(&Modifier::Shift),
        alt: mods.contains(&Modifier::Alt),
        win: mods.contains(&Modifier::Win),
    }
}

fn current_date_text() -> Result<String, String> {
    use windows::Win32::System::SystemInformation::GetLocalTime;

    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let system_time = unsafe { GetLocalTime() };
    let month_index = usize::from(system_time.wMonth.saturating_sub(1));
    let month = MONTHS
        .get(month_index)
        .ok_or_else(|| format!("Invalid local month value: {}", system_time.wMonth))?;

    Ok(format!("{} {}", system_time.wDay, month))
}

fn plugin_storage_root(plugin_id: &str) -> Result<PathBuf, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA is not available".to_string())?;
    Ok(PathBuf::from(local_app_data)
        .join("OmniPalette")
        .join("plugins")
        .join(plugin_id))
}

fn plugin_settings_text(plugin_id: &str) -> Result<String, String> {
    let install_root = crate::core::extensions::discovery::user_extensions_root()
        .ok_or_else(|| "APPDATA is not available".to_string())?;
    extension_settings_json(&install_root, plugin_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::extension::KeyChord,
        core::plugins::command::PluginCommand,
        domain::{
            action::{CommandPriority, InteractionContext},
            hotkey::Key,
        },
    };
    use std::fs;

    #[test]
    fn strict_build_rejects_invalid_static_extension() {
        let root = tempfile::tempdir().expect("temp root should be created");
        let static_dir = root.path().join("static");
        fs::create_dir_all(&static_dir).expect("static dir should be created");
        fs::write(static_dir.join("broken.toml"), "version = [")
            .expect("broken extension should be written");

        let discovery = ExtensionDiscovery::new(root.path());
        let err = MasterRegistry::build_strict(&discovery, Os::Windows)
            .expect_err("strict registry build should fail on invalid extension");

        assert!(err.to_string().contains("broken.toml"));
    }

    #[test]
    fn best_effort_build_skips_invalid_static_extension() {
        let root = tempfile::tempdir().expect("temp root should be created");
        let static_dir = root.path().join("static");
        fs::create_dir_all(&static_dir).expect("static dir should be created");
        fs::write(static_dir.join("broken.toml"), "version = [")
            .expect("broken extension should be written");

        let discovery = ExtensionDiscovery::new(root.path());
        let registry = MasterRegistry::build(&discovery, Os::Windows);

        assert!(registry.application_registry.is_empty());
    }

    fn registry_from_config(content: &str) -> MasterRegistry {
        let config: Config = toml::from_str(content).expect("config should parse");
        let app = Application::new(&config, &Os::Windows).expect("app should build");
        let mut registry = MasterRegistry::default();
        registry.application_registry.insert(0, app);
        registry
    }

    fn empty_context() -> ContextRoot {
        ContextRoot {
            fg_context: vec![],
            bg_context: vec![],
            active_interaction: InteractionContext::default(),
        }
    }

    #[test]
    fn context_condition_filters_global_actions() {
        let registry = registry_from_config(
            r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.bold]
name = "Bold text"
focus_state = "global"
cmd = { mods = ["ctrl"], key = "KeyB" }

[actions.bold.when]
any = ["ppt.selection.text", "ui.text_input"]
"#,
        );

        assert!(registry.get_actions(&empty_context()).is_empty());

        let context = ContextRoot {
            active_interaction: InteractionContext::from_tags(["ppt.selection.text".to_string()]),
            ..empty_context()
        };
        let actions = registry.get_actions(&context);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_name, "Bold text");
    }

    #[test]
    fn context_condition_rejects_empty_any_list() {
        let config: Config = toml::from_str(
            r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.bold]
name = "Bold text"
focus_state = "global"
cmd = { mods = ["ctrl"], key = "KeyB" }

[actions.bold.when]
any = []
"#,
        )
        .expect("config should parse");

        let err = Application::new(&config, &Os::Windows)
            .expect_err("empty context conditions should be rejected");

        assert!(err.contains("when.any"));
    }

    #[test]
    fn sequence_command_builds_sequence_execution_and_display_text() {
        let config: Config = toml::from_str(
            r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.select_draw_pen]
name = "Select drawing pen"
focus_state = "global"
cmd = { sequence = [
    { mods = ["alt"], key = "KeyJ" },
    { key = "KeyI" },
] }
"#,
        )
        .expect("config should parse");

        let app = Application::new(&config, &Os::Windows).expect("app should build");
        let action = app
            .application_registry
            .values()
            .next()
            .expect("action should exist");

        assert_eq!(action.shortcut_text, "Alt+J, I");
        match &action.execution {
            ActionExecution::ShortcutSequence(sequence) => assert_eq!(sequence.len(), 2),
            ActionExecution::Shortcut(_) | ActionExecution::PluginCommand { .. } => {
                panic!("action should be a shortcut sequence")
            }
        }
    }

    #[test]
    fn sequence_command_rejects_empty_sequence() {
        let err = app_build_error_for_command(r#"cmd = { sequence = [] }"#);

        assert!(err.contains("at least one step"));
    }

    #[test]
    fn sequence_command_rejects_more_than_five_steps() {
        let err = app_build_error_for_command(
            r#"cmd = { sequence = [
    { mods = ["alt"], key = "KeyJ" },
    { key = "KeyI" },
    { key = "KeyP" },
    { key = "KeyN" },
    { key = "KeyB" },
    { key = "KeyC" },
] }"#,
        );

        assert!(err.contains("at most 5 steps"));
    }

    #[test]
    fn sequence_command_rejects_win_modifier_and_key() {
        let modifier_err = app_build_error_for_command(
            r#"cmd = { sequence = [{ mods = ["win"], key = "KeyR" }] }"#,
        );
        let key_err = app_build_error_for_command(r#"cmd = { sequence = [{ key = "Win" }] }"#);

        assert!(modifier_err.contains("Win modifier"));
        assert!(key_err.contains("Win key"));
    }

    #[test]
    fn sequence_command_rejects_enter() {
        let err = app_build_error_for_command(r#"cmd = { sequence = [{ key = "Enter" }] }"#);

        assert!(err.contains("Enter"));
    }

    #[test]
    fn plugin_commands_with_direct_bindings_become_shortcuts() {
        let plugin = PluginApplication {
            plugin_id: "ahk_agent".to_string(),
            name: "AHK".to_string(),
            process_name: "ahk_agent".to_string(),
            commands: vec![PluginCommand {
                id: "script_hotkey".to_string(),
                name: "AHK: Demo : Ctrl+H".to_string(),
                priority: CommandPriority::Medium,
                focus_state: FocusState::Global,
                favorite: false,
                tags: vec!["ahk".to_string(), "demo".to_string()],
                shortcut_text: None,
                cmd: Some(CommandBinding::Shortcut(KeyChord {
                    mods: vec![Modifier::Ctrl],
                    key: Key::KeyH,
                })),
            }],
        };

        let app = Application::from_plugin(&plugin).expect("plugin app should build");
        let action = app
            .application_registry
            .values()
            .next()
            .expect("plugin action should exist");

        assert_eq!(action.shortcut_text, "Ctrl+H");
        match action.execution {
            ActionExecution::Shortcut(shortcut) => {
                assert!(shortcut.modifier.control);
                assert_eq!(shortcut.key, Key::KeyH);
            }
            ActionExecution::ShortcutSequence(_) | ActionExecution::PluginCommand { .. } => {
                panic!("direct plugin binding should build a shortcut action")
            }
        }
    }

    #[test]
    fn plugin_commands_without_direct_bindings_remain_plugin_commands() {
        let plugin = PluginApplication {
            plugin_id: "ahk_agent".to_string(),
            name: "AHK".to_string(),
            process_name: "ahk_agent".to_string(),
            commands: vec![PluginCommand {
                id: "script_hotstring".to_string(),
                name: "Demo : up; -> ⬆️".to_string(),
                priority: CommandPriority::Medium,
                focus_state: FocusState::Global,
                favorite: false,
                tags: vec!["ahk".to_string(), "demo".to_string()],
                shortcut_text: Some(String::new()),
                cmd: None,
            }],
        };

        let app = Application::from_plugin(&plugin).expect("plugin app should build");
        let action = app
            .application_registry
            .values()
            .next()
            .expect("plugin action should exist");

        assert_eq!(action.shortcut_text, "");
        match &action.execution {
            ActionExecution::PluginCommand {
                plugin_id,
                command_id,
            } => {
                assert_eq!(plugin_id, "ahk_agent");
                assert_eq!(command_id, "script_hotstring");
            }
            ActionExecution::Shortcut(_) | ActionExecution::ShortcutSequence(_) => {
                panic!("plugin command without a direct binding should remain a plugin command")
            }
        }
    }

    fn app_build_error_for_command(command_line: &str) -> String {
        let content = format!(
            r#"
version = 2
platform = "windows"

[app]
id = "powerpoint"
name = "PowerPoint"
process_name = "POWERPNT.EXE"

[actions]

[actions.test]
name = "Test"
{command_line}
"#
        );
        let config: Config = toml::from_str(&content).expect("config should parse");
        Application::new(&config, &Os::Windows).expect_err("app build should fail")
    }
}
