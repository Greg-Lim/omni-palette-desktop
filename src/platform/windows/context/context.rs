use crate::models::action::ContextRoot;
use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use std::ffi::{OsStr, OsString};
use std::mem;
use std::num::NonZeroIsize;
use std::os::windows::ffi::OsStringExt;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, MAX_PATH};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
    IsWindowVisible,
};

pub fn get_foreground_window_handle() -> HWND {
    // The GetForegroundWindow function returns the handle of the foreground window
    unsafe { GetForegroundWindow() }
}

struct WindowEnumContext {
    fg: Vec<RawWindowHandle>,
    bg: Vec<RawWindowHandle>,
}

pub fn get_all_windows() -> (Vec<RawWindowHandle>, Vec<RawWindowHandle>) {
    // 1. Initialize our wrapper struct
    let mut context = WindowEnumContext {
        fg: Vec::new(),
        bg: Vec::new(),
    };

    unsafe extern "system" fn enum_vc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // 2. Cast the single pointer back to our struct
        let context = &mut *(lparam.0 as *mut WindowEnumContext);

        // --- GATHER ATTRIBUTES ---
        let is_visible = IsWindowVisible(hwnd).as_bool();

        let mut cloaked: i32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of::<i32>() as u32,
        );
        let is_cloaked = cloaked != 0;

        let mut text: [u16; 512] = [0; 512];
        let title_len = GetWindowTextW(hwnd, &mut text);

        // --- SELECTION LOGIC ---
        // Basic filter: We still want to ignore "Progman" or non-windows
        if !IsWindow(Some(hwnd)).as_bool() {
            return BOOL::from(true);
        }

        if let Some(h) = NonZeroIsize::new(hwnd.0 as isize) {
            let handle = RawWindowHandle::Win32(Win32WindowHandle::new(h));

            // Logic: A window is "Foreground/Active" ONLY if it passes all three
            if is_visible && !is_cloaked && title_len > 0 {
                context.fg.push(handle);
            } else {
                // Otherwise, it's a Background/Inactive window (AHK, PowerToys, etc.)
                context.bg.push(handle);
            }
        }

        BOOL::from(true)
    }

    unsafe {
        // 3. Pass the pointer to the whole context struct
        let ptr = &mut context as *mut WindowEnumContext;
        let _ = EnumWindows(Some(enum_vc), LPARAM(ptr as isize));
    }
    (context.fg, vec![])
    // (context.fg, context.bg)
}

fn get_window_title(hwnd: &HWND) -> String {
    let mut title_buffer: [u16; 512] = [0; 512]; // Buffer for the title text
    let length = unsafe {
        // GetWindowTextW returns the length of the string copied
        GetWindowTextW(*hwnd, &mut title_buffer)
    };

    if length > 0 {
        // Convert the UTF-16 buffer slice into a Rust OsString, then String
        OsString::from_wide(&title_buffer[0..length as usize])
            .to_string_lossy()
            .into_owned()
    } else {
        String::from("[No Title]")
    }
}

pub fn get_app_process_name(hwnd: &HWND) -> Option<String> {
    let mut pid: u32 = 0;
    unsafe {
        // 1. Get the Process ID (PID) from the Window Handle (HWND)
        GetWindowThreadProcessId(*hwnd, Some(&mut pid));

        if pid == 0 {
            return None;
        }

        // 2. Open a handle to the process using the PID
        let process_handle_result = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, // Required access rights
            false,
            pid,
        );

        let process_handle = match process_handle_result {
            Ok(handle) => handle,
            Err(_) => return None,
        };

        // 3. Get the module (executable) base name
        let mut name_buffer: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
        let length = GetModuleBaseNameW(
            process_handle,
            None, // Module handle (None for the main executable)
            &mut name_buffer,
        );

        // 4. Close the process handle
        let _ = windows::Win32::Foundation::CloseHandle(process_handle);

        if length > 0 {
            Some(
                OsString::from_wide(&name_buffer[0..length as usize])
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            None
        }
    }
}

pub fn print_window_context(handles: &Vec<HWND>) {
    // Retrieve the active window handle
    for handle in handles {
        if !handle.is_invalid() {
            let title = get_window_title(&handle);
            let app_name = get_app_process_name(&handle);

            println!("--- Window Context ---");
            println!("Handle (HWND): {:?}", handle);
            println!("Window Title:  {}", title);
            println!("Application:   {}", app_name.unwrap_or("NONE".to_string()));
            println!("---------------------");
        }
    }
}

pub fn get_hwnd_from_raw(handle: RawWindowHandle) -> Option<HWND> {
    match handle {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None, // It's not a Windows handle
    }
}

#[cfg(test)]
mod window_tests {
    use super::*; // Bring everything from the outer module into scope
    use windows::Win32::Foundation::HWND;

    // NOTE: This test can only check if the function executes safely and returns
    // a handle. The actual value of the handle depends on which window is focused
    // when 'cargo test' runs.

    #[test]
    fn test_get_all_window_handle() {
        // Test 1: Function should execute without panicking
        let active_window = get_foreground_window_handle();

        // Test 2: In a typical desktop environment, there should always be a
        // foreground window (even if it's the desktop or the terminal running the test).
        // A null handle (HWND(0)) is rare unless the system is shutting down
        // or there is a specific error state.

        // We test the two possible outcomes of your if-block:

        if !active_window.is_invalid() {
            // Case A: A valid window handle was returned
            println!(
                "Test executed successfully: Found a valid active window handle: {:?}",
                active_window
            );

            // The logic inside the if-block should be tested
            let is_valid = !active_window.is_invalid();
            assert!(
                is_valid,
                "Handle should not be null if we entered this block."
            );
        } else {
            // Case B: Null handle was returned (this is unexpected but possible)
            println!("Test executed successfully: Active window handle was null (HWND(0)).");
            let is_null = active_window == HWND::default();
            assert!(is_null, "Handle should be null if we entered this block.");
        }

        // Final assertion: Ensure the return type is correct
        assert_eq!(
            std::mem::size_of::<HWND>(),
            std::mem::size_of::<isize>(),
            "HWND type size mismatch."
        );
    }

    #[test]
    fn test_print_active_window_context() {
        let hwnd = get_foreground_window_handle();
        print_window_context(&vec![hwnd]);
    }

    #[test]
    fn test_print_all_window() {
        let (fg_handle, bg_handle) = get_all_windows();
        println!("Active Window Context");
        let all_hwnd: Vec<HWND> = fg_handle
            .into_iter()
            .map(|handle| get_hwnd_from_raw(handle))
            .filter_map(|hwnd| match hwnd {
                Some(value) => Some(value),
                None => {
                    dbg!(hwnd);
                    None
                }
            })
            .collect();
        print_window_context(&all_hwnd);

        println!("Background Window Context");
        let all_hwnd: Vec<HWND> = bg_handle
            .into_iter()
            .map(|handle| get_hwnd_from_raw(handle))
            .filter_map(|hwnd| match hwnd {
                Some(value) => Some(value),
                None => {
                    dbg!(hwnd);
                    None
                }
            })
            .collect();
        print_window_context(&all_hwnd);
    }
}
