use crate::domain::{
    action::{KeySequenceStep, SequenceKey},
    hotkey::KeyboardShortcut,
};
use crate::platform::windows::mapper::hotkey_mapper::map_key;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_TYPE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(750);
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const SYNTHETIC_INPUT_SETTLE_DELAY: Duration = Duration::from_millis(20);

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
    prepare_synthetic_shortcut_input();

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

pub fn send_shortcut_sequence(sequence: &[KeySequenceStep]) {
    prepare_synthetic_shortcut_input();

    for step in sequence {
        send_sequence_step(step);
        std::thread::sleep(std::time::Duration::from_millis(35));
    }
}

fn prepare_synthetic_shortcut_input() {
    wait_for_modifier_keys_to_be_released(MODIFIER_RELEASE_TIMEOUT);
    release_modifier_keys();
    std::thread::sleep(SYNTHETIC_INPUT_SETTLE_DELAY);
}

fn wait_for_modifier_keys_to_be_released(timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !any_modifier_key_down() {
            return;
        }
        std::thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

fn any_modifier_key_down() -> bool {
    [
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ]
    .into_iter()
    .any(is_key_down)
}

fn is_key_down(vk: VIRTUAL_KEY) -> bool {
    const KEY_DOWN_MASK: i16 = i16::MIN;
    unsafe { GetAsyncKeyState(vk.0 as i32) & KEY_DOWN_MASK != 0 }
}

fn release_modifier_keys() {
    let inputs = [
        make_key_event(VK_RWIN, true),
        make_key_event(VK_LWIN, true),
        make_key_event(VK_RMENU, true),
        make_key_event(VK_LMENU, true),
        make_key_event(VK_MENU, true),
        make_key_event(VK_RSHIFT, true),
        make_key_event(VK_LSHIFT, true),
        make_key_event(VK_SHIFT, true),
        make_key_event(VK_RCONTROL, true),
        make_key_event(VK_LCONTROL, true),
        make_key_event(VK_CONTROL, true),
    ];

    unsafe {
        let _result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_sequence_step(step: &KeySequenceStep) {
    let mut inputs: Vec<INPUT> = Vec::new();

    if step.modifier.control {
        inputs.push(make_key_event(VK_CONTROL, false));
    }
    if step.modifier.shift {
        inputs.push(make_key_event(VK_SHIFT, false));
    }
    if step.modifier.alt {
        inputs.push(make_key_event(VK_MENU, false));
    }
    if step.modifier.win {
        inputs.push(make_key_event(VK_LWIN, false));
    }

    let vk = match step.key {
        SequenceKey::Key(key) => map_key(key),
        SequenceKey::Ctrl => VK_CONTROL,
        SequenceKey::Shift => VK_SHIFT,
        SequenceKey::Alt => VK_MENU,
    };
    inputs.push(make_key_event(vk, false));
    inputs.push(make_key_event(vk, true));

    if step.modifier.win {
        inputs.push(make_key_event(VK_LWIN, true));
    }
    if step.modifier.alt {
        inputs.push(make_key_event(VK_MENU, true));
    }
    if step.modifier.shift {
        inputs.push(make_key_event(VK_SHIFT, true));
    }
    if step.modifier.control {
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
