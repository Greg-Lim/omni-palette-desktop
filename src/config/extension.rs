use serde::Deserialize;
use std::collections::HashMap;

use crate::domain::{
    action::{CommandPriority, FocusState, Os},
    hotkey::Key,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub platform: Os,
    pub app: AppConfig,
    pub actions: HashMap<String, ActionConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub default_focus_state: Option<FocusState>,
    pub default_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    pub name: String,
    pub focus_state: Option<FocusState>,
    #[serde(alias = "action_priority")]
    pub priority: Option<CommandPriority>,
    pub tags: Option<Vec<String>>,
    pub starred: Option<bool>,
    pub cmd: KeyChord,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KeyChord {
    pub mods: Vec<Modifier>,
    pub key: Key,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Cmd,
    Win,
    Fn,
    // Meta,
}
