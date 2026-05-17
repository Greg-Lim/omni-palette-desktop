mod guide_lifecycle;
mod hotkey_bridge;
mod settings_window;
mod window_lifecycle;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use omni_palette::{
    backend_contract::{
        CommandExecutionResultDto, CommandId, PaletteBackend, PaletteBootstrapDto,
        PaletteSnapshotDto,
    },
    config::runtime::{CommandBehavior, GitHubExtensionSource, RuntimeConfig, ThemeMode},
    core::{
        extensions::settings::extension_settings_key,
        extensions::{
            catalog::{CatalogEntry, ExtensionCatalog, ExtensionKind},
            install::{
                ExtensionInstallService, InstalledExtension, BUNDLED_SOURCE_ID, GITHUB_SOURCE_ID,
            },
        },
    },
    domain::{
        action::Os,
        hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
    },
    extension_management::{
        extension_management_snapshot, set_extension_enabled as set_runtime_extension_enabled,
        uninstall_extension as uninstall_runtime_extension, ExtensionManagementSnapshot,
    },
    runtime_state::{OmniRuntimeState, RuntimeStateLoadOptions, RuntimeStatusDto},
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::guide_lifecycle::{GuideLifecycle, GuideRuntimeCommand, GuideStatusDto, GUIDE_DURATION};
use crate::hotkey_bridge::{HotkeyBridge, HotkeyStatusDto, PaletteActivationHandler};
use crate::settings_window::{SettingsWindow, SettingsWindowStatusDto};
use crate::window_lifecycle::{WindowLifecycle, WindowLifecycleStatusDto};

struct AppState {
    backend: Arc<PaletteBackend>,
    runtime_state: OmniRuntimeState,
    hotkey_bridge: Arc<HotkeyBridge>,
    window_lifecycle: Arc<WindowLifecycle>,
    settings_window: Arc<SettingsWindow>,
    guide_lifecycle: Arc<GuideLifecycle>,
    marketplace: Arc<MarketplaceState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthCheckPayload {
    pub app_name: &'static str,
    pub phase: &'static str,
    pub status: &'static str,
}

#[tauri::command]
fn health_check() -> HealthCheckPayload {
    HealthCheckPayload {
        app_name: "Omni Palette",
        phase: "Phase 6C.2 - Marketplace Catalog Refresh And Install",
        status: "ok",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubExtensionSourceDto {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub catalog_path: String,
    pub enabled: bool,
}

impl From<GitHubExtensionSource> for GitHubExtensionSourceDto {
    fn from(source: GitHubExtensionSource) -> Self {
        Self {
            owner: source.owner,
            repo: source.repo,
            branch: source.branch,
            catalog_path: source.catalog_path,
            enabled: source.enabled,
        }
    }
}

impl From<GitHubExtensionSourceDto> for GitHubExtensionSource {
    fn from(source: GitHubExtensionSourceDto) -> Self {
        Self {
            owner: source.owner,
            repo: source.repo,
            branch: source.branch,
            catalog_path: source.catalog_path,
            enabled: source.enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationShortcutDto {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub key: Key,
    pub display_text: String,
}

impl From<KeyboardShortcut> for ActivationShortcutDto {
    fn from(shortcut: KeyboardShortcut) -> Self {
        Self {
            control: shortcut.modifier.control,
            shift: shortcut.modifier.shift,
            alt: shortcut.modifier.alt,
            win: shortcut.modifier.win,
            key: shortcut.key,
            display_text: shortcut.to_string(),
        }
    }
}

impl ActivationShortcutDto {
    fn to_shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: self.control,
                shift: self.shift,
                alt: self.alt,
                win: self.win,
            },
            key: self.key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSettingsDto {
    pub activation_hint: String,
    pub activation_shortcut: ActivationShortcutDto,
    pub command_behavior: CommandBehavior,
    pub appearance_theme: ThemeMode,
    pub github: GitHubExtensionSourceDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsBootstrapDto {
    pub config: RuntimeSettingsDto,
    pub default_activation_shortcut: ActivationShortcutDto,
    pub config_path: Option<String>,
    pub config_error: Option<String>,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSettingsSaveRequestDto {
    pub activation_shortcut: ActivationShortcutDto,
    pub command_behavior: CommandBehavior,
    pub appearance_theme: ThemeMode,
    pub github: GitHubExtensionSourceDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSettingsResultStatusDto {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSettingsSaveResultDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub config: RuntimeSettingsDto,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReloadResultDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntryDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub platform: Os,
    pub kind: ExtensionKindDto,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRefreshResultDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub entries: Vec<CatalogEntryDto>,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKindDto {
    Static,
    WasmPlugin,
}

impl From<ExtensionKind> for ExtensionKindDto {
    fn from(kind: ExtensionKind) -> Self {
        match kind {
            ExtensionKind::Static => Self::Static,
            ExtensionKind::WasmPlugin => Self::WasmPlugin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRowDto {
    pub id: String,
    pub source_id: String,
    pub name: String,
    pub version: String,
    pub kind: ExtensionKindDto,
    pub enabled: bool,
    pub can_uninstall: bool,
    pub has_settings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionsBootstrapDto {
    pub bundled_extensions: Vec<ExtensionRowDto>,
    pub downloaded_extensions: Vec<ExtensionRowDto>,
    pub install_root: Option<String>,
    pub install_root_error: Option<String>,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionEnabledRequestDto {
    pub extension_id: String,
    pub source_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionTargetRequestDto {
    pub extension_id: String,
    pub source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionMutationResultDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub extensions: ExtensionsBootstrapDto,
    pub runtime_status: RuntimeStatusDto,
}

trait MarketplaceService: Send + Sync {
    fn fetch_catalog(
        &self,
        install_root: &Path,
        source: &GitHubExtensionSource,
    ) -> Result<ExtensionCatalog, String>;

    fn install_entry(
        &self,
        install_root: &Path,
        source: &GitHubExtensionSource,
        entry: &CatalogEntry,
        current_os: Os,
    ) -> Result<InstalledExtension, String>;
}

struct ExtensionInstallMarketplaceService;

impl MarketplaceService for ExtensionInstallMarketplaceService {
    fn fetch_catalog(
        &self,
        install_root: &Path,
        source: &GitHubExtensionSource,
    ) -> Result<ExtensionCatalog, String> {
        ExtensionInstallService::new(install_root)
            .fetch_catalog(source)
            .map_err(|err| err.to_string())
    }

    fn install_entry(
        &self,
        install_root: &Path,
        source: &GitHubExtensionSource,
        entry: &CatalogEntry,
        current_os: Os,
    ) -> Result<InstalledExtension, String> {
        ExtensionInstallService::new(install_root)
            .install_entry(source, entry, current_os)
            .map_err(|err| err.to_string())
    }
}

#[derive(Clone)]
struct CachedCatalog {
    catalog: ExtensionCatalog,
    source: GitHubExtensionSource,
}

struct MarketplaceState {
    service: Arc<dyn MarketplaceService>,
    cache: Mutex<Option<CachedCatalog>>,
}

impl MarketplaceState {
    fn new(service: Arc<dyn MarketplaceService>) -> Self {
        Self {
            service,
            cache: Mutex::new(None),
        }
    }

    fn cached_catalog(&self) -> Option<CachedCatalog> {
        self.cache
            .lock()
            .expect("catalog cache should lock")
            .clone()
    }

    fn set_cached_catalog(&self, catalog: ExtensionCatalog, source: GitHubExtensionSource) {
        *self.cache.lock().expect("catalog cache should lock") =
            Some(CachedCatalog { catalog, source });
    }
}

pub trait ActivationShortcutUpdater: Send + Sync {
    fn update_activation_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String>;
}

impl ActivationShortcutUpdater for HotkeyBridge {
    fn update_activation_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
        HotkeyBridge::update_activation_shortcut(self, shortcut)
    }
}

fn runtime_settings_from_config(config: RuntimeConfig) -> RuntimeSettingsDto {
    RuntimeSettingsDto {
        activation_hint: config.activation.to_string(),
        activation_shortcut: ActivationShortcutDto::from(config.activation),
        command_behavior: config.command_behavior,
        appearance_theme: config.appearance.theme,
        github: GitHubExtensionSourceDto::from(config.github),
    }
}

fn settings_bootstrap_from_runtime(runtime: &OmniRuntimeState) -> SettingsBootstrapDto {
    let config_load = runtime.config_load();
    SettingsBootstrapDto {
        config: runtime_settings_from_config(config_load.config),
        default_activation_shortcut: ActivationShortcutDto::from(
            RuntimeConfig::default_activation_shortcut(),
        ),
        config_path: runtime
            .config_path()
            .as_ref()
            .map(|path| path.display().to_string()),
        config_error: config_load.user_config_error,
        runtime_status: runtime.status(),
    }
}

fn save_runtime_settings_for_runtime(
    runtime: &OmniRuntimeState,
    activation_updater: &dyn ActivationShortcutUpdater,
    request: RuntimeSettingsSaveRequestDto,
) -> RuntimeSettingsSaveResultDto {
    let previous_config = runtime.config();
    let previous_activation = previous_config.activation;
    let next_activation = request.activation_shortcut.to_shortcut();
    let activation_changed = previous_activation != next_activation;
    let mut next_config = previous_config.clone();
    next_config.activation = next_activation;
    next_config.command_behavior = request.command_behavior;
    next_config.appearance.theme = request.appearance_theme;
    next_config.github = request.github.into();

    if activation_changed {
        if let Err(err) = activation_updater.update_activation_shortcut(next_activation) {
            return RuntimeSettingsSaveResultDto {
                status: RuntimeSettingsResultStatusDto::Failed,
                message: format!("Could not update activation shortcut: {err}"),
                config: runtime_settings_from_config(runtime.config()),
                runtime_status: runtime.status(),
            };
        }
    }

    match runtime.save_runtime_config(next_config) {
        Ok(message) => RuntimeSettingsSaveResultDto {
            status: RuntimeSettingsResultStatusDto::Succeeded,
            message,
            config: runtime_settings_from_config(runtime.config()),
            runtime_status: runtime.status(),
        },
        Err(message) => {
            if activation_changed {
                let rollback_result =
                    activation_updater.update_activation_shortcut(previous_activation);
                if let Err(rollback_err) = rollback_result {
                    return RuntimeSettingsSaveResultDto {
                        status: RuntimeSettingsResultStatusDto::Failed,
                        message: format!(
                            "{message}; additionally failed to restore previous activation shortcut: {rollback_err}"
                        ),
                        config: runtime_settings_from_config(runtime.config()),
                        runtime_status: runtime.status(),
                    };
                }
            }
            RuntimeSettingsSaveResultDto {
                status: RuntimeSettingsResultStatusDto::Failed,
                message,
                config: runtime_settings_from_config(runtime.config()),
                runtime_status: runtime.status(),
            }
        }
    }
}

fn reload_runtime_state_for_runtime(runtime: &OmniRuntimeState) -> RuntimeReloadResultDto {
    match runtime.reload_extensions() {
        Ok(report) => RuntimeReloadResultDto {
            status: RuntimeSettingsResultStatusDto::Succeeded,
            message: format!(
                "Reloaded extensions: {} applications, {} ignored processes, {} plugins",
                report.application_count, report.ignored_process_count, report.plugin_count
            ),
            runtime_status: runtime.status(),
        },
        Err(message) => RuntimeReloadResultDto {
            status: RuntimeSettingsResultStatusDto::Failed,
            message,
            runtime_status: runtime.status(),
        },
    }
}

fn catalog_entry_to_dto(entry: &CatalogEntry) -> CatalogEntryDto {
    CatalogEntryDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        platform: entry.platform,
        kind: ExtensionKindDto::from(entry.kind),
        description: entry.description.clone(),
        keywords: entry.keywords.clone(),
    }
}

fn catalog_entries_for_os(catalog: &ExtensionCatalog, current_os: Os) -> Vec<CatalogEntryDto> {
    let mut entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.platform == current_os)
        .map(catalog_entry_to_dto)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    entries
}

fn cached_catalog_entries(marketplace: &MarketplaceState, current_os: Os) -> Vec<CatalogEntryDto> {
    marketplace
        .cached_catalog()
        .map(|cached| catalog_entries_for_os(&cached.catalog, current_os))
        .unwrap_or_default()
}

fn refresh_extension_catalog_for_runtime(
    runtime: &OmniRuntimeState,
    marketplace: &MarketplaceState,
    source: GitHubExtensionSourceDto,
) -> CatalogRefreshResultDto {
    let source: GitHubExtensionSource = source.into();
    let install_root = match runtime.user_extensions_root() {
        Some(root) => root,
        None => {
            return CatalogRefreshResultDto {
                status: RuntimeSettingsResultStatusDto::Failed,
                message: "APPDATA is not set, so Omni Palette cannot refresh extension catalogs."
                    .to_string(),
                entries: cached_catalog_entries(marketplace, runtime.current_os()),
                runtime_status: runtime.status(),
            };
        }
    };

    match marketplace.service.fetch_catalog(&install_root, &source) {
        Ok(catalog) => {
            let entries = catalog_entries_for_os(&catalog, runtime.current_os());
            marketplace.set_cached_catalog(catalog, source);
            CatalogRefreshResultDto {
                status: RuntimeSettingsResultStatusDto::Succeeded,
                message: format!(
                    "Catalog refreshed: {} {} available",
                    entries.len(),
                    if entries.len() == 1 {
                        "extension"
                    } else {
                        "extensions"
                    }
                ),
                entries,
                runtime_status: runtime.status(),
            }
        }
        Err(message) => CatalogRefreshResultDto {
            status: RuntimeSettingsResultStatusDto::Failed,
            message,
            entries: cached_catalog_entries(marketplace, runtime.current_os()),
            runtime_status: runtime.status(),
        },
    }
}

fn extensions_bootstrap_from_runtime(runtime: &OmniRuntimeState) -> ExtensionsBootstrapDto {
    extensions_bootstrap_from_snapshot(runtime, extension_management_snapshot(runtime))
}

fn extensions_bootstrap_from_snapshot(
    runtime: &OmniRuntimeState,
    snapshot: ExtensionManagementSnapshot,
) -> ExtensionsBootstrapDto {
    let settings_available = &snapshot.extension_settings_available;
    let bundled_extensions = snapshot
        .bundled_extensions
        .iter()
        .map(|extension| ExtensionRowDto {
            id: extension.id.clone(),
            source_id: BUNDLED_SOURCE_ID.to_string(),
            name: extension.name.clone(),
            version: extension.version.clone(),
            kind: ExtensionKindDto::from(extension.kind),
            enabled: extension.enabled,
            can_uninstall: false,
            has_settings: settings_available
                .contains(&extension_settings_key(&extension.id, BUNDLED_SOURCE_ID)),
        })
        .collect();
    let downloaded_extensions = snapshot
        .installed_state
        .extensions
        .iter()
        .filter(|extension| extension.source_id != BUNDLED_SOURCE_ID)
        .map(|extension| ExtensionRowDto {
            id: extension.id.clone(),
            source_id: extension.source_id.clone(),
            name: extension.id.clone(),
            version: extension.version.clone(),
            kind: ExtensionKindDto::from(extension.kind),
            enabled: extension.enabled,
            can_uninstall: true,
            has_settings: settings_available
                .contains(&extension_settings_key(&extension.id, &extension.source_id)),
        })
        .collect();

    ExtensionsBootstrapDto {
        bundled_extensions,
        downloaded_extensions,
        install_root: snapshot
            .install_root
            .as_ref()
            .map(|path| path.display().to_string()),
        install_root_error: snapshot.install_root_error,
        runtime_status: runtime.status(),
    }
}

fn set_extension_enabled_for_runtime(
    runtime: &OmniRuntimeState,
    request: ExtensionEnabledRequestDto,
) -> ExtensionMutationResultDto {
    match set_runtime_extension_enabled(
        runtime,
        &request.extension_id,
        &request.source_id,
        request.enabled,
    ) {
        Ok((snapshot, message)) => {
            let extensions = extensions_bootstrap_from_snapshot(runtime, snapshot);
            ExtensionMutationResultDto {
                status: RuntimeSettingsResultStatusDto::Succeeded,
                message,
                runtime_status: runtime.status(),
                extensions,
            }
        }
        Err(message) => ExtensionMutationResultDto {
            status: RuntimeSettingsResultStatusDto::Failed,
            message,
            extensions: extensions_bootstrap_from_runtime(runtime),
            runtime_status: runtime.status(),
        },
    }
}

fn extension_mutation_failure(
    runtime: &OmniRuntimeState,
    message: String,
) -> ExtensionMutationResultDto {
    ExtensionMutationResultDto {
        status: RuntimeSettingsResultStatusDto::Failed,
        message,
        extensions: extensions_bootstrap_from_runtime(runtime),
        runtime_status: runtime.status(),
    }
}

fn uninstall_extension_for_runtime(
    runtime: &OmniRuntimeState,
    request: ExtensionTargetRequestDto,
) -> ExtensionMutationResultDto {
    match uninstall_runtime_extension(runtime, &request.extension_id, &request.source_id) {
        Ok((snapshot, message)) => {
            let extensions = extensions_bootstrap_from_snapshot(runtime, snapshot);
            ExtensionMutationResultDto {
                status: RuntimeSettingsResultStatusDto::Succeeded,
                message,
                runtime_status: runtime.status(),
                extensions,
            }
        }
        Err(message) => ExtensionMutationResultDto {
            status: RuntimeSettingsResultStatusDto::Failed,
            message,
            extensions: extensions_bootstrap_from_runtime(runtime),
            runtime_status: runtime.status(),
        },
    }
}

fn install_catalog_extension_for_runtime(
    runtime: &OmniRuntimeState,
    marketplace: &MarketplaceState,
    extension_id: &str,
) -> ExtensionMutationResultDto {
    let Some(cached) = marketplace.cached_catalog() else {
        return extension_mutation_failure(
            runtime,
            "Refresh the extension catalog before installing extensions.".to_string(),
        );
    };

    let Some(entry) = cached
        .catalog
        .entries
        .iter()
        .find(|entry| entry.id == extension_id && entry.platform == runtime.current_os())
        .cloned()
    else {
        return extension_mutation_failure(
            runtime,
            format!("Catalog extension not found: {extension_id}"),
        );
    };

    if entry.kind != ExtensionKind::Static {
        return extension_mutation_failure(
            runtime,
            "Only static catalog extensions can be installed in Phase 6C.2.".to_string(),
        );
    }

    let install_root = match runtime.user_extensions_root() {
        Some(root) => root,
        None => {
            return extension_mutation_failure(
                runtime,
                "APPDATA is not set, so Omni Palette cannot install user extensions.".to_string(),
            );
        }
    };
    let previous_version = extension_management_snapshot(runtime)
        .installed_state
        .extensions
        .iter()
        .find(|extension| extension.id == entry.id && extension.source_id == GITHUB_SOURCE_ID)
        .map(|extension| extension.version.clone());

    match marketplace.service.install_entry(
        &install_root,
        &cached.source,
        &entry,
        runtime.current_os(),
    ) {
        Ok(installed) => match runtime.reload_extensions() {
            Ok(_) => {
                let extensions = extensions_bootstrap_from_runtime(runtime);
                ExtensionMutationResultDto {
                    status: RuntimeSettingsResultStatusDto::Succeeded,
                    message: extension_install_message(
                        &entry.name,
                        previous_version.as_deref(),
                        &installed.version,
                    ),
                    runtime_status: runtime.status(),
                    extensions,
                }
            }
            Err(message) => extension_mutation_failure(runtime, message),
        },
        Err(message) => extension_mutation_failure(runtime, message),
    }
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

#[tauri::command]
fn get_palette_bootstrap(state: State<'_, AppState>) -> PaletteBootstrapDto {
    state.backend.get_palette_bootstrap()
}

#[tauri::command]
fn search_commands(query: String, state: State<'_, AppState>) -> PaletteSnapshotDto {
    state.backend.search_commands(&query)
}

#[tauri::command]
fn execute_command(command_id: String, state: State<'_, AppState>) -> CommandExecutionResultDto {
    state.backend.execute_command(&CommandId::new(command_id))
}

#[tauri::command]
fn get_hotkey_status(state: State<'_, AppState>) -> HotkeyStatusDto {
    state.hotkey_bridge.status()
}

#[tauri::command]
fn get_window_lifecycle_status(state: State<'_, AppState>) -> WindowLifecycleStatusDto {
    state.window_lifecycle.status()
}

#[tauri::command]
fn hide_palette_window(state: State<'_, AppState>) -> WindowLifecycleStatusDto {
    state.window_lifecycle.hide_palette_window()
}

#[tauri::command]
fn start_guide(command_id: String, state: State<'_, AppState>) -> GuideStatusDto {
    let command = match state.backend.guide_command(&CommandId::new(command_id)) {
        Ok(command) => command,
        Err(err) => return state.guide_lifecycle.record_start_error(err),
    };
    let command: Arc<dyn GuideRuntimeCommand> = Arc::new(command);
    let captured_shortcut = command.captured_shortcut();

    let status = state.guide_lifecycle.start(command);
    if status.active {
        if let Err(err) = state.hotkey_bridge.enable_guide_hotkeys(captured_shortcut) {
            return state.guide_lifecycle.record_start_error(err);
        }

        if let Some(generation) = state.guide_lifecycle.active_generation() {
            let guide_lifecycle = Arc::clone(&state.guide_lifecycle);
            let hotkey_bridge = Arc::clone(&state.hotkey_bridge);
            thread::spawn(move || {
                thread::sleep(GUIDE_DURATION);
                if guide_lifecycle.expire_generation(generation) {
                    let _ = hotkey_bridge.clear_guide_hotkeys();
                }
            });
        }
    }
    status
}

#[tauri::command]
fn cancel_guide(state: State<'_, AppState>) -> GuideStatusDto {
    if state.guide_lifecycle.cancel_active() {
        let _ = state.hotkey_bridge.clear_guide_hotkeys();
    }
    state.guide_lifecycle.status()
}

#[tauri::command]
fn get_guide_status(state: State<'_, AppState>) -> GuideStatusDto {
    state.guide_lifecycle.status()
}

#[tauri::command]
fn get_settings_bootstrap(state: State<'_, AppState>) -> SettingsBootstrapDto {
    settings_bootstrap_from_runtime(&state.runtime_state)
}

#[tauri::command]
fn save_runtime_settings(
    request: RuntimeSettingsSaveRequestDto,
    state: State<'_, AppState>,
) -> RuntimeSettingsSaveResultDto {
    save_runtime_settings_for_runtime(&state.runtime_state, state.hotkey_bridge.as_ref(), request)
}

#[tauri::command]
fn reload_runtime_state(state: State<'_, AppState>) -> RuntimeReloadResultDto {
    reload_runtime_state_for_runtime(&state.runtime_state)
}

#[tauri::command]
fn show_settings_window(state: State<'_, AppState>) -> SettingsWindowStatusDto {
    state.settings_window.show_settings_window()
}

#[tauri::command]
fn get_extensions_bootstrap(state: State<'_, AppState>) -> ExtensionsBootstrapDto {
    extensions_bootstrap_from_runtime(&state.runtime_state)
}

#[tauri::command]
fn set_extension_enabled(
    request: ExtensionEnabledRequestDto,
    state: State<'_, AppState>,
) -> ExtensionMutationResultDto {
    set_extension_enabled_for_runtime(&state.runtime_state, request)
}

#[tauri::command]
fn uninstall_extension(
    request: ExtensionTargetRequestDto,
    state: State<'_, AppState>,
) -> ExtensionMutationResultDto {
    uninstall_extension_for_runtime(&state.runtime_state, request)
}

#[tauri::command]
fn refresh_extension_catalog(
    source: GitHubExtensionSourceDto,
    state: State<'_, AppState>,
) -> CatalogRefreshResultDto {
    refresh_extension_catalog_for_runtime(&state.runtime_state, &state.marketplace, source)
}

#[tauri::command]
fn install_catalog_extension(
    extension_id: String,
    state: State<'_, AppState>,
) -> ExtensionMutationResultDto {
    install_catalog_extension_for_runtime(&state.runtime_state, &state.marketplace, &extension_id)
}

struct ActivationRouter {
    window_lifecycle: Arc<WindowLifecycle>,
    guide_lifecycle: Arc<GuideLifecycle>,
}

impl PaletteActivationHandler for ActivationRouter {
    fn handle_palette_activation(&self, context: omni_palette::domain::action::ContextRoot) {
        self.window_lifecycle.handle_activation(context);
    }

    fn handle_guide_activation(&self) -> bool {
        self.guide_lifecycle.complete_active().is_some()
    }

    fn handle_guide_cancel(
        &self,
        _shortcut: omni_palette::domain::hotkey::KeyboardShortcut,
    ) -> bool {
        self.guide_lifecycle.cancel_active()
    }

    fn handle_guide_shortcut(
        &self,
        shortcut: omni_palette::domain::hotkey::KeyboardShortcut,
    ) -> bool {
        if self.guide_lifecycle.captured_shortcut() == Some(shortcut) {
            return self.guide_lifecycle.cancel_active();
        }
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bundled_extensions_root = bundled_extensions_root();
    let runtime_state = OmniRuntimeState::load(RuntimeStateLoadOptions::from_environment(
        bundled_extensions_root,
        Os::Windows,
    ));
    let backend = Arc::new(PaletteBackend::from_runtime_state(runtime_state.clone()));

    tauri::Builder::default()
        .setup(move |app| {
            let window_lifecycle = Arc::new(WindowLifecycle::for_tauri(
                Arc::clone(&backend),
                app.handle().clone(),
            ));
            let settings_window = Arc::new(SettingsWindow::for_tauri(app.handle().clone()));
            let marketplace = Arc::new(MarketplaceState::new(Arc::new(
                ExtensionInstallMarketplaceService,
            )));
            let guide_lifecycle = Arc::new(GuideLifecycle::for_tauri(
                runtime_state.config().activation.to_string(),
                Arc::clone(&window_lifecycle),
                app.handle().clone(),
            ));
            let activation_handler: Arc<dyn hotkey_bridge::PaletteActivationHandler> =
                Arc::new(ActivationRouter {
                    window_lifecycle: Arc::clone(&window_lifecycle),
                    guide_lifecycle: Arc::clone(&guide_lifecycle),
                });
            let hotkey_bridge = Arc::new(HotkeyBridge::start(
                runtime_state.clone(),
                app.handle().clone(),
                activation_handler,
            ));
            app.manage(AppState {
                backend: Arc::clone(&backend),
                runtime_state: runtime_state.clone(),
                hotkey_bridge,
                window_lifecycle,
                settings_window,
                guide_lifecycle,
                marketplace,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_palette_bootstrap,
            search_commands,
            execute_command,
            get_hotkey_status,
            get_window_lifecycle_status,
            hide_palette_window,
            start_guide,
            cancel_guide,
            get_guide_status,
            get_settings_bootstrap,
            save_runtime_settings,
            reload_runtime_state,
            show_settings_window,
            get_extensions_bootstrap,
            set_extension_enabled,
            uninstall_extension,
            refresh_extension_catalog,
            install_catalog_extension
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

fn bundled_extensions_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("extensions")
        .join("bundled")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::settings_window::SettingsWindowController;

    use omni_palette::{
        config::runtime::{CommandBehavior, RuntimePaths, ThemeMode},
        domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
        runtime_state::RuntimeStateLoadOptions,
    };

    use super::*;

    #[test]
    fn health_check_reports_phase_six_marketplace_catalog_refresh_and_install() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(
            payload.phase,
            "Phase 6C.2 - Marketplace Catalog Refresh And Install"
        );
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn show_settings_window_shows_and_focuses_settings_window() {
        let controller = Arc::new(RecordingSettingsWindowController::default());
        let settings_window = SettingsWindow::new(controller.clone());

        let status = settings_window.show_settings_window();

        assert_eq!(status.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert!(status.visible);
        assert_eq!(status.show_count, 1);
        assert_eq!(status.focus_count, 1);
        assert_eq!(status.last_error, None);
        assert_eq!(controller.log(), vec!["show", "focus"]);
    }

    #[test]
    fn show_settings_window_failure_returns_controlled_error() {
        let controller = Arc::new(RecordingSettingsWindowController::failing_show());
        let settings_window = SettingsWindow::new(controller);

        let status = settings_window.show_settings_window();

        assert_eq!(status.status, RuntimeSettingsResultStatusDto::Failed);
        assert!(!status.visible);
        assert_eq!(status.show_count, 0);
        assert_eq!(status.focus_count, 0);
        assert_eq!(
            status.last_error,
            Some("Failed to show settings window: show failed".to_string())
        );
    }

    #[test]
    fn settings_bootstrap_includes_runtime_config_and_status() {
        let runtime = runtime_state_for_settings("settings-bootstrap", true);

        let bootstrap = settings_bootstrap_from_runtime(&runtime);

        assert_eq!(bootstrap.config.activation_hint, "Ctrl+Shift+P");
        assert_eq!(
            bootstrap.config.activation_shortcut,
            ActivationShortcutDto::from(ctrl_shift_p_shortcut())
        );
        assert_eq!(
            bootstrap.default_activation_shortcut,
            ActivationShortcutDto::from(ctrl_shift_p_shortcut())
        );
        assert_eq!(bootstrap.config.command_behavior, CommandBehavior::Execute);
        assert_eq!(bootstrap.config.appearance_theme, ThemeMode::System);
        assert_eq!(bootstrap.config.github.enabled, false);
        assert!(bootstrap
            .config_path
            .as_deref()
            .is_some_and(|path| path.ends_with("config.toml")));
        assert_eq!(bootstrap.config_error, None);
        assert_eq!(bootstrap.runtime_status.activation_hint, "Ctrl+Shift+P");
    }

    #[test]
    fn save_runtime_settings_updates_editable_fields_and_preserves_activation() {
        let runtime = runtime_state_for_settings("save-runtime-settings", true);
        let updater = RecordingActivationShortcutUpdater::default();
        let request = runtime_settings_save_request(
            ctrl_shift_p_shortcut(),
            CommandBehavior::Guide,
            ThemeMode::Dark,
        );

        let result = save_runtime_settings_for_runtime(&runtime, &updater, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Settings saved");
        assert_eq!(result.config.command_behavior, CommandBehavior::Guide);
        assert_eq!(result.config.appearance_theme, ThemeMode::Dark);
        assert_eq!(result.config.activation_hint, "Ctrl+Shift+P");
        assert_eq!(runtime.config().activation.to_string(), "Ctrl+Shift+P");
        assert_eq!(runtime.status().command_behavior, CommandBehavior::Guide);
        assert_eq!(updater.updates(), Vec::<String>::new());
    }

    #[test]
    fn save_runtime_settings_updates_activation_and_hotkey_listener() {
        let runtime = runtime_state_for_settings("save-runtime-settings-activation", true);
        let updater = RecordingActivationShortcutUpdater::default();
        let next_shortcut = ctrl_alt_space_shortcut();
        let request =
            runtime_settings_save_request(next_shortcut, CommandBehavior::Guide, ThemeMode::Dark);

        let result = save_runtime_settings_for_runtime(&runtime, &updater, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.config.activation_hint, "Ctrl+Alt+Space");
        assert_eq!(
            result.config.activation_shortcut,
            ActivationShortcutDto::from(next_shortcut)
        );
        assert_eq!(runtime.config().activation, next_shortcut);
        assert_eq!(runtime.status().activation_hint, "Ctrl+Alt+Space");
        assert_eq!(updater.updates(), vec!["Ctrl+Alt+Space".to_string()]);
    }

    #[test]
    fn failed_runtime_settings_save_does_not_update_config() {
        let runtime = runtime_state_for_settings("save-runtime-settings-missing-path", false);
        let updater = RecordingActivationShortcutUpdater::default();
        let request = runtime_settings_save_request(
            ctrl_shift_p_shortcut(),
            CommandBehavior::Guide,
            ThemeMode::Light,
        );

        let result = save_runtime_settings_for_runtime(&runtime, &updater, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "APPDATA is not set, so Omni Palette cannot save user settings."
        );
        assert_eq!(runtime.config().command_behavior, CommandBehavior::Execute);
        assert_eq!(updater.updates(), Vec::<String>::new());
    }

    #[test]
    fn hotkey_update_failure_does_not_save_activation_or_config() {
        let runtime = runtime_state_for_settings("save-runtime-settings-hotkey-failure", true);
        let updater = RecordingActivationShortcutUpdater::failing();
        let request = runtime_settings_save_request(
            ctrl_alt_space_shortcut(),
            CommandBehavior::Guide,
            ThemeMode::Dark,
        );

        let result = save_runtime_settings_for_runtime(&runtime, &updater, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "Could not update activation shortcut: register failed"
        );
        assert_eq!(runtime.config().activation, ctrl_shift_p_shortcut());
        assert_eq!(runtime.config().command_behavior, CommandBehavior::Execute);
        assert_eq!(updater.updates(), vec!["Ctrl+Alt+Space".to_string()]);
    }

    #[test]
    fn config_save_failure_rolls_back_hotkey_update() {
        let runtime = runtime_state_for_settings("save-runtime-settings-rollback", false);
        let updater = RecordingActivationShortcutUpdater::default();
        let request = runtime_settings_save_request(
            ctrl_alt_space_shortcut(),
            CommandBehavior::Guide,
            ThemeMode::Dark,
        );

        let result = save_runtime_settings_for_runtime(&runtime, &updater, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "APPDATA is not set, so Omni Palette cannot save user settings."
        );
        assert_eq!(runtime.config().activation, ctrl_shift_p_shortcut());
        assert_eq!(
            updater.updates(),
            vec!["Ctrl+Alt+Space".to_string(), "Ctrl+Shift+P".to_string()]
        );
    }

    #[test]
    fn reload_runtime_state_result_reports_counts() {
        let runtime = runtime_state_for_settings("reload-runtime-state", true);

        let result = reload_runtime_state_for_runtime(&runtime);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert!(result.message.contains("Reloaded extensions:"));
        assert_eq!(result.runtime_status.ignored_process_count, 0);
    }

    #[test]
    fn extensions_bootstrap_lists_bundled_and_downloaded_extensions() {
        let runtime = runtime_state_for_extensions("extensions-bootstrap", true);

        let bootstrap = extensions_bootstrap_from_runtime(&runtime);

        assert_eq!(bootstrap.install_root_error, None);
        assert!(bootstrap
            .install_root
            .as_deref()
            .is_some_and(|path| path.ends_with("user-extensions")));
        assert!(bootstrap.bundled_extensions.iter().any(|extension| {
            extension.id == "windows"
                && extension.source_id == "bundled"
                && extension.name == "Windows"
                && extension.kind == ExtensionKindDto::Static
                && extension.enabled
                && !extension.can_uninstall
                && !extension.has_settings
        }));
        assert!(bootstrap.bundled_extensions.iter().any(|extension| {
            extension.id == "ahk_agent"
                && extension.source_id == "bundled"
                && extension.kind == ExtensionKindDto::WasmPlugin
                && extension.has_settings
        }));
        assert_eq!(bootstrap.downloaded_extensions.len(), 1);
        assert_eq!(bootstrap.downloaded_extensions[0].id, "chrome");
        assert_eq!(bootstrap.downloaded_extensions[0].source_id, "github");
        assert!(bootstrap.downloaded_extensions[0].can_uninstall);
    }

    #[test]
    fn extensions_bootstrap_reports_downloaded_empty_state() {
        let runtime = runtime_state_for_extensions("extensions-empty", false);

        let bootstrap = extensions_bootstrap_from_runtime(&runtime);

        assert_eq!(bootstrap.downloaded_extensions, Vec::new());
        assert!(!bootstrap.bundled_extensions.is_empty());
    }

    #[test]
    fn bundled_extension_enablement_writes_state_reloads_runtime_and_returns_rows() {
        let runtime = runtime_state_for_extensions("bundled-enable", true);
        let request = ExtensionEnabledRequestDto {
            extension_id: "windows".to_string(),
            source_id: "bundled".to_string(),
            enabled: false,
        };

        let result = set_extension_enabled_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Disabled Windows");
        let windows = result
            .extensions
            .bundled_extensions
            .iter()
            .find(|extension| extension.id == "windows")
            .expect("windows bundled extension should remain visible");
        assert!(!windows.enabled);
        assert_eq!(result.runtime_status.application_count, 1);
    }

    #[test]
    fn downloaded_extension_enablement_writes_state_reloads_runtime_and_returns_rows() {
        let runtime = runtime_state_for_extensions("downloaded-enable", true);
        let request = ExtensionEnabledRequestDto {
            extension_id: "chrome".to_string(),
            source_id: "github".to_string(),
            enabled: false,
        };

        let result = set_extension_enabled_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Disabled chrome");
        let chrome = result
            .extensions
            .downloaded_extensions
            .iter()
            .find(|extension| extension.id == "chrome")
            .expect("chrome downloaded extension should remain visible");
        assert!(!chrome.enabled);
    }

    #[test]
    fn downloaded_extension_uninstall_removes_state_and_file() {
        let runtime = runtime_state_for_extensions("downloaded-uninstall", true);
        let install_root = runtime
            .user_extensions_root()
            .expect("install root should exist");
        let installed_path = install_root.join("static").join("chrome.toml");
        let request = ExtensionTargetRequestDto {
            extension_id: "chrome".to_string(),
            source_id: "github".to_string(),
        };

        let result = uninstall_extension_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Uninstalled chrome");
        assert!(!installed_path.exists());
        assert!(result.extensions.downloaded_extensions.is_empty());
    }

    #[test]
    fn bundled_extension_uninstall_returns_controlled_failure() {
        let runtime = runtime_state_for_extensions("bundled-uninstall", false);
        let request = ExtensionTargetRequestDto {
            extension_id: "windows".to_string(),
            source_id: "bundled".to_string(),
        };

        let result = uninstall_extension_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "Bundled extensions can be disabled, but not uninstalled."
        );
        assert!(!result.extensions.bundled_extensions.is_empty());
    }

    #[test]
    fn missing_extension_target_returns_controlled_failure() {
        let runtime = runtime_state_for_extensions("missing-extension-target", false);
        let request = ExtensionEnabledRequestDto {
            extension_id: "missing".to_string(),
            source_id: "github".to_string(),
            enabled: false,
        };

        let result = set_extension_enabled_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert!(result
            .message
            .contains("Installed extension not found: github/missing"));
    }

    #[test]
    fn catalog_refresh_returns_current_platform_entries_and_stores_cache() {
        let runtime = runtime_state_for_extensions("catalog-refresh", false);
        let service = Arc::new(RecordingMarketplaceService::with_fetch_results(vec![Ok(
            catalog_with_entries(vec![
                catalog_entry("chrome", "Chrome", Os::Windows, ExtensionKind::Static),
                catalog_entry("linux", "Linux Tools", Os::Linux, ExtensionKind::Static),
            ]),
        )]));
        let marketplace = MarketplaceState::new(service.clone());

        let result =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].id, "chrome");
        assert_eq!(result.entries[0].platform, Os::Windows);
        assert_eq!(
            service.fetch_sources(),
            vec!["Greg-Lim/omni-palette-desktop"]
        );
        assert_eq!(
            marketplace
                .cached_catalog()
                .expect("catalog should be cached")
                .source
                .repo,
            "omni-palette-desktop"
        );
    }

    #[test]
    fn catalog_refresh_failure_preserves_previous_cached_entries() {
        let runtime = runtime_state_for_extensions("catalog-refresh-failure", false);
        let service = Arc::new(RecordingMarketplaceService::with_fetch_results(vec![
            Ok(catalog_with_entries(vec![catalog_entry(
                "chrome",
                "Chrome",
                Os::Windows,
                ExtensionKind::Static,
            )])),
            Err("network down".to_string()),
        ]));
        let marketplace = MarketplaceState::new(service);

        let success =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());
        let failure =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());

        assert_eq!(success.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(failure.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(failure.message, "network down");
        assert_eq!(failure.entries.len(), 1);
        assert_eq!(failure.entries[0].id, "chrome");
    }

    #[test]
    fn install_without_cached_catalog_returns_controlled_failure() {
        let runtime = runtime_state_for_extensions("install-without-catalog", false);
        let service = Arc::new(RecordingMarketplaceService::default());
        let marketplace = MarketplaceState::new(service);

        let result = install_catalog_extension_for_runtime(&runtime, &marketplace, "chrome");

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "Refresh the extension catalog before installing extensions."
        );
        assert!(result.extensions.downloaded_extensions.is_empty());
    }

    #[test]
    fn install_cached_static_entry_writes_state_reloads_runtime_and_returns_rows() {
        let runtime = runtime_state_for_extensions("install-cached-static", false);
        let service = Arc::new(RecordingMarketplaceService::with_fetch_results(vec![Ok(
            catalog_with_entries(vec![catalog_entry(
                "chrome",
                "Chrome",
                Os::Windows,
                ExtensionKind::Static,
            )]),
        )]));
        let marketplace = MarketplaceState::new(service.clone());
        let refresh =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());
        assert_eq!(refresh.status, RuntimeSettingsResultStatusDto::Succeeded);

        let result = install_catalog_extension_for_runtime(&runtime, &marketplace, "chrome");

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Installed Chrome v0.1.0");
        assert_eq!(
            service.install_sources(),
            vec!["Greg-Lim/omni-palette-desktop"]
        );
        assert!(result
            .extensions
            .downloaded_extensions
            .iter()
            .any(|extension| {
                extension.id == "chrome"
                    && extension.source_id == "github"
                    && extension.version == "0.1.0"
                    && extension.enabled
            }));
        assert_eq!(result.runtime_status.application_count, 2);
    }

    #[test]
    fn missing_catalog_entry_returns_controlled_failure() {
        let runtime = runtime_state_for_extensions("install-missing-catalog-entry", false);
        let service = Arc::new(RecordingMarketplaceService::with_fetch_results(vec![Ok(
            catalog_with_entries(vec![catalog_entry(
                "chrome",
                "Chrome",
                Os::Windows,
                ExtensionKind::Static,
            )]),
        )]));
        let marketplace = MarketplaceState::new(service);
        let refresh =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());
        assert_eq!(refresh.status, RuntimeSettingsResultStatusDto::Succeeded);

        let result = install_catalog_extension_for_runtime(&runtime, &marketplace, "missing");

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(result.message, "Catalog extension not found: missing");
    }

    #[test]
    fn unsupported_wasm_catalog_entry_returns_controlled_failure() {
        let runtime = runtime_state_for_extensions("install-unsupported-wasm", false);
        let service = Arc::new(RecordingMarketplaceService::with_fetch_results(vec![Ok(
            catalog_with_entries(vec![catalog_entry(
                "plugin",
                "Plugin",
                Os::Windows,
                ExtensionKind::WasmPlugin,
            )]),
        )]));
        let marketplace = MarketplaceState::new(service.clone());
        let refresh =
            refresh_extension_catalog_for_runtime(&runtime, &marketplace, github_source_dto());
        assert_eq!(refresh.status, RuntimeSettingsResultStatusDto::Succeeded);

        let result = install_catalog_extension_for_runtime(&runtime, &marketplace, "plugin");

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "Only static catalog extensions can be installed in Phase 6C.2."
        );
        assert_eq!(service.install_sources(), Vec::<String>::new());
    }

    #[test]
    fn bundled_extensions_root_points_to_repo_extensions() {
        let root = bundled_extensions_root();

        assert!(root.ends_with("extensions/bundled") || root.ends_with("extensions\\bundled"));
    }

    fn runtime_state_for_settings(name: &str, with_config_path: bool) -> OmniRuntimeState {
        let root = PathBuf::from("target")
            .join("tauri-settings-tests")
            .join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("settings test root should reset");
        }
        std::fs::create_dir_all(root.join("static")).expect("static dir should be created");
        let config_path = with_config_path.then(|| root.join("config.toml"));

        OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        })
    }

    fn runtime_state_for_extensions(name: &str, with_downloaded: bool) -> OmniRuntimeState {
        let root = PathBuf::from("target")
            .join("tauri-extension-tests")
            .join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("extension test root should reset");
        }
        let bundled_root = root.join("bundled");
        let user_root = root.join("user-extensions");
        std::fs::create_dir_all(bundled_root.join("static"))
            .expect("bundled static dir should be created");
        std::fs::create_dir_all(bundled_root.join("plugins").join("ahk_agent"))
            .expect("bundled plugin dir should be created");
        std::fs::create_dir_all(user_root.join("static"))
            .expect("user static dir should be created");
        write_static_extension(
            &bundled_root.join("static").join("windows.toml"),
            "windows",
            "Windows",
        );
        write_plugin_extension(
            &bundled_root
                .join("plugins")
                .join("ahk_agent")
                .join("plugin.toml"),
            "ahk_agent",
            "AHK",
            true,
        );

        if with_downloaded {
            write_static_extension(
                &user_root.join("static").join("chrome.toml"),
                "chrome",
                "Chrome",
            );
            let mut state = omni_palette::core::extensions::install::InstalledState::default();
            state.upsert(
                omni_palette::core::extensions::install::InstalledExtension {
                    id: "chrome".to_string(),
                    version: "0.1.0".to_string(),
                    platform: Os::Windows,
                    kind: omni_palette::core::extensions::catalog::ExtensionKind::Static,
                    source_id: "github".to_string(),
                    package_sha256: "0".repeat(64),
                    enabled: true,
                    installed_path: PathBuf::from("static").join("chrome.toml"),
                },
            );
            omni_palette::core::extensions::install::save_installed_state(&user_root, &state)
                .expect("installed state should be saved");
        }

        OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: bundled_root,
            user_extensions_root: Some(user_root),
            dev_config_path: root.join("missing-dev-config.toml"),
            runtime_paths: RuntimePaths {
                config_path: Some(root.join("config.toml")),
                local_cache_root: None,
            },
            current_os: Os::Windows,
        })
    }

    fn write_static_extension(path: &std::path::Path, id: &str, name: &str) {
        std::fs::write(
            path,
            format!(
                r#"
version = 2
platform = "windows"

[app]
id = "{id}"
name = "{name}"
process_name = "{id}.exe"

[actions.copy]
name = "Copy"
cmd = {{ mods = ["ctrl"], key = "KeyC" }}
"#
            ),
        )
        .expect("static extension should be written");
    }

    fn write_plugin_extension(path: &std::path::Path, id: &str, name: &str, has_settings: bool) {
        let settings = if has_settings {
            r#"
[settings]
source = "wasm"
"#
        } else {
            ""
        };
        std::fs::write(
            path,
            format!(
                r#"
id = "{id}"
name = "{name}"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = []
{settings}
"#
            ),
        )
        .expect("plugin extension should be written");
    }

    #[derive(Default)]
    struct RecordingSettingsWindowController {
        log: Mutex<Vec<&'static str>>,
        fail_on_show: bool,
    }

    impl RecordingSettingsWindowController {
        fn failing_show() -> Self {
            Self {
                log: Mutex::new(Vec::new()),
                fail_on_show: true,
            }
        }

        fn log(&self) -> Vec<&'static str> {
            self.log.lock().expect("log should lock").clone()
        }
    }

    impl SettingsWindowController for RecordingSettingsWindowController {
        fn show(&self) -> Result<(), String> {
            self.log.lock().expect("log should lock").push("show");
            if self.fail_on_show {
                return Err("show failed".to_string());
            }
            Ok(())
        }

        fn focus(&self) -> Result<(), String> {
            self.log.lock().expect("log should lock").push("focus");
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingActivationShortcutUpdater {
        updates: Mutex<Vec<KeyboardShortcut>>,
        fail: bool,
    }

    impl RecordingActivationShortcutUpdater {
        fn failing() -> Self {
            Self {
                updates: Mutex::new(Vec::new()),
                fail: true,
            }
        }

        fn updates(&self) -> Vec<String> {
            self.updates
                .lock()
                .expect("updates should lock")
                .iter()
                .map(ToString::to_string)
                .collect()
        }
    }

    impl ActivationShortcutUpdater for RecordingActivationShortcutUpdater {
        fn update_activation_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
            self.updates
                .lock()
                .expect("updates should lock")
                .push(shortcut);
            if self.fail {
                return Err("register failed".to_string());
            }
            Ok(())
        }
    }

    fn runtime_settings_save_request(
        activation_shortcut: KeyboardShortcut,
        command_behavior: CommandBehavior,
        appearance_theme: ThemeMode,
    ) -> RuntimeSettingsSaveRequestDto {
        RuntimeSettingsSaveRequestDto {
            activation_shortcut: ActivationShortcutDto::from(activation_shortcut),
            command_behavior,
            appearance_theme,
            github: GitHubExtensionSourceDto {
                owner: "Example".to_string(),
                repo: "extensions".to_string(),
                branch: "stable".to_string(),
                catalog_path: "catalog.json".to_string(),
                enabled: true,
            },
        }
    }

    fn ctrl_shift_p_shortcut() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            key: Key::KeyP,
        }
    }

    fn ctrl_alt_space_shortcut() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                alt: true,
                ..Default::default()
            },
            key: Key::Space,
        }
    }

    fn github_source_dto() -> GitHubExtensionSourceDto {
        GitHubExtensionSourceDto {
            owner: "Greg-Lim".to_string(),
            repo: "omni-palette-desktop".to_string(),
            branch: "master".to_string(),
            catalog_path: "extensions/registry/catalog.v1.json".to_string(),
            enabled: true,
        }
    }

    fn catalog_with_entries(entries: Vec<CatalogEntry>) -> ExtensionCatalog {
        ExtensionCatalog {
            schema_version: 1,
            generated_at: None,
            expires_at_unix: None,
            entries,
        }
    }

    fn catalog_entry(id: &str, name: &str, platform: Os, kind: ExtensionKind) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            platform,
            kind,
            package_url: format!(
                "https://github.com/Greg-Lim/omni-palette-desktop/releases/download/{id}-0.1.0/{id}.gpext"
            ),
            package_sha256: "0".repeat(64),
            size_bytes: Some(128),
            publisher: Some("Omni Palette".to_string()),
            description: Some(format!("{name} command pack")),
            license: None,
            homepage: None,
            repository: None,
            keywords: vec![id.to_string(), "commands".to_string()],
            min_app_version: None,
        }
    }

    #[derive(Default)]
    struct RecordingMarketplaceService {
        fetch_results: Mutex<Vec<Result<ExtensionCatalog, String>>>,
        fetch_sources: Mutex<Vec<String>>,
        install_sources: Mutex<Vec<String>>,
    }

    impl RecordingMarketplaceService {
        fn with_fetch_results(results: Vec<Result<ExtensionCatalog, String>>) -> Self {
            Self {
                fetch_results: Mutex::new(results.into_iter().rev().collect()),
                fetch_sources: Mutex::new(Vec::new()),
                install_sources: Mutex::new(Vec::new()),
            }
        }

        fn fetch_sources(&self) -> Vec<String> {
            self.fetch_sources
                .lock()
                .expect("fetch sources should lock")
                .clone()
        }

        fn install_sources(&self) -> Vec<String> {
            self.install_sources
                .lock()
                .expect("install sources should lock")
                .clone()
        }
    }

    impl MarketplaceService for RecordingMarketplaceService {
        fn fetch_catalog(
            &self,
            _install_root: &std::path::Path,
            source: &GitHubExtensionSource,
        ) -> Result<ExtensionCatalog, String> {
            self.fetch_sources
                .lock()
                .expect("fetch sources should lock")
                .push(format!("{}/{}", source.owner, source.repo));
            self.fetch_results
                .lock()
                .expect("fetch results should lock")
                .pop()
                .unwrap_or_else(|| Err("no fetch result queued".to_string()))
        }

        fn install_entry(
            &self,
            install_root: &std::path::Path,
            source: &GitHubExtensionSource,
            entry: &CatalogEntry,
            _current_os: Os,
        ) -> Result<omni_palette::core::extensions::install::InstalledExtension, String> {
            self.install_sources
                .lock()
                .expect("install sources should lock")
                .push(format!("{}/{}", source.owner, source.repo));
            let static_dir = install_root.join("static");
            std::fs::create_dir_all(&static_dir).map_err(|err| err.to_string())?;
            let installed_path = static_dir.join(format!("{}.toml", entry.id));
            write_static_extension(&installed_path, &entry.id, &entry.name);

            let installed = omni_palette::core::extensions::install::InstalledExtension {
                id: entry.id.clone(),
                version: entry.version.clone(),
                platform: entry.platform,
                kind: entry.kind,
                source_id: "github".to_string(),
                package_sha256: entry.package_sha256.clone(),
                enabled: true,
                installed_path: PathBuf::from("static").join(format!("{}.toml", entry.id)),
            };
            let mut state =
                omni_palette::core::extensions::install::load_installed_state(install_root)
                    .map_err(|err| err.to_string())?;
            state.upsert(installed.clone());
            omni_palette::core::extensions::install::save_installed_state(install_root, &state)
                .map_err(|err| err.to_string())?;
            Ok(installed)
        }
    }
}
