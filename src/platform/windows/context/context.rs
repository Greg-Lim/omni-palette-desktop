use std::ffi::{OsStr, OsString};
use std::mem;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{HWND, MAX_PATH};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

pub fn get_active_window_handle() -> HWND {
    // The GetForegroundWindow function returns the handle of the foreground window
    unsafe { GetForegroundWindow() }
}

fn get_window_title(hwnd: HWND) -> String {
    let mut title_buffer: [u16; 512] = [0; 512]; // Buffer for the title text
    let length = unsafe {
        // GetWindowTextW returns the length of the string copied
        GetWindowTextW(hwnd, &mut title_buffer)
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

fn get_application_name(hwnd: HWND) -> String {
    let mut pid: u32 = 0;
    unsafe {
        // 1. Get the Process ID (PID) from the Window Handle (HWND)
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid == 0 {
            return String::from("[Unknown Process]");
        }

        // 2. Open a handle to the process using the PID
        let process_handle_result = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, // Required access rights
            false,
            pid,
        );

        let process_handle = match process_handle_result {
            Ok(handle) => handle,
            Err(_) => return format!("[PID {} - Access Denied]", pid),
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
            OsString::from_wide(&name_buffer[0..length as usize])
                .to_string_lossy()
                .into_owned()
        } else {
            format!("[PID {} - Name Not Found]", pid)
        }
    }
}

pub fn print_active_window_context() {
    // Retrieve the active window handle
    let hwnd = unsafe { GetForegroundWindow() };

    if !hwnd.is_invalid() {
        let title = get_window_title(hwnd);
        let app_name = get_application_name(hwnd);

        println!("--- Active Window Context ---");
        println!("Handle (HWND): {:?}", hwnd);
        println!("Window Title:  {}", title);
        println!("Application:   {}", app_name);
        println!("-----------------------------");
    } else {
        println!("No active foreground window found.");
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
    fn test_get_active_window_handle_is_safe() {
        // Test 1: Function should execute without panicking
        let active_window = get_active_window_handle();

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
        let ctx = print_active_window_context();
    }
}
