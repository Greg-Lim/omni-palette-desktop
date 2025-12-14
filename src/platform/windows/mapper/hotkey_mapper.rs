use crate::models::hotkey::{self, HotkeyModifiers, Key, Modifier};
use crate::platform::windows::models::HotkeyEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub fn map_modifier(modifiers: &HotkeyModifiers) -> HOT_KEY_MODIFIERS {
    let mut flags: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS::default();

    if modifiers.control {
        flags |= MOD_CONTROL;
    }

    if modifiers.shift {
        flags |= MOD_SHIFT;
    }

    if modifiers.alt {
        flags |= MOD_ALT;
    }

    if modifiers.win {
        flags |= MOD_WIN;
    }

    flags
}

pub fn map_key(key: Key) -> VIRTUAL_KEY {
    match key {
        // --- 1. Alphanumeric Keys ---
        Key::KeyA => VK_A,
        Key::KeyB => VK_B,
        Key::KeyC => VK_C,
        Key::KeyD => VK_D,
        Key::KeyE => VK_E,
        Key::KeyF => VK_F,
        Key::KeyG => VK_G,
        Key::KeyH => VK_H,
        Key::KeyI => VK_I,
        Key::KeyJ => VK_J,
        Key::KeyK => VK_K,
        Key::KeyL => VK_L,
        Key::KeyM => VK_M,
        Key::KeyN => VK_N,
        Key::KeyO => VK_O,
        Key::KeyP => VK_P,
        Key::KeyQ => VK_Q,
        Key::KeyR => VK_R,
        Key::KeyS => VK_S,
        Key::KeyT => VK_T,
        Key::KeyU => VK_U,
        Key::KeyV => VK_V,
        Key::KeyW => VK_W,
        Key::KeyX => VK_X,
        Key::KeyY => VK_Y,
        Key::KeyZ => VK_Z,

        Key::Key0 => VK_0,
        Key::Key1 => VK_1,
        Key::Key2 => VK_2,
        Key::Key3 => VK_3,
        Key::Key4 => VK_4,
        Key::Key5 => VK_5,
        Key::Key6 => VK_6,
        Key::Key7 => VK_7,
        Key::Key8 => VK_8,
        Key::Key9 => VK_9,

        // --- 2. Function Keys ---
        Key::F1 => VK_F1,
        Key::F2 => VK_F2,
        Key::F3 => VK_F3,
        Key::F4 => VK_F4,
        Key::F5 => VK_F5,
        Key::F6 => VK_F6,
        Key::F7 => VK_F7,
        Key::F8 => VK_F8,
        Key::F9 => VK_F9,
        Key::F10 => VK_F10,
        Key::F11 => VK_F11,
        Key::F12 => VK_F12,

        // --- 3. Punctuation & Symbol Keys ---
        // Note: VK_OEM_* are “layout-dependent” keys (physical key positions). :contentReference[oaicite:1]{index=1}
        Key::Semicolon => VK_OEM_1,    // ';:' on US
        Key::Equal => VK_OEM_PLUS,     // '=+' on US
        Key::Comma => VK_OEM_COMMA,    // ',<' on US
        Key::Minus => VK_OEM_MINUS,    // '-_' on US
        Key::Period => VK_OEM_PERIOD,  // '.>' on US
        Key::Slash => VK_OEM_2,        // '/?' on US
        Key::Grave => VK_OEM_3,        // '`~' on US
        Key::LeftBracket => VK_OEM_4,  // '[{' on US
        Key::Backslash => VK_OEM_5,    // '\|' on US
        Key::RightBracket => VK_OEM_6, // ']}' on US
        Key::Apostrophe => VK_OEM_7,   // ''"' on US

        // --- 4. Special Keys ---
        Key::Enter => VK_RETURN,
        Key::Space => VK_SPACE,
        Key::Tab => VK_TAB,
        Key::Escape => VK_ESCAPE,
        Key::Delete => VK_DELETE,
        Key::BackSpace => VK_BACK,

        // --- 5. Navigation & Movement Keys ---
        Key::Home => VK_HOME,
        Key::End => VK_END,
        Key::PageUp => VK_PRIOR,  // Page Up
        Key::PageDown => VK_NEXT, // Page Down
        Key::Insert => VK_INSERT,
        Key::PrintScreen => VK_SNAPSHOT,
        Key::ScrollLock => VK_SCROLL,
        Key::Pause => VK_PAUSE,

        Key::LeftArrow => VK_LEFT,
        Key::RightArrow => VK_RIGHT,
        Key::UpArrow => VK_UP,
        Key::DownArrow => VK_DOWN,
    }
}
