mod guide_lifecycle;
mod hotkey_bridge;
mod window_lifecycle;

use std::{path::PathBuf, sync::Arc, thread};

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

use crate::guide_lifecycle::{GuideLifecycle, GuideRuntimeCommand, GuideStatusDto, GUIDE_DURATION};
use crate::hotkey_bridge::{HotkeyBridge, HotkeyStatusDto, PaletteActivationHandler};
use crate::window_lifecycle::{WindowLifecycle, WindowLifecycleStatusDto};

struct AppState {
    backend: Arc<PaletteBackend>,
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
        phase: "Phase 5B - Guide Mode And Refined Palette Positioning",
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
            get_guide_status
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
    fn health_check_reports_phase_five_guide_mode() {
        let payload = health_check();

        assert_eq!(payload.app_name, "Omni Palette");
        assert_eq!(
            payload.phase,
            "Phase 5B - Guide Mode And Refined Palette Positioning"
        );
        assert_eq!(payload.status, "ok");
    }

    #[test]
    fn bundled_extensions_root_points_to_repo_extensions() {
        let root = bundled_extensions_root();

        assert!(root.ends_with("extensions/bundled") || root.ends_with("extensions\\bundled"));
    }
}
