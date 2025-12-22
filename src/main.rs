use std::path::Path;

use env_logger::Builder;

use crate::core::context;
use crate::core::registry::registry::UnitAction;
use crate::core::search::{get_score, MatchResult};
use crate::models::action::{AppProcessName, ContextRoot};
use crate::platform::platform_interface::{get_all_context, RawWindowHandleExt};
use crate::{core::registry::registry::MasterRegistry, models::action::Os};
use std::env::consts::OS;
use std::io;
use std::io::Write;

mod core;
mod models;
mod platform;
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

    // Register and listern for hot keys
    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener();

    std::thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            println!("Hotkey pressed: {ev:?}");
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
    println!("Input sarch term: ");
    io::stdin()
        .read_line(&mut user_input) // read_line appends the input to the string
        .expect("Failed to read line"); // Handle potential errors
    let user_input: String = user_input.trim().to_string();

    let mut match_results: Vec<MatchResult> = vec![];
    for unit_action in avail_actions {
        let match_res = get_score(&unit_action.action_name, &user_input);
        match_results.push(match_res);
    }

    dbg!(match_results);

    // Keep process alive until Ctrl+C
    loop {
        // std::thread::sleep(std::time::Duration::from_secs(10));
        break;
    }

    // dbg!(&master_registry);
    // dbg!(master_registry.get_actions(context));

    // cleanup
    handle.stop();
    // platform::hotkey_actions::send_ctrl_v();
}
