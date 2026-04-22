use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    mpsc::{self, Receiver, RecvTimeoutError, Sender},
    Arc, OnceLock, RwLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use env_logger::Builder;

use crate::config::{
    ignore::{load_ignored_process_names, normalize_process_name},
    runtime::{RuntimeConfig, RuntimePaths},
};
use crate::core::extensions::{
    catalog::{CatalogEntry, ExtensionCatalog, ExtensionKind},
    discovery::{user_extensions_root, ExtensionDiscovery},
    install::ExtensionInstallService,
};
use crate::core::plugins::PluginRegistry;
use crate::core::registry::registry::{MasterRegistry, UnitAction};
use crate::domain::action::{ActionExecution, CommandPriority, ContextRoot, FocusState, Os};
use crate::domain::hotkey::KeyboardShortcut;
use crate::platform::hotkey_actions::HotkeyPassthrough;
use crate::platform::platform_interface::{get_all_context, RawWindowHandleExt};
use crate::platform::windows::context::context::{
    focus_window, get_hwnd_from_raw, monitor_work_area_from_window,
};
use crate::platform::windows::sender::hotkey_sender::send_shortcut;
use crate::ui::ui_main;
use crate::ui::ui_main::{
    Command, PaletteWorkArea, SharedUiContext, SharedUiVisibility, UiEvent, UiSignal,
};
use std::env::consts::OS;
use std::io::Write;
use windows::Win32::Foundation::HWND;

mod config;
mod core;
mod domain;
mod platform;
mod ui;

const BUNDLED_EXTENSIONS_ROOT: &str = "./extensions/bundled";

type SharedRegistry = Arc<RwLock<MasterRegistry>>;
type SharedIgnoredProcessNames = Arc<RwLock<HashSet<String>>>;

#[derive(Clone)]
struct RuntimeState {
    registry: SharedRegistry,
    ignored_process_names: SharedIgnoredProcessNames,
    current_os: Os,
    bundled_extensions_root: PathBuf,
}

fn main() {
    init_logger();

    let current_os = current_os();
    let runtime_paths = RuntimePaths::from_environment();
    let runtime_config = RuntimeConfig::load(
        runtime_paths.config_path.as_deref(),
        Path::new("./config.toml"),
    );
    log::info!(
        "Using palette activation shortcut: {}",
        runtime_config.activation
    );
    log::debug!(
        "Runtime startup config: launch_on_login={}, start_hidden={}",
        runtime_config.startup.launch_on_login,
        runtime_config.startup.start_hidden
    );
    if let Some(cache_root) = &runtime_paths.local_cache_root {
        log::debug!("Using local cache root: {:?}", cache_root);
    }
    if runtime_config.github.enabled {
        log::info!(
            "Using GitHub extension catalog: {}",
            runtime_config.github.catalog_url()
        );
        log::debug!(
            "GitHub extension catalog public key configured: {}",
            !runtime_config.github.public_key.is_empty()
        );
        log::debug!(
            "Using GitHub extension catalog signature: {}",
            runtime_config.github.signature_url()
        );
    }

    match handle_cli_command(&runtime_config, current_os) {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    let (ui_tx, ui_rx) = mpsc::channel::<UiSignal>();
    let (event_tx, event_rx) = mpsc::channel::<UiEvent>();
    let ui_context: SharedUiContext = Arc::new(OnceLock::new());
    let ui_visibility: SharedUiVisibility = Arc::new(AtomicBool::new(false));

    let bundled_extensions_root = PathBuf::from(BUNDLED_EXTENSIONS_ROOT);
    let extension_discovery = ExtensionDiscovery::bundled_with_user_root(&bundled_extensions_root);
    let registry = Arc::new(RwLock::new(MasterRegistry::build(
        &extension_discovery,
        current_os,
    )));
    let ignored_process_names = Arc::new(RwLock::new(load_ignored_process_names(
        &extension_discovery.ignore_file_path(),
        current_os,
    )));
    let runtime_state = RuntimeState {
        registry,
        ignored_process_names,
        current_os,
        bundled_extensions_root,
    };

    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener(runtime_config.activation);
    let hotkey_passthrough = handle.passthrough_sender();
    let _hotkey_bridge = spawn_hotkey_bridge(
        rx,
        ui_tx,
        event_rx,
        runtime_state.clone(),
        hotkey_passthrough,
        runtime_config.activation,
        Arc::clone(&ui_context),
    );
    #[cfg(debug_assertions)]
    let _telemetry_thread =
        spawn_debug_telemetry(runtime_state.clone(), Arc::clone(&ui_visibility));

    ui_main::ui_main_with_shared_state(ui_rx, event_tx, ui_context, ui_visibility);

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

fn handle_cli_command(runtime_config: &RuntimeConfig, current_os: Os) -> Result<bool, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(false);
    }
    if matches!(args[0].as_str(), "-h" | "--help") {
        print_extension_cli_usage();
        return Ok(true);
    }
    if args[0] != "ext" {
        return Ok(false);
    }

    match args.get(1).map(String::as_str) {
        Some("catalog") if args.len() == 2 => {
            list_extension_catalog(runtime_config, current_os)?;
            Ok(true)
        }
        Some("install") if args.len() == 3 => {
            install_extension_from_catalog(runtime_config, current_os, &args[2])?;
            Ok(true)
        }
        Some("public-key") if args.len() == 3 => {
            print_public_key(&args[2])?;
            Ok(true)
        }
        Some("sign-catalog") if args.len() == 4 => {
            sign_catalog(Path::new(&args[2]), &args[3])?;
            Ok(true)
        }
        Some("-h" | "--help" | "help") => {
            print_extension_cli_usage();
            Ok(true)
        }
        _ => Err(extension_cli_usage()),
    }
}

fn list_extension_catalog(runtime_config: &RuntimeConfig, current_os: Os) -> Result<(), String> {
    let service = ExtensionInstallService::new(extension_install_root()?);
    let catalog = service
        .fetch_catalog(&runtime_config.github)
        .map_err(|err| err.to_string())?;
    let matching_entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.platform == current_os)
        .collect::<Vec<_>>();

    if matching_entries.is_empty() {
        println!(
            "Catalog fetched, but no {} extensions are available.",
            os_name(current_os)
        );
        return Ok(());
    }

    println!("Available {} extensions:", os_name(current_os));
    for entry in matching_entries {
        println!("- {} {} ({:?})", entry.id, entry.version, entry.kind);
    }
    Ok(())
}

fn install_extension_from_catalog(
    runtime_config: &RuntimeConfig,
    current_os: Os,
    extension_id: &str,
) -> Result<(), String> {
    let install_root = extension_install_root()?;
    let service = ExtensionInstallService::new(&install_root);
    let catalog = service
        .fetch_catalog(&runtime_config.github)
        .map_err(|err| err.to_string())?;
    let entry = find_install_entry(&catalog, extension_id, current_os)?;
    let installed = service
        .install_entry(&runtime_config.github, entry, current_os)
        .map_err(|err| err.to_string())?;

    println!(
        "Installed {} {} to {}",
        installed.id,
        installed.version,
        installed.installed_path.display()
    );
    println!(
        "Installed metadata: {}",
        install_root.join("installed.toml").display()
    );
    Ok(())
}

fn find_install_entry<'a>(
    catalog: &'a ExtensionCatalog,
    extension_id: &str,
    current_os: Os,
) -> Result<&'a CatalogEntry, String> {
    let same_id = catalog
        .entries
        .iter()
        .filter(|entry| entry.id == extension_id)
        .collect::<Vec<_>>();
    if same_id.is_empty() {
        let available = current_os_catalog_ids(catalog, current_os);
        return Err(format!(
            "Extension '{extension_id}' was not found for any platform. Available {} extensions: {}",
            os_name(current_os),
            available
        ));
    }

    same_id
        .into_iter()
        .find(|entry| entry.platform == current_os && entry.kind == ExtensionKind::Static)
        .ok_or_else(|| {
            format!(
                "Extension '{extension_id}' exists in the catalog, but there is no static {} package for it.",
                os_name(current_os)
            )
        })
}

fn current_os_catalog_ids(catalog: &ExtensionCatalog, current_os: Os) -> String {
    let ids = catalog
        .entries
        .iter()
        .filter(|entry| entry.platform == current_os)
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();

    if ids.is_empty() {
        "(none)".to_string()
    } else {
        ids.join(", ")
    }
}

fn extension_install_root() -> Result<std::path::PathBuf, String> {
    user_extensions_root().ok_or_else(|| {
        "APPDATA is not set, so Omni Palette cannot determine the user extension install folder."
            .to_string()
    })
}

fn sign_catalog(catalog_path: &Path, secret_key_base64: &str) -> Result<(), String> {
    let catalog_bytes = fs::read(catalog_path)
        .map_err(|err| format!("Could not read catalog {}: {err}", catalog_path.display()))?;
    let signing_key = signing_key_from_base64(secret_key_base64)?;
    let signature = signing_key.sign(&catalog_bytes);
    let signature_base64 = STANDARD.encode(signature.to_bytes());
    let signature_path = catalog_signature_path(catalog_path)?;

    fs::write(&signature_path, signature_base64).map_err(|err| {
        format!(
            "Could not write signature {}: {err}",
            signature_path.display()
        )
    })?;

    println!("Wrote {}", signature_path.display());
    println!(
        "Public key: {}",
        STANDARD.encode(signing_key.verifying_key().to_bytes())
    );
    Ok(())
}

fn print_public_key(secret_key_base64: &str) -> Result<(), String> {
    let signing_key = signing_key_from_base64(secret_key_base64)?;
    println!(
        "{}",
        STANDARD.encode(signing_key.verifying_key().to_bytes())
    );
    Ok(())
}

fn signing_key_from_base64(secret_key_base64: &str) -> Result<SigningKey, String> {
    let bytes = STANDARD
        .decode(secret_key_base64.trim())
        .map_err(|err| format!("Could not decode secret key base64: {err}"))?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Secret key must decode to exactly 32 bytes.".to_string())?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

fn catalog_signature_path(catalog_path: &Path) -> Result<std::path::PathBuf, String> {
    let file_name = catalog_path
        .file_name()
        .ok_or_else(|| format!("Catalog path has no file name: {}", catalog_path.display()))?;
    Ok(catalog_path.with_file_name(format!("{}.sig", file_name.to_string_lossy())))
}

fn print_extension_cli_usage() {
    println!("{}", extension_cli_usage());
}

fn extension_cli_usage() -> String {
    [
        "Usage:",
        "  cargo run -- ext catalog",
        "  cargo run -- ext install <extension_id>",
        "  cargo run -- ext public-key <secret_key_base64>",
        "  cargo run -- ext sign-catalog <catalog_json_path> <secret_key_base64>",
    ]
    .join("\n")
}

fn os_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "windows",
        Os::Mac => "macos",
        Os::Linux => "linux",
    }
}

fn spawn_hotkey_bridge(
    rx: Receiver<KeyboardShortcut>,
    ui_tx: Sender<UiSignal>,
    event_rx: Receiver<UiEvent>,
    runtime_state: RuntimeState,
    hotkey_passthrough: HotkeyPassthrough,
    activation_shortcut: KeyboardShortcut,
    ui_context: SharedUiContext,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut palette_open = false;

        loop {
            handle_ui_events(&event_rx, &mut palette_open);

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(shortcut) if is_palette_hotkey(shortcut, activation_shortcut) => {
                    handle_palette_hotkey(
                        shortcut,
                        &ui_tx,
                        &runtime_state,
                        &hotkey_passthrough,
                        &mut palette_open,
                        &ui_context,
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

fn is_palette_hotkey(shortcut: KeyboardShortcut, activation_shortcut: KeyboardShortcut) -> bool {
    shortcut == activation_shortcut
}

fn handle_palette_hotkey(
    shortcut: KeyboardShortcut,
    ui_tx: &Sender<UiSignal>,
    runtime_state: &RuntimeState,
    hotkey_passthrough: &HotkeyPassthrough,
    palette_open: &mut bool,
    ui_context: &SharedUiContext,
) {
    let context_root = get_all_context();

    if let Some(process_name) =
        ignored_active_process_name_from_shared(&context_root, &runtime_state.ignored_process_names)
    {
        log::debug!(
            "Forwarding palette hotkey to ignored application: {}",
            process_name
        );
        hotkey_passthrough.forward_shortcut(shortcut);
        return;
    }

    if *palette_open {
        if ui_tx.send(UiSignal::Hide).is_ok() {
            request_ui_repaint(ui_context);
        }
        return;
    }

    show_palette(
        ui_tx,
        runtime_state,
        &context_root,
        palette_open,
        ui_context,
    );
}

fn ignored_active_process_name_from_shared(
    context_root: &ContextRoot,
    ignored_process_names: &SharedIgnoredProcessNames,
) -> Option<String> {
    let ignored_process_names = ignored_process_names.read().map_err(|err| {
        log::error!("Ignored process registry lock poisoned: {err}");
    });
    let ignored_process_names = ignored_process_names.ok()?;
    ignored_active_process_name(context_root, &ignored_process_names)
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
    runtime_state: &RuntimeState,
    context_root: &ContextRoot,
    palette_open: &mut bool,
    ui_context: &SharedUiContext,
) {
    let work_area = palette_work_area(context_root);
    let active_hwnd = context_root
        .get_active()
        .and_then(|handle| get_hwnd_from_raw(*handle))
        .map(|hwnd| hwnd.0 as isize);
    let registry_read = match runtime_state.registry.read() {
        Ok(registry) => registry,
        Err(err) => {
            log::error!("Extension registry lock poisoned: {err}");
            return;
        }
    };
    let mut commands = commands_from_unit_actions(
        registry_read.get_actions(context_root),
        registry_read.plugin_registry(),
        active_hwnd,
    );
    drop(registry_read);

    commands.push(reload_extensions_command(
        commands.len(),
        runtime_state.clone(),
    ));

    if ui_tx
        .send(UiSignal::Show {
            commands,
            work_area,
        })
        .is_ok()
    {
        *palette_open = true;
        request_ui_repaint(ui_context);
    }
}

fn request_ui_repaint(ui_context: &SharedUiContext) {
    if let Some(ctx) = ui_context.get() {
        ctx.request_repaint();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReloadReport {
    application_count: usize,
    ignored_process_count: usize,
}

fn reload_runtime_state(
    registry: &SharedRegistry,
    ignored_process_names: &SharedIgnoredProcessNames,
    bundled_extensions_root: &Path,
    current_os: Os,
) -> Result<ReloadReport, String> {
    let extension_discovery = ExtensionDiscovery::bundled_with_user_root(bundled_extensions_root);
    let new_registry = MasterRegistry::build_strict(&extension_discovery, current_os)
        .map_err(|err| err.to_string())?;
    let application_count = new_registry.application_registry.len();
    let new_ignored_process_names =
        load_ignored_process_names(&extension_discovery.ignore_file_path(), current_os);
    let ignored_process_count = new_ignored_process_names.len();

    {
        let mut registry = registry
            .write()
            .map_err(|err| format!("Extension registry lock poisoned: {err}"))?;
        *registry = new_registry;
    }

    {
        let mut ignored_process_names = ignored_process_names
            .write()
            .map_err(|err| format!("Ignored process registry lock poisoned: {err}"))?;
        *ignored_process_names = new_ignored_process_names;
    }

    Ok(ReloadReport {
        application_count,
        ignored_process_count,
    })
}

fn reload_extensions_command(original_order: usize, runtime_state: RuntimeState) -> Command {
    Command {
        label: "Omni Palette: Reload extensions".to_string(),
        shortcut_text: String::new(),
        priority: CommandPriority::Medium,
        focus_state: FocusState::Global,
        favorite: false,
        tags: vec!["extensions".to_string(), "reload".to_string()],
        original_order,
        action: Box::new(move || {
            match reload_runtime_state(
                &runtime_state.registry,
                &runtime_state.ignored_process_names,
                &runtime_state.bundled_extensions_root,
                runtime_state.current_os,
            ) {
                Ok(report) => log::info!(
                    "Reloaded extensions: {} applications, {} ignored processes",
                    report.application_count,
                    report.ignored_process_count
                ),
                Err(err) => log::error!("Failed to reload extensions: {err}"),
            }
        }),
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
    let shortcut_focus_target = shortcut_focus_target(target_hwnd_val, active_hwnd_val);

    Command {
        label: format!("{}: {}", unit_action.app_name, unit_action.action_name),
        shortcut_text: unit_action.shortcut_text,
        priority: unit_action.metadata.priority,
        focus_state: unit_action.focus_state,
        favorite: unit_action.metadata.favorite,
        tags: unit_action.metadata.tags,
        original_order,
        action: Box::new(move || match &execution {
            ActionExecution::Shortcut(shortcut) => {
                if let Some(val) = shortcut_focus_target {
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

fn shortcut_focus_target(
    target_hwnd_val: Option<isize>,
    active_hwnd_val: Option<isize>,
) -> Option<isize> {
    target_hwnd_val.or(active_hwnd_val)
}

#[cfg(debug_assertions)]
fn spawn_debug_telemetry(
    runtime_state: RuntimeState,
    ui_visibility: SharedUiVisibility,
) -> JoinHandle<()> {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));

        let (application_count, plugin_snapshot) = match runtime_state.registry.read() {
            Ok(registry) => (
                registry.application_registry.len(),
                registry.plugin_registry().execution_snapshot(),
            ),
            Err(err) => {
                log::error!("Could not collect telemetry; registry lock poisoned: {err}");
                continue;
            }
        };

        let ignored_process_count = match runtime_state.ignored_process_names.read() {
            Ok(ignored) => ignored.len(),
            Err(err) => {
                log::error!("Could not collect telemetry; ignored-process lock poisoned: {err}");
                continue;
            }
        };

        let palette_visible = ui_visibility.load(Ordering::Relaxed);
        let memory = debug_process_memory_bytes();
        let thread_count = debug_process_thread_count();

        log::debug!(
            "Runtime telemetry: visible={}, apps={}, plugins={}, plugin_apps={}, ignored_processes={}, plugin_started={}, plugin_completed={}, plugin_failed={}, plugin_timed_out={}, memory_private_bytes={:?}, thread_count={:?}",
            palette_visible,
            application_count,
            plugin_snapshot.loaded_plugins,
            plugin_snapshot.registered_applications,
            ignored_process_count,
            plugin_snapshot.started,
            plugin_snapshot.completed,
            plugin_snapshot.failed,
            plugin_snapshot.timed_out,
            memory,
            thread_count,
        );
    })
}

#[cfg(debug_assertions)]
fn debug_process_memory_bytes() -> Option<usize> {
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };

    unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .as_bool()
        .then_some(counters.WorkingSetSize)
    }
}

#[cfg(debug_assertions)]
fn debug_process_thread_count() -> Option<u32> {
    use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).ok()? };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut count = 0_u32;

    unsafe {
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == process_id {
                    count += 1;
                }

                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_with_entries(entries: Vec<CatalogEntry>) -> ExtensionCatalog {
        ExtensionCatalog {
            schema_version: 1,
            generated_at: None,
            expires_at_unix: None,
            entries,
        }
    }

    fn catalog_entry(id: &str, platform: Os, kind: ExtensionKind) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            platform,
            kind,
            package_url: format!(
                "https://github.com/Greg-Lim/Omni-Palette/releases/download/{id}-v1/{id}.gpext"
            ),
            package_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            size_bytes: None,
            publisher: None,
            description: None,
            license: None,
            homepage: None,
            repository: None,
            keywords: Vec::new(),
            min_app_version: None,
        }
    }

    #[test]
    fn finds_static_entry_for_current_platform() {
        let catalog = catalog_with_entries(vec![
            catalog_entry("downloaded_test", Os::Mac, ExtensionKind::Static),
            catalog_entry("downloaded_test", Os::Windows, ExtensionKind::Static),
        ]);

        let entry = find_install_entry(&catalog, "downloaded_test", Os::Windows)
            .expect("windows entry should be selected");

        assert_eq!(entry.platform, Os::Windows);
    }

    #[test]
    fn rejects_entry_without_current_platform_static_package() {
        let catalog = catalog_with_entries(vec![catalog_entry(
            "downloaded_test",
            Os::Mac,
            ExtensionKind::Static,
        )]);

        let err = find_install_entry(&catalog, "downloaded_test", Os::Windows)
            .expect_err("wrong platform should fail");

        assert!(err.contains("no static windows package"));
    }

    #[test]
    fn shortcut_focus_target_prefers_specific_target_window() {
        assert_eq!(shortcut_focus_target(Some(10), Some(20)), Some(10));
    }

    #[test]
    fn shortcut_focus_target_falls_back_to_active_window() {
        assert_eq!(shortcut_focus_target(None, Some(20)), Some(20));
    }

    #[test]
    fn shortcut_focus_target_allows_missing_windows() {
        assert_eq!(shortcut_focus_target(None, None), None);
    }
}
