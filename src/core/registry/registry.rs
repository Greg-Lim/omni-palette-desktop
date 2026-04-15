// register action is for the user to register new actions given the context and action

use std::{collections::HashMap, fs, path::Path};

use log::{error, info, warn};

use raw_window_handle::RawWindowHandle;

use crate::{
    core::extensions::extensions::load_config,
    models::{
        action::{
            Action, ActionId, ActionMetadata, ActionName, AppName, AppProcessName, ApplicationID,
            CommandPriority, ContextRoot, FocusState, Os,
        },
        config::{CmdByOs, Config, KeyChord, Modifier},
        hotkey::{HotkeyModifiers, KeyboardShortcut},
    },
    platform::platform_interface::RawWindowHandleExt,
};

#[derive(Default, Debug)]
pub struct MasterRegistry {
    // represents the global registry to determine all possible commands
    // 2 way: can be lazy generated when the user pulls up the palette or pregenerated.
    pub application_registry: HashMap<ApplicationID, Application>,
    pub application_process_name_id: HashMap<AppProcessName, ApplicationID>,
}

impl MasterRegistry {
    pub fn build(extensions_folder: &Path, current_os: Os) -> MasterRegistry {
        let mut master_registry = MasterRegistry::default();

        // Use a match on read_dir to handle the folder being missing/inaccessible
        match fs::read_dir(extensions_folder) {
            Ok(entries) => {
                for (idx, entry) in entries.enumerate() {
                    // Handle individual directory entry errors
                    let entry = match entry {
                        Ok(e) => e,
                        Err(err) => {
                            warn!("Failed to read directory entry {}: {}", idx, err);
                            continue;
                        }
                    };

                    let path = entry.path();

                    // Skip non-toml files
                    if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                        continue;
                    }

                    // Load and build application
                    match load_config(&path).and_then(|c| Application::new(&c, &current_os)) {
                        Ok(app) => {
                            info!(
                                "Successfully loaded extension: {:?}",
                                path.file_name().unwrap()
                            );
                            master_registry
                                .application_registry
                                .insert(idx as u32, app.clone());
                            master_registry
                                .application_process_name_id
                                .insert(app.application_process_name.clone(), idx as u32);
                        }
                        Err(err) => {
                            error!("Failed to load extension at {:?}: {}", path, err);
                        }
                    }
                }
            }
            Err(e) => error!(
                "Could not access extensions directory at {:?}: {}",
                extensions_folder, e
            ),
        };

        master_registry
    }
}

#[derive(Debug)]
pub struct UnitAction {
    // This struct will be use for search and generating the UI
    pub app_name: AppName,
    pub action_id: ActionId,
    pub action_name: ActionName,
    pub focus_state: FocusState,
    pub keyboard_shortcut: KeyboardShortcut,
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

            for (&action_id, action) in app
                .application_registry
                .iter()
                .filter(|(_, a)| a.focus_state == FocusState::Background)
            {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_id,
                    action_name: action.name.clone(),
                    focus_state: FocusState::Background,
                    keyboard_shortcut: action.keyboard_shortcut,
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

            for (&action_id, action) in app
                .application_registry
                .iter()
                .filter(|(_, a)| a.focus_state == FocusState::Focused)
            {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_id,
                    action_name: action.name.clone(),
                    focus_state: FocusState::Focused,
                    keyboard_shortcut: action.keyboard_shortcut,
                    metadata: action.metadata.clone(),
                    target_window: Some(*active),
                });
            }
        }

        for app in self.application_registry.values() {
            for (&action_id, action) in app
                .application_registry
                .iter()
                .filter(|(_, a)| a.focus_state == FocusState::Global)
            {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_id,
                    action_name: action.name.clone(),
                    focus_state: FocusState::Global,
                    keyboard_shortcut: action.keyboard_shortcut,
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
        let application_os_name = match current_os {
            Os::Windows => app_config
                .app
                .application_os_name
                .windows
                .clone()
                .ok_or("No OS app name"),
            Os::Mac => app_config
                .app
                .application_os_name
                .macos
                .clone()
                .ok_or("No OS app name"),
            Os::Linux => app_config
                .app
                .application_os_name
                .linux
                .clone()
                .ok_or("No OS app name"),
        }?;

        let mut application_registry: HashMap<ActionId, Action> = HashMap::new();
        let default_priority = app_config
            .app
            .default_priority
            .unwrap_or(CommandPriority::Normal);
        let default_tags = app_config.app.default_tags.clone().unwrap_or_default();

        fn extract_os_binding<'a>(
            action: &'a CmdByOs,
            _current_os: &Os,
        ) -> Result<&'a KeyChord, String> {
            match _current_os {
                Os::Windows => action
                    .windows
                    .as_ref()
                    .ok_or_else(|| "No Windows action".to_string()),
                Os::Mac => action
                    .macos
                    .as_ref()
                    .ok_or_else(|| "No Mac action".to_string()),
                Os::Linux => action
                    .linux
                    .as_ref()
                    .ok_or_else(|| "No Linux action".to_string()),
            }
        }

        let mut count: u32 = 0;

        for (_app_id, config_action) in app_config.actions.iter() {
            let binding = extract_os_binding(&config_action.cmd, current_os);
            let binding = match binding {
                Err(s) => {
                    warn!("{s}");
                    continue;
                }
                Ok(binding) => binding,
            };

            let mut tags = default_tags.clone();
            if let Some(action_tags) = &config_action.tags {
                tags.extend(action_tags.iter().cloned());
            }
            tags.sort();
            tags.dedup();

            let app_action: Action = Action {
                name: config_action.name.clone(),
                keyboard_shortcut: KeyboardShortcut {
                    modifier: HotkeyModifiers {
                        control: binding.mods.contains(&Modifier::Ctrl),
                        shift: binding.mods.contains(&Modifier::Shift),
                        alt: binding.mods.contains(&Modifier::Alt),
                        win: binding.mods.contains(&Modifier::Win),
                    },
                    key: binding.key,
                },
                focus_state: config_action.focus_state.unwrap_or(
                    app_config
                        .app
                        .default_focus_state
                        .unwrap_or(FocusState::Focused),
                ),
                metadata: ActionMetadata {
                    priority: config_action.priority.unwrap_or(default_priority),
                    starred: config_action.starred.unwrap_or(false),
                    tags,
                },
            };
            application_registry.insert(count, app_action);
            count += 1;
        }

        Ok(Application {
            application_name: app_config.app.name.clone(),
            application_process_name: application_os_name,
            application_registry,
        })
    }

    pub fn get_action(&self, action_id: &ActionId) -> Option<&Action> {
        self.application_registry.get(action_id)
    }
}
