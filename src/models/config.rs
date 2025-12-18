use serde::Deserialize;
use std::collections::HashMap;

use crate::models::{action::FocusState, hotkey::Key};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: u32,
    pub app: App,
    pub actions: HashMap<String, Action>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct App {
    pub id: String,
    pub name: String,
    pub default_focus_state: Option<FocusState>,
    pub default_priority: Priority,
    pub application_os_name: AppOsName,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppOsName {
    pub windows: Option<String>,
    pub macos: Option<String>,
    pub linux: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Action {
    pub name: String,
    pub focus_state: Option<FocusState>,
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

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum Priority {
    #[serde(rename = "OSReserved")]
    OSReserved,
    #[serde(rename = "GlobalRemapper")]
    GlobalRemapper,
    #[serde(rename = "OSGlobal")]
    OSGlobal,
    #[serde(rename = "UserOverrides")]
    UserOverrides,
    #[serde(rename = "Application")]
    Application,
    #[serde(rename = "ApplicationExtensions")]
    ApplicationExtensions,
    #[serde(rename = "DocumentOrWebApp")]
    DocumentOrWebApp,
}
