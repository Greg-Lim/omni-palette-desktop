mod hotkey_bridge;

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

struct AppState {
    backend: Arc<PaletteBackend>,
    hotkey_bridge: Arc<HotkeyBridge>,
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
        phase: "Phase 4C - Hotkey Listener And Ignored-App Passthrough",
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
            let hotkey_bridge = Arc::new(HotkeyBridge::start(
                runtime_state.clone(),
                app.handle().clone(),
            ));
            app.manage(AppState {
                backend: Arc::clone(&backend),
                hotkey_bridge,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_palette_bootstrap,
            search_commands,
            execute_command,
            get_hotkey_status
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
    fn health_check_reports_phase_four_hotkey_listener() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(
            payload.phase,
            "Phase 4C - Hotkey Listener And Ignored-App Passthrough"
        );
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn bundled_extensions_root_points_to_repo_extensions() {
        let root = bundled_extensions_root();

        assert!(root.ends_with("extensions/bundled") || root.ends_with("extensions\\bundled"));
    }
}
