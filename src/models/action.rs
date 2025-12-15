use std::rc::Rc;

use crate::models::hotkey::KeyBoardShortcut;

#[derive(Debug, Clone, Hash)]
pub struct Action {
    pub name: Box<str>,
    pub cmd: KeyBoardShortcut,
    pub focus_state: FocusState,
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum FocusState {
    Focused,
    Backgroud,
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
pub type AppName = Rc<str>; // Represent name of the app this action belongs to
pub type ApplicationProcessName = Rc<str>;
pub type ActionId = u32; // uniquely identifies what the user is trying to do, ie paste, copy, new tab
                         // Not sure if u32 or string or some special struct is better
                         // Open to change
pub type ActionName = Rc<str>;

// All Context Root should have a mapping to all available actions that can be taken
#[derive(Debug, Clone, Hash)]

pub struct ContextRoot {
    pub context_stack: Vec<Context>, // Top of stack is current context, ie chrome
    pub background_context: Vec<Context>, // Order does not matter. Use to hold other context
}

impl ContextRoot {
    pub fn get_current_context(&self) -> Option<&Context> {
        return self.context_stack.last();
    }
}

#[derive(Debug, Clone, Hash)]
pub struct Context {
    application: Rc<str>,
}
