// register action is for the user to register new actions given the context and action

use std::{
    collections::{HashMap, HashSet},
    fmt::Error,
    rc::Rc,
};

use crate::models::{
    action::{
        Action, ActionId, ActionName, AppName, ApplicationID, ApplicationProcessName, ContextRoot,
        FocusState, Priority,
    },
    hotkey::KeyBoardShortcut,
};

pub struct GlobalRegistry {
    // represents the global registry to determine all possible commands
    // 2 way: can be lazy generated when the user pulls up the palette or pregenerated.
    application_registry: HashMap<ApplicationID, Application>,
}

impl GlobalRegistry {
    pub fn get_all_commands(self, context: &ContextRoot) -> Box<[ActionId]> {
        todo!("given a context build the action ids")
    }
}

pub struct Application {
    application_name: AppName,
    application_process_name: ApplicationProcessName,
    application_registry: HashMap<ActionId, Rc<Action>>,
}

impl Application {
    pub fn get_action(&self, action_id: &ActionId) -> Option<&Rc<Action>> {
        self.application_registry.get(action_id)
    }
}

//This function is to register
fn register_activation(hotkeys: KeyBoardShortcut) -> Result<(), Error> {
    todo!("Add registering code to add the activation");
}
