use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub version: u32,
    pub app: App,
    pub actions: HashMap<String, Action>,
}

#[derive(Debug, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub default_priority: Priority,
    pub os: AppOs,
}

#[derive(Debug, Deserialize)]
pub struct AppOs {
    pub windows: Option<String>,
    pub macos: Option<String>,
    pub linux: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Action {
    pub name: String,
    pub focus_state: FocusState,
    pub cmd: CmdByOs,
}

#[derive(Debug, Deserialize)]
pub struct CmdByOs {
    pub windows: Option<KeyChord>,
    pub macos: Option<KeyChord>,
    pub linux: Option<KeyChord>,
}

#[derive(Debug, Deserialize)]
pub struct KeyChord {
    pub mods: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusState {
    Focused,
    Global,
    Background,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Cmd,
    Win,
    // Meta,
}

#[derive(Debug, Deserialize)]
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
