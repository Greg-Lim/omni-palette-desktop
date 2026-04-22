// register action is for the user to register new actions given the context and action

use std::{collections::HashMap, sync::Arc};

use log::{error, info};

use raw_window_handle::RawWindowHandle;

use crate::{
    config::extension::{Config, Modifier},
    core::{
        extensions::{discovery::ExtensionDiscovery, extensions::load_config},
        plugins::{PluginApplication, PluginRegistry},
    },
    domain::{
        action::{
            Action, ActionExecution, ActionId, ActionMetadata, ActionName, AppName, AppProcessName,
            ApplicationID, ContextRoot, FocusState, Os,
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
            let app = Application::from_plugin(plugin_app);
            master_registry
                .application_process_name_id
                .insert(app.application_process_name.clone(), app_id);
            master_registry.application_registry.insert(app_id, app);
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
                .filter(|a| a.focus_state == FocusState::Background)
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

            for action in app
                .application_registry
                .values()
                .filter(|a| a.focus_state == FocusState::Focused)
            {
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
            for action in app
                .application_registry
                .values()
                .filter(|a| a.focus_state == FocusState::Global)
            {
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
            let binding = &config_action.cmd;

            let mut tags = default_tags.clone();
            if let Some(action_tags) = &config_action.tags {
                tags.extend(action_tags.iter().cloned());
            }
            tags.sort();
            tags.dedup();

            let shortcut = KeyboardShortcut {
                modifier: HotkeyModifiers {
                    control: binding.mods.contains(&Modifier::Ctrl),
                    shift: binding.mods.contains(&Modifier::Shift),
                    alt: binding.mods.contains(&Modifier::Alt),
                    win: binding.mods.contains(&Modifier::Win),
                },
                key: binding.key,
            };

            let app_action: Action = Action {
                name: config_action.name.clone(),
                shortcut_text: shortcut.to_string(),
                execution: ActionExecution::Shortcut(shortcut),
                focus_state: config_action.focus_state.unwrap_or(
                    app_config
                        .app
                        .default_focus_state
                        .unwrap_or(FocusState::Focused),
                ),
                metadata: ActionMetadata {
                    priority: config_action.priority.unwrap_or_default(),
                    starred: config_action.starred.unwrap_or(false),
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

    pub fn from_plugin(plugin: &PluginApplication) -> Application {
        let application_registry = plugin
            .commands
            .iter()
            .enumerate()
            .map(|(idx, command)| {
                (
                    idx as u32,
                    Action {
                        name: command.name.clone(),
                        shortcut_text: command.shortcut_text.clone(),
                        execution: ActionExecution::PluginCommand {
                            plugin_id: plugin.plugin_id.clone(),
                            command_id: command.id.clone(),
                        },
                        focus_state: command.focus_state,
                        metadata: ActionMetadata {
                            priority: command.priority,
                            starred: command.starred,
                            tags: command.tags.clone(),
                        },
                    },
                )
            })
            .collect();

        Application {
            application_name: plugin.name.clone(),
            application_process_name: plugin.process_name.clone(),
            application_registry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
