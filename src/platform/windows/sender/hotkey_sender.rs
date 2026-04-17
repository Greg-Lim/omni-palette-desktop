use crate::domain::hotkey::KeyboardShortcut;
use crate::platform::windows::mapper::hotkey_mapper::map_key;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_TYPE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

// Helper function to create a keyboard press/release event
fn make_key_event(vk: VIRTUAL_KEY, is_release: bool) -> INPUT {
    let mut flags: KEYBD_EVENT_FLAGS = KEYBD_EVENT_FLAGS(0_u32);
    if is_release {
        flags |= KEYEVENTF_KEYUP;
    }

    INPUT {
        r#type: INPUT_TYPE(1), // INPUT_KEYBOARD
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn send_shortcut(shortcut: &KeyboardShortcut) {
    let mut inputs: Vec<INPUT> = Vec::new();

    // Press modifiers
    if shortcut.modifier.control {
        inputs.push(make_key_event(VK_CONTROL, false));
    }
    if shortcut.modifier.shift {
        inputs.push(make_key_event(VK_SHIFT, false));
    }
    if shortcut.modifier.alt {
        inputs.push(make_key_event(VK_MENU, false));
    }
    if shortcut.modifier.win {
        inputs.push(make_key_event(VK_LWIN, false));
    }

    // Press and release the main key
    let vk = map_key(shortcut.key);
    inputs.push(make_key_event(vk, false));
    inputs.push(make_key_event(vk, true));

    // Release modifiers in reverse order
    if shortcut.modifier.win {
        inputs.push(make_key_event(VK_LWIN, true));
    }
    if shortcut.modifier.alt {
        inputs.push(make_key_event(VK_MENU, true));
    }
    if shortcut.modifier.shift {
        inputs.push(make_key_event(VK_SHIFT, true));
    }
    if shortcut.modifier.control {
        inputs.push(make_key_event(VK_CONTROL, true));
    }

    unsafe {
        let _result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn make_unicode_key_event(unit: u16, is_release: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if is_release {
        flags |= KEYEVENTF_KEYUP;
    }

    INPUT {
        r#type: INPUT_TYPE(1), // INPUT_KEYBOARD
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn send_text(text: &str) {
    let mut inputs = Vec::new();
    for unit in text.encode_utf16() {
        inputs.push(make_unicode_key_event(unit, false));
        inputs.push(make_unicode_key_event(unit, true));
    }

    unsafe {
        let _result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::GetLastError;

    #[test]
    fn test_send_alt_tab_sequence_correctness() {
        // Arrange: Prepare the key sequence
        let alt_press = make_key_event(VK_MENU, false);
        let tab_press = make_key_event(VK_TAB, false);
        let tab_release = make_key_event(VK_TAB, true);
        let alt_release = make_key_event(VK_MENU, true);

        let inputs = [alt_press, tab_press, tab_release, alt_release];

        unsafe {
            // Send the sequence of four events
            let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            println!("{result:?}");
            println!("{:?}", GetLastError());
        }
    }

    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_DELETE, VK_TAB};
    #[test]
    fn test_send_ctrl_alt_del_sequence_correctness() {
        // Arrange: Prepare the key sequence
        let ctrl_press = make_key_event(VK_CONTROL, false);
        let clt_press = make_key_event(VK_MENU, false);
        let del_press = make_key_event(VK_DELETE, false);
        let del_release = make_key_event(VK_DELETE, true);
        let clt_release = make_key_event(VK_MENU, true);
        let ctrl_release = make_key_event(VK_CONTROL, true);

        let inputs = [
            ctrl_press,
            clt_press,
            del_press,
            del_release,
            clt_release,
            ctrl_release,
        ];

        unsafe {
            // Send the sequence of four events
            let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            println!("{result:?}");
            println!("{:?}", GetLastError());
        }
    }
}
