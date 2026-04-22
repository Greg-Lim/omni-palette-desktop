use crate::domain::hotkey::KeyboardShortcut;
use raw_window_handle::RawWindowHandle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash)]
pub struct Action {
    pub name: String,
    pub execution: ActionExecution,
    pub shortcut_text: String,
    pub focus_state: FocusState,
    pub metadata: ActionMetadata,
}

#[derive(Debug, Clone, Hash)]
pub enum ActionExecution {
    Shortcut(KeyboardShortcut),
    PluginCommand {
        plugin_id: String,
        command_id: String,
    },
}

#[derive(Debug, Clone, Hash)]
pub struct ActionMetadata {
    pub priority: CommandPriority,
    pub favorite: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CommandPriority {
    #[serde(alias = "Suppressed")]
    Suppressed,
    #[serde(alias = "Low")]
    Low,
    #[default]
    #[serde(alias = "normal", alias = "Normal", alias = "Medium")]
    Medium,
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

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    #[serde(rename = "macos")]
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
