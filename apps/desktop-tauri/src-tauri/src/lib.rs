mod guide_lifecycle;
mod hotkey_bridge;
mod window_lifecycle;

use std::{path::PathBuf, sync::Arc, thread};

use omni_palette::{
    backend_contract::{
        CommandExecutionResultDto, CommandId, PaletteBackend, PaletteBootstrapDto,
        PaletteSnapshotDto,
    },
    config::runtime::{CommandBehavior, GitHubExtensionSource, RuntimeConfig, ThemeMode},
    domain::action::Os,
    runtime_state::{OmniRuntimeState, RuntimeStateLoadOptions, RuntimeStatusDto},
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::guide_lifecycle::{GuideLifecycle, GuideRuntimeCommand, GuideStatusDto, GUIDE_DURATION};
use crate::hotkey_bridge::{HotkeyBridge, HotkeyStatusDto, PaletteActivationHandler};
use crate::window_lifecycle::{WindowLifecycle, WindowLifecycleStatusDto};

struct AppState {
    backend: Arc<PaletteBackend>,
    runtime_state: OmniRuntimeState,
    hotkey_bridge: Arc<HotkeyBridge>,
    window_lifecycle: Arc<WindowLifecycle>,
    guide_lifecycle: Arc<GuideLifecycle>,
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
        phase: "Phase 6A - Runtime Settings Foundation",
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
pub struct RuntimeSettingsDto {
    pub activation_hint: String,
    pub command_behavior: CommandBehavior,
    pub appearance_theme: ThemeMode,
    pub github: GitHubExtensionSourceDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsBootstrapDto {
    pub config: RuntimeSettingsDto,
    pub config_path: Option<String>,
    pub config_error: Option<String>,
    pub runtime_status: RuntimeStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSettingsSaveRequestDto {
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

fn runtime_settings_from_config(config: RuntimeConfig) -> RuntimeSettingsDto {
    RuntimeSettingsDto {
        activation_hint: config.activation.to_string(),
        command_behavior: config.command_behavior,
        appearance_theme: config.appearance.theme,
        github: GitHubExtensionSourceDto::from(config.github),
    }
}

fn settings_bootstrap_from_runtime(runtime: &OmniRuntimeState) -> SettingsBootstrapDto {
    let config_load = runtime.config_load();
    SettingsBootstrapDto {
        config: runtime_settings_from_config(config_load.config),
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
    request: RuntimeSettingsSaveRequestDto,
) -> RuntimeSettingsSaveResultDto {
    let mut next_config = runtime.config();
    next_config.command_behavior = request.command_behavior;
    next_config.appearance.theme = request.appearance_theme;
    next_config.github = request.github.into();

    match runtime.save_runtime_config(next_config) {
        Ok(message) => RuntimeSettingsSaveResultDto {
            status: RuntimeSettingsResultStatusDto::Succeeded,
            message,
            config: runtime_settings_from_config(runtime.config()),
            runtime_status: runtime.status(),
        },
        Err(message) => RuntimeSettingsSaveResultDto {
            status: RuntimeSettingsResultStatusDto::Failed,
            message,
            config: runtime_settings_from_config(runtime.config()),
            runtime_status: runtime.status(),
        },
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
    save_runtime_settings_for_runtime(&state.runtime_state, request)
}

#[tauri::command]
fn reload_runtime_state(state: State<'_, AppState>) -> RuntimeReloadResultDto {
    reload_runtime_state_for_runtime(&state.runtime_state)
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
                guide_lifecycle,
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
            reload_runtime_state
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
    use omni_palette::{
        config::runtime::{CommandBehavior, GitHubExtensionSource, RuntimePaths, ThemeMode},
        runtime_state::RuntimeStateLoadOptions,
    };

    use super::*;

    #[test]
    fn health_check_reports_phase_six_runtime_settings() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(payload.phase, "Phase 6A - Runtime Settings Foundation");
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn settings_bootstrap_includes_runtime_config_and_status() {
        let runtime = runtime_state_for_settings("settings-bootstrap", true);

        let bootstrap = settings_bootstrap_from_runtime(&runtime);

        assert_eq!(bootstrap.config.activation_hint, "Ctrl+Shift+P");
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
        let request = RuntimeSettingsSaveRequestDto {
            command_behavior: CommandBehavior::Guide,
            appearance_theme: ThemeMode::Dark,
            github: GitHubExtensionSourceDto {
                owner: "Example".to_string(),
                repo: "extensions".to_string(),
                branch: "stable".to_string(),
                catalog_path: "catalog.json".to_string(),
                enabled: true,
            },
        };

        let result = save_runtime_settings_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Succeeded);
        assert_eq!(result.message, "Settings saved");
        assert_eq!(result.config.command_behavior, CommandBehavior::Guide);
        assert_eq!(result.config.appearance_theme, ThemeMode::Dark);
        assert_eq!(result.config.activation_hint, "Ctrl+Shift+P");
        assert_eq!(runtime.config().activation.to_string(), "Ctrl+Shift+P");
        assert_eq!(runtime.status().command_behavior, CommandBehavior::Guide);
    }

    #[test]
    fn failed_runtime_settings_save_does_not_update_config() {
        let runtime = runtime_state_for_settings("save-runtime-settings-missing-path", false);
        let request = RuntimeSettingsSaveRequestDto {
            command_behavior: CommandBehavior::Guide,
            appearance_theme: ThemeMode::Light,
            github: GitHubExtensionSourceDto::from(GitHubExtensionSource::default()),
        };

        let result = save_runtime_settings_for_runtime(&runtime, request);

        assert_eq!(result.status, RuntimeSettingsResultStatusDto::Failed);
        assert_eq!(
            result.message,
            "APPDATA is not set, so Omni Palette cannot save user settings."
        );
        assert_eq!(runtime.config().command_behavior, CommandBehavior::Execute);
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
}
