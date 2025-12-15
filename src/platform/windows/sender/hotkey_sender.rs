use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_TYPE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_MENU, VK_TAB, VK_V,
};

// Helper function to create a keyboard press/release event
fn make_key_event(vk: VIRTUAL_KEY, is_release: bool) -> INPUT {
    let mut flags: KEYBD_EVENT_FLAGS = KEYBD_EVENT_FLAGS(0 as u32);
    if is_release {
        flags |= KEYEVENTF_KEYUP;
    }

    // The INPUT structure must be initialized carefully
    let mut input = INPUT {
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
    };
    input
}

pub fn send_ctrl_v() {
    // 1. CTRL Press
    let ctrl_press = make_key_event(VK_MENU, false);
    // 2. C Press
    let c_press = make_key_event(VK_TAB, false);
    // 3. C Release
    let c_release = make_key_event(VK_TAB, true);
    // 4. CTRL Release
    let ctrl_release = make_key_event(VK_MENU, true);

    let inputs = [ctrl_press, c_press, c_release, ctrl_release];

    unsafe {
        // Send the sequence of four events
        let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        // Check result for success/failure...
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

    use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
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
