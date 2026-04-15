use std::path::Path;
use std::sync::Arc;

use env_logger::Builder;

use crate::models::hotkey::Key;
use crate::platform::platform_interface::get_all_context;
use crate::platform::windows::context::context::{focus_window, get_hwnd_from_raw};
use crate::platform::windows::sender::hotkey_sender::send_shortcut;
use crate::ui::ui_main;
use crate::ui::ui_main::{Command, UiEvent, UiSignal};
use crate::{core::registry::registry::MasterRegistry, models::action::Os};
use std::env::consts::OS;
use std::io::Write;
use std::sync::mpsc;
use windows::Win32::Foundation::HWND;

mod core;
mod models;
mod platform;
mod ui;

fn main() {
    let mut builder = Builder::from_default_env();

    builder.format(|buf, record| {
        writeln!(
            buf,
            "[{}] {}:{}: {}",
            record.level(),
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.args()
        )
    });

    builder.init();

    let current_os = match OS {
        "windows" => Os::Windows,
        "macos" => Os::Mac,
        "linux" => Os::Linux,
        _ => panic!("OS not supported"),
    };

    let (ui_tx, ui_rx) = mpsc::channel::<UiSignal>();
    let (event_tx, event_rx) = mpsc::channel::<UiEvent>();

    let extensions_folder = Path::new("./extensions");
    let master_registry = Arc::new(MasterRegistry::build(extensions_folder, current_os));

    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener();
    let registry_clone = Arc::clone(&master_registry);

    std::thread::spawn(move || {
        use std::sync::mpsc::RecvTimeoutError;
        use std::time::Duration;

        let mut palette_open = false;

        loop {
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    UiEvent::Closed => palette_open = false,
                    UiEvent::ActionExecuted => {}
                }
            }

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ev) => {
                    if ev.modifier.control && ev.modifier.shift && matches!(ev.key, Key::KeyP) {
                        if palette_open {
                            let _ = ui_tx.send(UiSignal::Hide);
                        } else {
                            let context_root = get_all_context();
                            let unit_actions = registry_clone.get_actions(&context_root);

                            let commands: Vec<Command> = unit_actions
                                .into_iter()
                                .enumerate()
                                .map(|(original_order, ua)| {
                                    let label = format!("{}: {}", ua.app_name, ua.action_name);
                                    let shortcut = ua.keyboard_shortcut;
                                    let target_hwnd_val: Option<isize> = ua
                                        .target_window
                                        .and_then(get_hwnd_from_raw)
                                        .map(|hwnd| hwnd.0 as isize);

                                    let shortcut_text = ua.keyboard_shortcut.to_string();

                                    Command {
                                        label,
                                        shortcut_text,
                                        priority: ua.metadata.priority,
                                        focus_state: ua.focus_state,
                                        starred: ua.metadata.starred,
                                        tags: ua.metadata.tags,
                                        original_order,
                                        action: Box::new(move || {
                                            if let Some(val) = target_hwnd_val {
                                                focus_window(HWND(val as *mut _));
                                            }
                                            send_shortcut(&shortcut);
                                        }),
                                    }
                                })
                                .collect();

                            if ui_tx.send(UiSignal::Show(commands)).is_ok() {
                                palette_open = true;
                            }
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    ui_main::ui_main(ui_rx, event_tx);

    handle.stop();
}
