pub mod hotkey_actions;
pub mod platform_interface;
pub mod register_receiver;

#[cfg(target_os = "windows")]
mod windows;
