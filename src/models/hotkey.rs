use std::fmt;

use serde::Deserialize;
use strum_macros::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Control, // Ctrl (Windows/Linux) or Control (macOS)
    Shift,   // Shift
    Alt,     // Alt (Windows/Linux) or Option (macOS)
    Win,     // Windows Key (Windows) or Command (macOS/Cmd)
}

// Optional: Implement a helper struct or method to hold a combination of modifiers
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]

pub struct HotkeyModifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
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

impl fmt::Display for KeyboardShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifier.control {
            parts.push("Ctrl");
        }
        if self.modifier.shift {
            parts.push("Shift");
        }
        if self.modifier.alt {
            parts.push("Alt");
        }
        if self.modifier.win {
            parts.push("Win");
        }
        parts.push(self.key.display_name());
        write!(f, "{}", parts.join("+"))
    }
}

impl Key {
    pub fn display_name(&self) -> &'static str {
        match self {
            Key::KeyA => "A",
            Key::KeyB => "B",
            Key::KeyC => "C",
            Key::KeyD => "D",
            Key::KeyE => "E",
            Key::KeyF => "F",
            Key::KeyG => "G",
            Key::KeyH => "H",
            Key::KeyI => "I",
            Key::KeyJ => "J",
            Key::KeyK => "K",
            Key::KeyL => "L",
            Key::KeyM => "M",
            Key::KeyN => "N",
            Key::KeyO => "O",
            Key::KeyP => "P",
            Key::KeyQ => "Q",
            Key::KeyR => "R",
            Key::KeyS => "S",
            Key::KeyT => "T",
            Key::KeyU => "U",
            Key::KeyV => "V",
            Key::KeyW => "W",
            Key::KeyX => "X",
            Key::KeyY => "Y",
            Key::KeyZ => "Z",
            Key::Key0 => "0",
            Key::Key1 => "1",
            Key::Key2 => "2",
            Key::Key3 => "3",
            Key::Key4 => "4",
            Key::Key5 => "5",
            Key::Key6 => "6",
            Key::Key7 => "7",
            Key::Key8 => "8",
            Key::Key9 => "9",
            Key::F1 => "F1",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::Semicolon => ";",
            Key::Equal => "=",
            Key::Comma => ",",
            Key::Minus => "-",
            Key::Period => ".",
            Key::Slash => "/",
            Key::Grave => "`",
            Key::LeftBracket => "[",
            Key::Backslash => "\\",
            Key::RightBracket => "]",
            Key::Apostrophe => "'",
            Key::Enter => "Enter",
            Key::Space => "Space",
            Key::Tab => "Tab",
            Key::Escape => "Esc",
            Key::Delete => "Del",
            Key::BackSpace => "Backspace",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PgUp",
            Key::PageDown => "PgDn",
            Key::Insert => "Ins",
            Key::PrintScreen => "PrtSc",
            Key::ScrollLock => "ScrLk",
            Key::Pause => "Pause",
            Key::LeftArrow => "Left",
            Key::RightArrow => "Right",
            Key::UpArrow => "Up",
            Key::DownArrow => "Down",
        }
    }
}
