use serde::Deserialize;
use strum_macros::{Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Control, // Ctrl (Windows/Linux) or Control (macOS)
    Shift,   // Shift
    Alt,     // Alt (Windows/Linux) or Option (macOS)
    Win,     // Windows Key (Windows) or Command (macOS/Cmd)
}

// Optional: Implement a helper struct or method to hold a combination of modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

pub struct HotkeyModifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl Default for HotkeyModifiers {
    fn default() -> Self {
        HotkeyModifiers {
            control: false,
            shift: false,
            alt: false,
            win: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, Deserialize, Display)]
#[strum(serialize_all = "snake_case")] // Converts KeyA to "key_a" automatically
pub enum Key {
    // --- 1. Alphanumeric Keys ---
    #[serde(alias = "a", alias = "A", alias = "KeyA")]
    KeyA,
    #[serde(alias = "b", alias = "B", alias = "KeyB")]
    KeyB,
    #[serde(alias = "c", alias = "C", alias = "KeyC")]
    KeyC,
    #[serde(alias = "d", alias = "D", alias = "KeyD")]
    KeyD,
    #[serde(alias = "e", alias = "E", alias = "KeyE")]
    KeyE,
    #[serde(alias = "f", alias = "F", alias = "KeyF")]
    KeyF,
    #[serde(alias = "g", alias = "G", alias = "KeyG")]
    KeyG,
    #[serde(alias = "h", alias = "H", alias = "KeyH")]
    KeyH,
    #[serde(alias = "i", alias = "I", alias = "KeyI")]
    KeyI,
    #[serde(alias = "j", alias = "J", alias = "KeyJ")]
    KeyJ,
    #[serde(alias = "k", alias = "K", alias = "KeyK")]
    KeyK,
    #[serde(alias = "l", alias = "L", alias = "KeyL")]
    KeyL,
    #[serde(alias = "m", alias = "M", alias = "KeyM")]
    KeyM,
    #[serde(alias = "n", alias = "N", alias = "KeyN")]
    KeyN,
    #[serde(alias = "o", alias = "O", alias = "KeyO")]
    KeyO,
    #[serde(alias = "p", alias = "P", alias = "KeyP")]
    KeyP,
    #[serde(alias = "q", alias = "Q", alias = "KeyQ")]
    KeyQ,
    #[serde(alias = "r", alias = "R", alias = "KeyR")]
    KeyR,
    #[serde(alias = "s", alias = "S", alias = "KeyS")]
    KeyS,
    #[serde(alias = "t", alias = "T", alias = "KeyT")]
    KeyT,
    #[serde(alias = "u", alias = "U", alias = "KeyU")]
    KeyU,
    #[serde(alias = "v", alias = "V", alias = "KeyV")]
    KeyV,
    #[serde(alias = "w", alias = "W", alias = "KeyW")]
    KeyW,
    #[serde(alias = "x", alias = "X", alias = "KeyX")]
    KeyX,
    #[serde(alias = "y", alias = "Y", alias = "KeyY")]
    KeyY,
    #[serde(alias = "z", alias = "Z", alias = "KeyZ")]
    KeyZ,

    #[serde(alias = "0", alias = "Key0", alias = "Digit0")]
    Key0,
    #[serde(alias = "1", alias = "Key1", alias = "Digit1")]
    Key1,
    #[serde(alias = "2", alias = "Key2", alias = "Digit2")]
    Key2,
    #[serde(alias = "3", alias = "Key3", alias = "Digit3")]
    Key3,
    #[serde(alias = "4", alias = "Key4", alias = "Digit4")]
    Key4,
    #[serde(alias = "5", alias = "Key5", alias = "Digit5")]
    Key5,
    #[serde(alias = "6", alias = "Key6", alias = "Digit6")]
    Key6,
    #[serde(alias = "7", alias = "Key7", alias = "Digit7")]
    Key7,
    #[serde(alias = "8", alias = "Key8", alias = "Digit8")]
    Key8,
    #[serde(alias = "9", alias = "Key9", alias = "Digit9")]
    Key9,

    // --- 2. Function Keys (As Is) ---
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // --- 3. Punctuation & Symbol Keys (New) ---
    // These generally map to the un-shifted key on the keyboard:
    #[strum(serialize = ";", serialize = ":", serialize = "semicolon")]
    Semicolon, // ; or :
    Equal,        // = or +
    Comma,        // , or <
    Minus,        // - or _
    Period,       // . or >
    Slash,        // / or ?
    Grave,        // ` or ~ (tilde)
    LeftBracket,  // [ or {
    Backslash,    // \ or |
    RightBracket, // ] or }
    Apostrophe,   // ' or "

    // --- 4. Special Keys (As Is) ---
    Enter,
    Space,
    Tab,
    Escape,
    Delete,
    BackSpace, // Backspace

    // --- 5. Navigation & Movement Keys (As Is) ---
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    PrintScreen,
    ScrollLock,
    Pause,

    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
}

#[derive(Debug, Clone, Copy, Hash)]

pub struct KeyboardShortcut {
    pub modifier: HotkeyModifiers,
    pub key: Key,
}
