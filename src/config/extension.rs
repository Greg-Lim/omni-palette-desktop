use serde::Deserialize;
use std::collections::HashMap;

use crate::domain::{
    action::{CommandPriority, FocusState},
    hotkey::Key,
};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[allow(dead_code)]
    pub version: u32,
    pub app: AppConfig,
    pub actions: HashMap<String, ActionConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub default_focus_state: Option<FocusState>,
    pub default_tags: Option<Vec<String>>,
    #[serde(alias = "app_os_name")]
    pub application_os_name: AppOsName,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppOsName {
    pub windows: Option<String>,
    pub macos: Option<String>,
    pub linux: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ActionConfig {
    pub name: String,
    pub focus_state: Option<FocusState>,
    pub priority: Option<CommandPriority>,
    pub tags: Option<Vec<String>>,
    pub starred: Option<bool>,
    pub cmd: CmdByOs,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CmdByOs {
    pub windows: Option<KeyChord>,
    pub macos: Option<KeyChord>,
    pub linux: Option<KeyChord>,
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
