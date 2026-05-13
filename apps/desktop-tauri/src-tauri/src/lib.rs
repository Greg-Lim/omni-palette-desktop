use std::{path::PathBuf, sync::Arc};

use omni_palette::{
    backend_contract::{
        CommandExecutionResultDto, CommandId, PaletteBackend, PaletteBootstrapDto,
        PaletteSnapshotDto,
    },
    domain::action::Os,
};
use serde::Serialize;
use tauri::State;

struct AppState {
    backend: Arc<PaletteBackend>,
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
        phase: "Phase 3 - Backend Command Bridge",
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
fn execute_command(
    command_id: String,
    state: State<'_, AppState>,
) -> CommandExecutionResultDto {
    state.backend.execute_command(&CommandId::new(command_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            backend: Arc::new(PaletteBackend::default_for_bundled_root(
                bundled_extensions_root(),
                Os::Windows,
            )),
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_palette_bootstrap,
            search_commands,
            execute_command
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
    fn health_check_reports_phase_three_bridge() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(payload.phase, "Phase 3 - Backend Command Bridge");
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn bundled_extensions_root_points_to_repo_extensions() {
        let root = bundled_extensions_root();

        assert!(root.ends_with("extensions/bundled") || root.ends_with("extensions\\bundled"));
    }
}
