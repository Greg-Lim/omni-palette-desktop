use crate::domain::hotkey::KeyboardShortcut;
use raw_window_handle::RawWindowHandle;
use serde::Deserialize;

#[derive(Debug, Clone, Hash)]
pub struct Action {
    pub name: String,
    pub keyboard_shortcut: KeyboardShortcut,
    pub focus_state: FocusState,
    pub metadata: ActionMetadata,
}

#[derive(Debug, Clone, Hash)]
pub struct ActionMetadata {
    pub priority: CommandPriority,
    pub starred: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CommandPriority {
    #[serde(alias = "Suppressed")]
    Suppressed,
    #[default]
    #[serde(alias = "Normal")]
    Normal,
    #[serde(alias = "High")]
    High,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum FocusState {
    Focused,
    Background,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

pub type ApplicationID = u32;
pub type AppName = String;
pub type AppProcessName = String;
pub type ActionId = u32;
pub type ActionName = String;

#[derive(Debug, Clone, Hash)]
pub struct ContextRoot {
    pub fg_context: Vec<Context>,
    pub bg_context: Vec<Context>,
}

impl ContextRoot {
    pub fn get_active(&self) -> Option<&Context> {
        self.fg_context.first()
    }
}

type Context = RawWindowHandle;

#[allow(dead_code)]
pub trait ContextExt {
    fn get_all_names(&self) -> Vec<String>;
}
