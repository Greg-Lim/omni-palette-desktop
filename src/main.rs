use std::path::Path;

use env_logger::Builder;

use crate::{core::registry::registry::MasterRegistry, models::action::Os};
use std::env::consts::OS;
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

    // Keep process alive until Ctrl+C
    loop {
        std::thread::sleep(std::time::Duration::from_secs(10));
        break;
    }

    // Find and load extentions // This needs to be hot loaded in the future
    let extensions_folder = Path::new("./extensions");
    let mut master_registry = MasterRegistry::build(extensions_folder, current_os);

    // // Use a match on read_dir to handle the folder being missing/inaccessible
    // match fs::read_dir(extensions_folder) {
    //     Ok(entries) => {
    //         for (idx, entry) in entries.enumerate() {
    //             // Handle individual directory entry errors
    //             let entry = match entry {
    //                 Ok(e) => e,
    //                 Err(err) => {
    //                     warn!("Failed to read directory entry {}: {}", idx, err);
    //                     continue;
    //                 }
    //             };

    //             let path = entry.path();

    //             // Skip non-toml files
    //             if path.extension().and_then(|s| s.to_str()) != Some("toml") {
    //                 continue;
    //             }

    //             // Load and build application
    //             match load_config(&path).and_then(|c| Application::new(&c, &current_os)) {
    //                 Ok(app) => {
    //                     info!(
    //                         "Successfully loaded extension: {:?}",
    //                         path.file_name().unwrap()
    //                     );
    //                     master_registry.application_registry.insert(idx as u32, app);
    //                 }
    //                 Err(err) => {
    //                     error!("Failed to load extension at {:?}: {}", path, err);
    //                 }
    //             }
    //         }
    //     }
    //     Err(e) => error!(
    //         "Could not access extensions directory at {:?}: {}",
    //         extensions_folder, e
    //     ),
    // }

    dbg!(&master_registry);

    // cleanup
    handle.stop();
    // platform::hotkey_actions::send_ctrl_v();
}
