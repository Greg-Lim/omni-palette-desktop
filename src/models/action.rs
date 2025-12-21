use std::rc::Rc;

use crate::models::hotkey::KeyboardShortcut;
use raw_window_handle::RawWindowHandle;
use serde::Deserialize;

use linked_hash_set::LinkedHashSet;

#[derive(Debug, Clone, Hash)]
pub struct Action {
    pub name: String,
    pub keyboard_shortcut: KeyboardShortcut,
    pub focus_state: FocusState,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum FocusState {
    Focused,
    Background,
    Global,
}

pub enum Os {
    Windows,
    Mac,
    Linux,
}

#[derive(PartialEq, Eq, Debug)] // Debug is useful for printing
#[repr(u8)]
pub enum Priority {
    // Un-interceptable / always wins
    OSReserved = 100,

    // User tools that hook/remap keys globally (AHK, PowerToys, Karabiner, OEM tools)
    GlobalRemapper = 90,

    // OS-level (shell / registered global hotkeys)
    OSGlobal = 80,

    // Your own user-defined overrides (should usually beat app defaults)
    UserOverrides = 70,

    // Focused app’s own shortcuts (Chrome UI, Word app commands)
    Application = 60,

    // App plugins / extensions (Chrome extension commands, Office add-ins)
    ApplicationExtensions = 50,

    // Webpage / webapp / document/editor handlers (Google Docs, in-page shortcuts)
    DocumentOrWebApp = 40,
    // “Interceptors” as a concept usually isn’t a priority layer;
    // it’s an implementation detail (they *apply* a layer’s decision).
}

pub type ApplicationID = u32; // use to uniquely identify apps that may have same names
pub type AppName = String; // Represent name of the app this action belongs to
pub type AppProcessName = String;
pub type ActionId = u32; // uniquely identifies what the user is trying to do, ie paste, copy, new tab
                         // Not sure if u32 or string or some special struct is better
                         // Open to change
pub type ActionName = String;

// All Context Root should have a mapping to all available actions that can be taken
#[derive(Debug, Clone, Hash)]

pub struct ContextRoot {
    pub fg_context: Vec<Context>,
    pub bg_context: Vec<Context>, // Order does not matter. Use to hold other context
}

impl ContextRoot {
    pub fn get_active(&self) -> Option<&Context> {
        return self.fg_context.first();
    }
}

type Context = RawWindowHandle;

pub trait ContextExt {
    fn get_all_names(&self) -> Vec<String>;
}

// impl ContextExt for ContextExt {}
