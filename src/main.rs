use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    mpsc::{self, Receiver, RecvTimeoutError, Sender},
    Arc, OnceLock, RwLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use env_logger::Builder;

use crate::config::{
    ignore::{load_ignored_process_names, normalize_process_name},
    runtime::{RuntimeConfig, RuntimeConfigLoad, RuntimePaths},
};
use crate::core::extensions::{
    catalog::{CatalogEntry, ExtensionCatalog, ExtensionKind},
    discovery::{user_extensions_root, ExtensionDiscovery},
    extensions::load_config,
    install::{
        load_installed_state, set_bundled_extension_enabled, set_installed_extension_enabled,
        uninstall_installed_extension, BundledStaticExtension, ExtensionInstallService,
        InstalledState, BUNDLED_SOURCE_ID,
    },
};
use crate::core::performance::{current_process_private_bytes, current_process_thread_count};
use crate::core::plugins::PluginRegistry;
use crate::core::registry::registry::{MasterRegistry, UnitAction};
use crate::domain::action::{ActionExecution, CommandPriority, ContextRoot, FocusState, Os};
use crate::domain::hotkey::KeyboardShortcut;
use crate::platform::hotkey_actions::HotkeyPassthrough;
use crate::platform::platform_interface::{get_all_context, RawWindowHandleExt};
use crate::platform::windows::context::context::{
    focus_window, get_hwnd_from_raw, monitor_work_area_from_window,
};
use crate::platform::windows::sender::hotkey_sender::{send_shortcut, send_shortcut_sequence};
use crate::ui::app as ui_app;
use crate::ui::app::{
    Command, InstalledExtensionsUpdate, PaletteWorkArea, SharedUiContext, SharedUiVisibility,
    UiEvent, UiSignal,
};
use crate::ui::settings::SettingsBootstrap;
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
    let runtime_config_load = RuntimeConfig::load_with_diagnostics(
        runtime_paths.config_path.as_deref(),
        Path::new("./config.toml"),
    );
    let runtime_config = runtime_config_load.config.clone();
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
    let settings_bootstrap =
        settings_bootstrap(&runtime_paths, runtime_config_load.clone(), current_os);

    let (handle, rx) = platform::hotkey_actions::start_hotkey_listener(runtime_config.activation);
    let hotkey_passthrough = handle.passthrough_sender();
    let _hotkey_bridge = spawn_hotkey_bridge(
        rx,
        ui_tx,
        event_rx,
        runtime_state.clone(),
        hotkey_passthrough,
        runtime_config.activation,
        runtime_config,
        runtime_paths.config_path.clone(),
        Arc::clone(&ui_context),
    );
    #[cfg(debug_assertions)]
    let _telemetry_thread =
        spawn_debug_telemetry(runtime_state.clone(), Arc::clone(&ui_visibility));

    ui_app::run_with_shared_state(
        ui_rx,
        event_tx,
        ui_context,
        ui_visibility,
        settings_bootstrap,
    );

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

fn settings_bootstrap(
    runtime_paths: &RuntimePaths,
    runtime_config_load: RuntimeConfigLoad,
    current_os: Os,
) -> SettingsBootstrap {
    let install_root = user_extensions_root();
    let (installed_state, installed_state_error) = match install_root.as_deref() {
        Some(root) => match load_installed_state(root) {
            Ok(state) => (state, None),
            Err(err) => (InstalledState::default(), Some(err.to_string())),
        },
        None => (InstalledState::default(), None),
    };
    let bundled_static_extensions = bundled_static_extensions(
        Path::new(BUNDLED_EXTENSIONS_ROOT),
        current_os,
        &installed_state,
    );

    SettingsBootstrap {
        config: runtime_config_load.config,
        config_path: runtime_paths.config_path.clone(),
        config_error: runtime_config_load.user_config_error,
        current_os,
        install_root,
        bundled_static_extensions,
        installed_state,
        installed_state_error,
    }
}

fn bundled_static_extensions(
    bundled_root: &Path,
    current_os: Os,
    installed_state: &InstalledState,
) -> Vec<BundledStaticExtension> {
    let static_root = bundled_root.join("static");
    let entries = match std::fs::read_dir(&static_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            log::warn!(
                "Could not scan bundled static extensions at {:?}: {}",
                static_root,
                err
            );
            return Vec::new();
        }
    };

    let mut extensions = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                return None;
            }

            let config = match load_config(&path) {
                Ok(config) => config,
                Err(err) => {
                    log::warn!("Could not load bundled extension {:?}: {}", path, err);
                    return None;
                }
            };
            if config.platform != current_os {
                return None;
            }

            let enabled = installed_state
                .enabled_for(&config.app.id, BUNDLED_SOURCE_ID)
                .unwrap_or(true);

            Some(BundledStaticExtension {
                id: config.app.id,
                name: config.app.name,
                version: format!("schema {}", config.version),
                platform: config.platform,
                kind: ExtensionKind::Static,
                installed_path: path,
                enabled,
            })
        })
        .collect::<Vec<_>>();

    extensions.sort_by(|left, right| left.name.cmp(&right.name));
    extensions
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

fn print_extension_cli_usage() {
    println!("{}", extension_cli_usage());
}

fn extension_cli_usage() -> String {
    [
        "Usage:",
        "  cargo run -- ext catalog",
        "  cargo run -- ext install <extension_id>",
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
    mut activation_shortcut: KeyboardShortcut,
    mut runtime_config: RuntimeConfig,
    config_path: Option<PathBuf>,
    ui_context: SharedUiContext,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut palette_open = false;

        loop {
            handle_ui_events(
                &event_rx,
                &ui_tx,
                &runtime_state,
                &hotkey_passthrough,
                &mut activation_shortcut,
                &mut runtime_config,
                config_path.as_deref(),
                &mut palette_open,
                &ui_context,
            );

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

fn handle_ui_events(
    event_rx: &Receiver<UiEvent>,
    ui_tx: &Sender<UiSignal>,
    runtime_state: &RuntimeState,
    hotkey_passthrough: &HotkeyPassthrough,
    activation_shortcut: &mut KeyboardShortcut,
    runtime_config: &mut RuntimeConfig,
    config_path: Option<&Path>,
    palette_open: &mut bool,
    ui_context: &SharedUiContext,
) {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            UiEvent::Closed => *palette_open = false,
            UiEvent::ActionExecuted => {}
            UiEvent::OpenPaletteRequested => {
                let context_root = get_all_context();
                show_palette(
                    ui_tx,
                    runtime_state,
                    &context_root,
                    palette_open,
                    ui_context,
                );
            }
            UiEvent::SaveRuntimeConfigRequested(config) => {
                let result = save_runtime_config(
                    config.clone(),
                    runtime_config,
                    config_path,
                    hotkey_passthrough,
                    activation_shortcut,
                );
                send_ui_signal(
                    ui_tx,
                    ui_context,
                    UiSignal::RuntimeConfigSaved { config, result },
                );
            }
            UiEvent::RefreshCatalogRequested(source) => {
                spawn_catalog_refresh(ui_tx.clone(), Arc::clone(ui_context), source);
            }
            UiEvent::InstallExtensionRequested {
                source,
                entry,
                installed_version,
            } => {
                spawn_extension_install(
                    ui_tx.clone(),
                    Arc::clone(ui_context),
                    runtime_state.clone(),
                    source,
                    entry,
                    installed_version,
                );
            }
            UiEvent::UninstallExtensionRequested {
                extension_id,
                source_id,
                display_name,
            } => {
                spawn_extension_uninstall(
                    ui_tx.clone(),
                    Arc::clone(ui_context),
                    runtime_state.clone(),
                    extension_id,
                    source_id,
                    display_name,
                );
            }
            UiEvent::SetExtensionEnabledRequested {
                extension_id,
                source_id,
                display_name,
                enabled,
            } => {
                spawn_extension_enabled_update(
                    ui_tx.clone(),
                    Arc::clone(ui_context),
                    runtime_state.clone(),
                    extension_id,
                    source_id,
                    display_name,
                    enabled,
                );
            }
            UiEvent::SetBundledExtensionEnabledRequested { extension, enabled } => {
                spawn_bundled_extension_enabled_update(
                    ui_tx.clone(),
                    Arc::clone(ui_context),
                    runtime_state.clone(),
                    extension,
                    enabled,
                );
            }
            UiEvent::ReloadExtensionsRequested => {
                let result = reload_runtime_state(
                    &runtime_state.registry,
                    &runtime_state.ignored_process_names,
                    &runtime_state.bundled_extensions_root,
                    runtime_state.current_os,
                )
                .map(|report| {
                    format!(
                        "Reloaded extensions: {} applications, {} ignored processes",
                        report.application_count, report.ignored_process_count
                    )
                });
                send_ui_signal(
                    ui_tx,
                    ui_context,
                    UiSignal::ReloadExtensionsFinished(result),
                );
            }
            UiEvent::QuitRequested => {
                send_ui_signal(ui_tx, ui_context, UiSignal::Quit);
            }
        }
    }
}

fn save_runtime_config(
    requested_config: RuntimeConfig,
    runtime_config: &mut RuntimeConfig,
    config_path: Option<&Path>,
    hotkey_passthrough: &HotkeyPassthrough,
    activation_shortcut: &mut KeyboardShortcut,
) -> Result<String, String> {
    let path = config_path.ok_or_else(|| {
        "APPDATA is not set, so Omni Palette cannot save user settings.".to_string()
    })?;
    let old_config = runtime_config.clone();
    let hotkey_changed = requested_config.activation != old_config.activation;

    if hotkey_changed {
        hotkey_passthrough.update_shortcut(requested_config.activation)?;
        *activation_shortcut = requested_config.activation;
    }

    if let Err(err) = requested_config.save_user_config(path) {
        if hotkey_changed {
            match hotkey_passthrough.update_shortcut(old_config.activation) {
                Ok(()) => *activation_shortcut = old_config.activation,
                Err(rollback_err) => {
                    return Err(format!(
                        "{err}; also failed to restore the previous hotkey: {rollback_err}"
                    ));
                }
            }
        }
        return Err(err);
    }

    *runtime_config = requested_config;
    Ok("Settings saved".to_string())
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

fn send_ui_signal(ui_tx: &Sender<UiSignal>, ui_context: &SharedUiContext, signal: UiSignal) {
    if ui_tx.send(signal).is_ok() {
        request_ui_repaint(ui_context);
    }
}

fn spawn_catalog_refresh(
    ui_tx: Sender<UiSignal>,
    ui_context: SharedUiContext,
    source: crate::config::runtime::GitHubExtensionSource,
) {
    std::thread::spawn(move || {
        let result = (|| {
            let service = ExtensionInstallService::new(extension_install_root()?);
            service
                .fetch_catalog(&source)
                .map_err(|err| err.to_string())
        })();
        send_ui_signal(&ui_tx, &ui_context, UiSignal::CatalogRefreshed(result));
    });
}

fn spawn_extension_install(
    ui_tx: Sender<UiSignal>,
    ui_context: SharedUiContext,
    runtime_state: RuntimeState,
    source: crate::config::runtime::GitHubExtensionSource,
    entry: CatalogEntry,
    installed_version: Option<String>,
) {
    std::thread::spawn(move || {
        let result = (|| {
            let install_root = extension_install_root()?;
            let service = ExtensionInstallService::new(&install_root);
            let installed_extension = service
                .install_entry(&source, &entry, runtime_state.current_os)
                .map_err(|err| err.to_string())?;
            reload_runtime_state(
                &runtime_state.registry,
                &runtime_state.ignored_process_names,
                &runtime_state.bundled_extensions_root,
                runtime_state.current_os,
            )?;
            let state = load_installed_state(&install_root).map_err(|err| err.to_string())?;
            Ok(InstalledExtensionsUpdate {
                state,
                message: extension_install_message(
                    &entry.name,
                    installed_version.as_deref(),
                    &installed_extension.version,
                ),
            })
        })();
        send_ui_signal(
            &ui_tx,
            &ui_context,
            UiSignal::InstalledExtensionsUpdated(result),
        );
    });
}

fn spawn_extension_enabled_update(
    ui_tx: Sender<UiSignal>,
    ui_context: SharedUiContext,
    runtime_state: RuntimeState,
    extension_id: String,
    source_id: String,
    display_name: String,
    enabled: bool,
) {
    std::thread::spawn(move || {
        let result = (|| {
            let install_root = extension_install_root()?;
            let state =
                set_installed_extension_enabled(&install_root, &extension_id, &source_id, enabled)
                    .map_err(|err| err.to_string())?;
            reload_runtime_state(
                &runtime_state.registry,
                &runtime_state.ignored_process_names,
                &runtime_state.bundled_extensions_root,
                runtime_state.current_os,
            )?;
            Ok(InstalledExtensionsUpdate {
                state,
                message: extension_enabled_message(&display_name, enabled),
            })
        })();
        send_ui_signal(
            &ui_tx,
            &ui_context,
            UiSignal::InstalledExtensionsUpdated(result),
        );
    });
}

fn spawn_extension_uninstall(
    ui_tx: Sender<UiSignal>,
    ui_context: SharedUiContext,
    runtime_state: RuntimeState,
    extension_id: String,
    source_id: String,
    display_name: String,
) {
    std::thread::spawn(move || {
        let result = (|| {
            let install_root = extension_install_root()?;
            let state = uninstall_installed_extension(&install_root, &extension_id, &source_id)
                .map_err(|err| err.to_string())?;
            reload_runtime_state(
                &runtime_state.registry,
                &runtime_state.ignored_process_names,
                &runtime_state.bundled_extensions_root,
                runtime_state.current_os,
            )?;
            Ok(InstalledExtensionsUpdate {
                state,
                message: format!("Uninstalled {display_name}"),
            })
        })();
        send_ui_signal(
            &ui_tx,
            &ui_context,
            UiSignal::InstalledExtensionsUpdated(result),
        );
    });
}

fn spawn_bundled_extension_enabled_update(
    ui_tx: Sender<UiSignal>,
    ui_context: SharedUiContext,
    runtime_state: RuntimeState,
    extension: BundledStaticExtension,
    enabled: bool,
) {
    std::thread::spawn(move || {
        let result = (|| {
            let install_root = extension_install_root()?;
            let state = set_bundled_extension_enabled(&install_root, &extension, enabled)
                .map_err(|err| err.to_string())?;
            reload_runtime_state(
                &runtime_state.registry,
                &runtime_state.ignored_process_names,
                &runtime_state.bundled_extensions_root,
                runtime_state.current_os,
            )?;
            Ok(InstalledExtensionsUpdate {
                state,
                message: extension_enabled_message(&extension.name, enabled),
            })
        })();
        send_ui_signal(
            &ui_tx,
            &ui_context,
            UiSignal::InstalledExtensionsUpdated(result),
        );
    });
}

fn extension_enabled_message(display_name: &str, enabled: bool) -> String {
    let action = if enabled { "Enabled" } else { "Disabled" };
    format!("{action} {display_name}")
}

fn extension_install_message(
    display_name: &str,
    previous_version: Option<&str>,
    installed_version: &str,
) -> String {
    match previous_version {
        Some(previous_version) if previous_version == installed_version => {
            format!("Reinstalled {display_name} v{installed_version}")
        }
        Some(previous_version) => {
            format!("Updated {display_name} from v{previous_version} to v{installed_version}")
        }
        None => format!("Installed {display_name} v{installed_version}"),
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
            ActionExecution::ShortcutSequence(sequence) => {
                if let Some(val) = shortcut_focus_target {
                    focus_window(HWND(val as *mut _));
                }
                send_shortcut_sequence(sequence);
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
        let memory = current_process_private_bytes();
        let thread_count = current_process_thread_count();

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
                "https://github.com/Greg-Lim/omni-palette-desktop/releases/download/{id}-v1/{id}.gpext"
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
            catalog_entry("chrome", Os::Mac, ExtensionKind::Static),
            catalog_entry("chrome", Os::Windows, ExtensionKind::Static),
        ]);

        let entry = find_install_entry(&catalog, "chrome", Os::Windows)
            .expect("windows entry should be selected");

        assert_eq!(entry.platform, Os::Windows);
    }

    #[test]
    fn rejects_entry_without_current_platform_static_package() {
        let catalog = catalog_with_entries(vec![catalog_entry(
            "chrome",
            Os::Mac,
            ExtensionKind::Static,
        )]);

        let err = find_install_entry(&catalog, "chrome", Os::Windows)
            .expect_err("wrong platform should fail");

        assert!(err.contains("no static windows package"));
    }

    #[test]
    fn extension_enabled_message_names_enabled_extension() {
        assert_eq!(extension_enabled_message("Chrome", true), "Enabled Chrome");
    }

    #[test]
    fn extension_enabled_message_names_disabled_extension() {
        assert_eq!(
            extension_enabled_message("Windows", false),
            "Disabled Windows"
        );
    }

    #[test]
    fn extension_install_message_distinguishes_install_reinstall_and_update() {
        assert_eq!(
            extension_install_message("File Explorer", None, "0.1.0"),
            "Installed File Explorer v0.1.0"
        );
        assert_eq!(
            extension_install_message("Chrome", Some("0.1.0"), "0.1.0"),
            "Reinstalled Chrome v0.1.0"
        );
        assert_eq!(
            extension_install_message("Chrome", Some("0.1.0"), "0.2.0"),
            "Updated Chrome from v0.1.0 to v0.2.0"
        );
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
