use std::collections::HashSet;
use std::path::Path;
use std::sync::{
    mpsc::{self, Receiver, RecvTimeoutError, Sender},
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

use env_logger::Builder;

use crate::config::ignore::{load_ignored_process_names, normalize_process_name};
use crate::core::extensions::discovery::ExtensionDiscovery;
use crate::core::plugins::PluginRegistry;
use crate::core::registry::registry::{MasterRegistry, UnitAction};
use crate::domain::action::{ActionExecution, ContextRoot, Os};
use crate::domain::hotkey::{Key, KeyboardShortcut};
use crate::platform::hotkey_actions::HotkeyPassthrough;
use crate::platform::platform_interface::{get_all_context, RawWindowHandleExt};
use crate::platform::windows::context::context::{
    focus_window, get_hwnd_from_raw, monitor_work_area_from_window,
};
use crate::platform::windows::sender::hotkey_sender::send_shortcut;
use crate::ui::ui_main;
use crate::ui::ui_main::{Command, PaletteWorkArea, UiEvent, UiSignal};
use std::env::consts::OS;
use std::io::Write;
use windows::Win32::Foundation::HWND;

mod config;
mod core;
mod domain;
mod platform;
mod ui;

fn main() {
    init_logger();

    let current_os = current_os();
    let (ui_tx, ui_rx) = mpsc::channel::<UiSignal>();
    let (event_tx, event_rx) = mpsc::channel::<UiEvent>();

    let extensions_folder = Path::new("./extensions");
    let extension_discovery = ExtensionDiscovery::new(extensions_folder);
    let master_registry = Arc::new(MasterRegistry::build(&extension_discovery, current_os));
    let ignored_process_names = Arc::new(load_ignored_process_names(
        &extension_discovery.ignore_file_path(),
        current_os,
    ));

    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener();
    let hotkey_passthrough = handle.passthrough_sender();
    let _hotkey_bridge = spawn_hotkey_bridge(
        rx,
        ui_tx,
        event_rx,
        Arc::clone(&master_registry),
        Arc::clone(&ignored_process_names),
        hotkey_passthrough,
    );

    ui_main::ui_main(ui_rx, event_tx);

    handle.stop();
}

fn init_logger() {
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
}

fn current_os() -> Os {
    match OS {
        "windows" => Os::Windows,
        "macos" => Os::Mac,
        "linux" => Os::Linux,
        _ => panic!("OS not supported"),
    }
}

fn spawn_hotkey_bridge(
    rx: Receiver<KeyboardShortcut>,
    ui_tx: Sender<UiSignal>,
    event_rx: Receiver<UiEvent>,
    registry: Arc<MasterRegistry>,
    ignored_process_names: Arc<HashSet<String>>,
    hotkey_passthrough: HotkeyPassthrough,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut palette_open = false;

        loop {
            handle_ui_events(&event_rx, &mut palette_open);

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(shortcut) if is_palette_hotkey(shortcut) => {
                    handle_palette_hotkey(
                        shortcut,
                        &ui_tx,
                        &registry,
                        &ignored_process_names,
                        &hotkey_passthrough,
                        &mut palette_open,
                    );
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    })
}

fn handle_ui_events(event_rx: &Receiver<UiEvent>, palette_open: &mut bool) {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            UiEvent::Closed => *palette_open = false,
            UiEvent::ActionExecuted => {}
        }
    }
}

fn is_palette_hotkey(shortcut: KeyboardShortcut) -> bool {
    shortcut.modifier.control && shortcut.modifier.shift && matches!(shortcut.key, Key::KeyP)
}

fn handle_palette_hotkey(
    shortcut: KeyboardShortcut,
    ui_tx: &Sender<UiSignal>,
    registry: &MasterRegistry,
    ignored_process_names: &HashSet<String>,
    hotkey_passthrough: &HotkeyPassthrough,
    palette_open: &mut bool,
) {
    let context_root = get_all_context();

    if let Some(process_name) = ignored_active_process_name(&context_root, ignored_process_names) {
        log::debug!(
            "Forwarding palette hotkey to ignored application: {}",
            process_name
        );
        hotkey_passthrough.forward_shortcut(shortcut);
        return;
    }

    if *palette_open {
        let _ = ui_tx.send(UiSignal::Hide);
        return;
    }

    show_palette(ui_tx, registry, &context_root, palette_open);
}

fn ignored_active_process_name(
    context_root: &ContextRoot,
    ignored_process_names: &HashSet<String>,
) -> Option<String> {
    let process_name = context_root
        .get_active()
        .and_then(|handle| handle.get_app_process_name())?;
    let normalized_name = normalize_process_name(&process_name)?;

    ignored_process_names
        .contains(&normalized_name)
        .then_some(process_name)
}

fn show_palette(
    ui_tx: &Sender<UiSignal>,
    registry: &MasterRegistry,
    context_root: &ContextRoot,
    palette_open: &mut bool,
) {
    let work_area = palette_work_area(context_root);
    let active_hwnd = context_root
        .get_active()
        .and_then(|handle| get_hwnd_from_raw(*handle))
        .map(|hwnd| hwnd.0 as isize);
    let commands = commands_from_unit_actions(
        registry.get_actions(context_root),
        registry.plugin_registry(),
        active_hwnd,
    );

    if ui_tx
        .send(UiSignal::Show {
            commands,
            work_area,
        })
        .is_ok()
    {
        *palette_open = true;
    }
}

fn palette_work_area(context_root: &ContextRoot) -> Option<PaletteWorkArea> {
    context_root
        .get_active()
        .and_then(|handle| get_hwnd_from_raw(*handle))
        .and_then(monitor_work_area_from_window)
        .map(|(left, top, right, bottom)| PaletteWorkArea::from_ltrb(left, top, right, bottom))
}

fn commands_from_unit_actions(
    unit_actions: Vec<UnitAction>,
    plugin_registry: Arc<PluginRegistry>,
    active_hwnd_val: Option<isize>,
) -> Vec<Command> {
    unit_actions
        .into_iter()
        .enumerate()
        .map(|unit_action| {
            command_from_unit_action(unit_action, Arc::clone(&plugin_registry), active_hwnd_val)
        })
        .collect()
}

fn command_from_unit_action(
    (original_order, unit_action): (usize, UnitAction),
    plugin_registry: Arc<PluginRegistry>,
    active_hwnd_val: Option<isize>,
) -> Command {
    let execution = unit_action.execution;
    let target_hwnd_val = unit_action
        .target_window
        .and_then(get_hwnd_from_raw)
        .map(|hwnd| hwnd.0 as isize);

    Command {
        label: format!("{}: {}", unit_action.app_name, unit_action.action_name),
        shortcut_text: unit_action.shortcut_text,
        priority: unit_action.metadata.priority,
        focus_state: unit_action.focus_state,
        starred: unit_action.metadata.starred,
        tags: unit_action.metadata.tags,
        original_order,
        action: Box::new(move || match &execution {
            ActionExecution::Shortcut(shortcut) => {
                if let Some(val) = target_hwnd_val {
                    focus_window(HWND(val as *mut _));
                }
                send_shortcut(shortcut);
            }
            ActionExecution::PluginCommand {
                plugin_id,
                command_id,
            } => {
                if let Some(val) = active_hwnd_val {
                    focus_window(HWND(val as *mut _));
                    std::thread::sleep(Duration::from_millis(75));
                }
                if let Err(err) = plugin_registry.execute(plugin_id, command_id) {
                    log::error!("Failed to execute WASM plugin command: {err}");
                }
            }
        }),
    }
}
