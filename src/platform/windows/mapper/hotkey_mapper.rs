use crate::models::hotkey::{self, HotkeyModifiers, Key, Modifier};
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

pub fn map_modifier_back(flags: HOT_KEY_MODIFIERS) -> HotkeyModifiers {
    HotkeyModifiers {
        // Check if the CONTROL bit is set in the flags
        control: (flags & MOD_CONTROL) == MOD_CONTROL,

        // Check if the SHIFT bit is set in the flags
        shift: (flags & MOD_SHIFT) == MOD_SHIFT,

        // Check if the ALT bit is set in the flags
        alt: (flags & MOD_ALT) == MOD_ALT,

        // Check if the WIN bit is set in the flags
        win: (flags & MOD_WIN) == MOD_WIN,
    }
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

pub fn map_key_back(vk: VIRTUAL_KEY) -> Option<Key> {
    match vk {
        // --- 1. Alphanumeric Keys ---
        VK_A => Some(Key::KeyA),
        VK_B => Some(Key::KeyB),
        VK_C => Some(Key::KeyC),
        VK_D => Some(Key::KeyD),
        VK_E => Some(Key::KeyE),
        VK_F => Some(Key::KeyF),
        VK_G => Some(Key::KeyG),
        VK_H => Some(Key::KeyH),
        VK_I => Some(Key::KeyI),
        VK_J => Some(Key::KeyJ),
        VK_K => Some(Key::KeyK),
        VK_L => Some(Key::KeyL),
        VK_M => Some(Key::KeyM),
        VK_N => Some(Key::KeyN),
        VK_O => Some(Key::KeyO),
        VK_P => Some(Key::KeyP),
        VK_Q => Some(Key::KeyQ),
        VK_R => Some(Key::KeyR),
        VK_S => Some(Key::KeyS),
        VK_T => Some(Key::KeyT),
        VK_U => Some(Key::KeyU),
        VK_V => Some(Key::KeyV),
        VK_W => Some(Key::KeyW),
        VK_X => Some(Key::KeyX),
        VK_Y => Some(Key::KeyY),
        VK_Z => Some(Key::KeyZ),

        VK_0 => Some(Key::Key0),
        VK_1 => Some(Key::Key1),
        VK_2 => Some(Key::Key2),
        VK_3 => Some(Key::Key3),
        VK_4 => Some(Key::Key4),
        VK_5 => Some(Key::Key5),
        VK_6 => Some(Key::Key6),
        VK_7 => Some(Key::Key7),
        VK_8 => Some(Key::Key8),
        VK_9 => Some(Key::Key9),

        // --- 2. Function Keys ---
        VK_F1 => Some(Key::F1),
        VK_F2 => Some(Key::F2),
        VK_F3 => Some(Key::F3),
        VK_F4 => Some(Key::F4),
        VK_F5 => Some(Key::F5),
        VK_F6 => Some(Key::F6),
        VK_F7 => Some(Key::F7),
        VK_F8 => Some(Key::F8),
        VK_F9 => Some(Key::F9),
        VK_F10 => Some(Key::F10),
        VK_F11 => Some(Key::F11),
        VK_F12 => Some(Key::F12),

        // --- 3. Punctuation & Symbol Keys ---
        VK_OEM_1 => Some(Key::Semicolon),
        VK_OEM_PLUS => Some(Key::Equal),
        VK_OEM_COMMA => Some(Key::Comma),
        VK_OEM_MINUS => Some(Key::Minus),
        VK_OEM_PERIOD => Some(Key::Period),
        VK_OEM_2 => Some(Key::Slash),
        VK_OEM_3 => Some(Key::Grave),
        VK_OEM_4 => Some(Key::LeftBracket),
        VK_OEM_5 => Some(Key::Backslash),
        VK_OEM_6 => Some(Key::RightBracket),
        VK_OEM_7 => Some(Key::Apostrophe),

        // --- 4. Special Keys ---
        VK_RETURN => Some(Key::Enter),
        VK_SPACE => Some(Key::Space),
        VK_TAB => Some(Key::Tab),
        VK_ESCAPE => Some(Key::Escape),
        VK_DELETE => Some(Key::Delete),
        VK_BACK => Some(Key::BackSpace),

        // --- 5. Navigation & Movement Keys ---
        VK_HOME => Some(Key::Home),
        VK_END => Some(Key::End),
        VK_PRIOR => Some(Key::PageUp),
        VK_NEXT => Some(Key::PageDown),
        VK_INSERT => Some(Key::Insert),
        VK_SNAPSHOT => Some(Key::PrintScreen),
        VK_SCROLL => Some(Key::ScrollLock),
        VK_PAUSE => Some(Key::Pause),

        VK_LEFT => Some(Key::LeftArrow),
        VK_RIGHT => Some(Key::RightArrow),
        VK_UP => Some(Key::UpArrow),
        VK_DOWN => Some(Key::DownArrow),

        // Handle unknown keys gracefully
        _ => None,
    }
}
