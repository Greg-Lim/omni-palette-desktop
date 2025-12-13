mod platform;
fn main() {
    let (handle, rx) = platform::hotkey::start_hotkey_listener();

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

    handle.stop(); // unreachable here unless you add a break condition
    platform::hotkey::send_ctrl_v();
}
