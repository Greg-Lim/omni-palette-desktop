mod hotkey_bridge;
mod window_lifecycle;

use std::{path::PathBuf, sync::Arc};

use omni_palette::{
    backend_contract::{
        CommandExecutionResultDto, CommandId, PaletteBackend, PaletteBootstrapDto,
        PaletteSnapshotDto,
    },
    domain::action::Os,
    runtime_state::{OmniRuntimeState, RuntimeStateLoadOptions},
};
use serde::Serialize;
use tauri::{Manager, State};

use crate::hotkey_bridge::{HotkeyBridge, HotkeyStatusDto};
use crate::window_lifecycle::{WindowLifecycle, WindowLifecycleStatusDto};

struct AppState {
    backend: Arc<PaletteBackend>,
    hotkey_bridge: Arc<HotkeyBridge>,
    window_lifecycle: Arc<WindowLifecycle>,
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
        phase: "Phase 4D - Tauri Window Lifecycle",
        status: "ok",
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
            let activation_handler: Arc<dyn hotkey_bridge::PaletteActivationHandler> =
                window_lifecycle.clone();
            let hotkey_bridge = Arc::new(HotkeyBridge::start(
                runtime_state.clone(),
                app.handle().clone(),
                activation_handler,
            ));
            app.manage(AppState {
                backend: Arc::clone(&backend),
                hotkey_bridge,
                window_lifecycle,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_palette_bootstrap,
            search_commands,
            execute_command,
            get_hotkey_status,
            get_window_lifecycle_status
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
    use super::*;

    #[test]
    fn health_check_reports_phase_four_window_lifecycle() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(payload.phase, "Phase 4D - Tauri Window Lifecycle");
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn bundled_extensions_root_points_to_repo_extensions() {
        let root = bundled_extensions_root();

        assert!(root.ends_with("extensions/bundled") || root.ends_with("extensions\\bundled"));
    }
}
