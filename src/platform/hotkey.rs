#[cfg(target_os = "windows")]
pub use crate::platform::windows::hotkey_reciever::*;
#[cfg(target_os = "windows")]
pub use crate::platform::windows::hotkey_sender::*;

#[cfg(not(target_os = "windows"))]
mod stub {
    use std::sync::mpsc::{self, Receiver};

    #[derive(Debug, Clone, Copy)]
    pub struct HotkeyEvent {
        pub id: i32,
        pub vk: u32,
        pub modifiers: u32,
    }

    pub struct HotkeyHandle;
    impl HotkeyHandle {
        pub fn stop(self) {}
    }

    pub fn start_hotkey_listener() -> (HotkeyHandle, Receiver<HotkeyEvent>) {
        // No-op on non-Windows for now
        let (_tx, rx) = mpsc::channel();
        (HotkeyHandle, rx)
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::*;
