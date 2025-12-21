// register action is for the user to register new actions given the context and action

use std::{
    collections::{HashMap, HashSet},
    fmt::Error,
    fs,
    ops::Add,
    path::Path,
    rc::Rc,
    task::Context,
};

use log::{error, info, warn};
use windows::Win32::Foundation::HWND;

use crate::{
    core::extensions::extensions::load_config,
    models::{
        action::{
            self, Action, ActionId, ActionName, AppName, AppProcessName, ApplicationID,
            ContextRoot, FocusState, Os, Priority,
        },
        config::{CmdByOs, Config, KeyChord, Modifier},
        hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
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
    app_name: AppName,
    action_id: ActionId,
    action_name: ActionName,
    focus_state: FocusState,
    keyboard_shortcut: KeyboardShortcut,
}

impl MasterRegistry {
    pub fn get_actions(&self, context: &ContextRoot) -> Vec<UnitAction> {
        // for now we will either call this every time the context change or the user opens the page
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
                    action_id: action_id,
                    action_name: action.name.clone(),
                    focus_state: FocusState::Background,
                    keyboard_shortcut: action.keyboard_shortcut,
                });
            }
        }

        // Extract Focused Actions
        'add_focused_actions: {
            dbg!("app_id");

            let Some(active) = context.get_active() else {
                break 'add_focused_actions;
            };
            dbg!(&active);

            let Some(process_name) = active.get_app_process_name() else {
                break 'add_focused_actions;
            };
            dbg!(&process_name);
            dbg!(&self.application_process_name_id);

            let Some(app_id) = self.application_process_name_id.get(&process_name) else {
                break 'add_focused_actions;
            };

            dbg!(&app_id);

            let Some(app) = self.application_registry.get(&app_id) else {
                break 'add_focused_actions;
            };

            for (&action_id, action) in app
                .application_registry
                .iter()
                .filter(|(_, a)| a.focus_state == FocusState::Focused)
            {
                all_actions.push(UnitAction {
                    app_name: app.application_name.clone(),
                    action_id: action_id,
                    action_name: action.name.clone(),
                    focus_state: FocusState::Background,
                    keyboard_shortcut: action.keyboard_shortcut,
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

        for (appId, config_action) in app_config.actions.iter() {
            let binding = extract_os_binding(&config_action.cmd, &current_os);
            let binding = match binding {
                Err(s) => {
                    warn!("{s}");
                    continue;
                }
                Ok(binding) => binding,
            };

            // let binding_ref = binding.as_ref();
            let app_action: Action = Action {
                name: config_action.name.clone(),
                keyboard_shortcut: KeyboardShortcut {
                    modifier: HotkeyModifiers {
                        control: binding.mods.contains(&Modifier::Ctrl),
                        shift: binding.mods.contains(&Modifier::Shift),
                        alt: binding.mods.contains(&Modifier::Alt),
                        win: binding.mods.contains(&Modifier::Win),
                    },
                    key: binding.key.into(),
                },
                focus_state: config_action.focus_state.clone().unwrap_or(
                    app_config
                        .app
                        .default_focus_state
                        .clone()
                        .ok_or("No Focus state found".to_string())?,
                ),
            }
            .into();
            application_registry.insert(count, app_action);
            count += 1;
        }

        Ok(Application {
            application_name: app_config.app.name.clone().into(),
            application_process_name: application_os_name.into(),
            application_registry: application_registry.into(),
        })
    }

    pub fn get_action(&self, action_id: &ActionId) -> Option<&Action> {
        self.application_registry.get(action_id)
    }
}
