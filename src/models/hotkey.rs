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

#[derive(Debug, Clone, Copy, Hash)]

pub enum Key {
    // --- 1. Alphanumeric Keys (As Is) ---
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
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
    Semicolon,    // ; or :
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

pub struct KeyBoardShortcut {
    modifier: HotkeyModifiers,
    key: Key,
}
