use std::path::Path;

use env_logger::Builder;
use log::{error, info};

use crate::core::context;
use crate::core::registry::registry::UnitAction;
use crate::core::search::{get_score, MatchResult};
use crate::models::action::{AppProcessName, ContextRoot};
use crate::models::hotkey::Key;
use crate::platform::platform_interface::{get_all_context, RawWindowHandleExt};
use crate::ui::ui_main;
use crate::ui::ui_main::UiSignal;
use crate::{core::registry::registry::MasterRegistry, models::action::Os};
use std::env::consts::OS;
use std::io;
use std::io::Write;
use std::sync::mpsc;

mod core;
mod models;
mod platform;
mod ui;
// fn main() {
//     let (handle, rx) = platform::hotkey_actions::start_hotkey_listener();

//     std::thread::spawn(move || {
//         while let Ok(ev) = rx.recv() {
//             println!("Hotkey pressed: {ev:?}");
//         }
//     });

//     // Keep process alive until Ctrl+C
//     loop {
//         std::thread::sleep(std::time::Duration::from_secs(10));
//         break;
//     }

//     handle.stop(); // unreachable here unless you add a break condition
//     platform::hotkey_actions::send_ctrl_v();
// }

fn main() {
    // Set Up logger
    let mut builder = Builder::from_default_env();

    builder.format(|buf, record| {
        writeln!(
            buf,
            "[{}] {}:{}: {}",
            record.level(),
            record.file().unwrap_or("unknown"), // The file name
            record.line().unwrap_or(0),         // The line number
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

    // UI channel
    let (ui_tx, ui_rx) = mpsc::channel::<UiSignal>();

    // Register and listen for hot keys
    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener();
    let ui_tx_clone = ui_tx.clone();
    std::thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            dbg!(ev);

            // For now we do this
            if ev.modifier.control && ev.modifier.shift && matches!(ev.key, Key::KeyP) {
                match ui_tx_clone.send(UiSignal::ToggleVisibility) {
                    Ok(_) => {
                        info!("Successfully sent UiSignal::ToggleVisibility");
                        std::io::stdout().flush().unwrap();
                        // Give UI thread a moment to process
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => error!("Failed to send UiSignal: {e}"),
                }
            }
        }
    });

    // Find and load extentions // This needs to be hot loaded in the future
    let extensions_folder = Path::new("./extensions");
    let master_registry = MasterRegistry::build(extensions_folder, current_os);
    let context_root = get_all_context();

    let avail_actions = master_registry.get_actions(&context_root);
    // dbg!(&master_registry);
    let process_names: AppProcessName = context_root
        .fg_context
        .iter()
        .map(|h| h.get_app_process_name().unwrap_or("missing".into()))
        .collect();
    dbg!(&process_names);
    dbg!(&avail_actions);

    let mut user_input = String::new();

    // 2. Read the line from stdin
    // println!("Input sarch term: ");
    // io::stdin()
    //     .read_line(&mut user_input) // read_line appends the input to the string
    //     .expect("Failed to read line"); // Handle potential errors
    // let user_input: String = user_input.trim().to_string();

    // let mut match_results: Vec<MatchResult> = vec![];
    // for unit_action in avail_actions {
    //     let match_res = get_score(&unit_action.action_name, &user_input);
    //     match_results.push(match_res);
    // }

    // dbg!(match_results);

    // Run UI on the main thread (winit requires the event loop on main)
    ui_main::ui_main(ui_rx);

    // cleanup
    handle.stop();
    // platform::hotkey_actions::send_ctrl_v();
}
