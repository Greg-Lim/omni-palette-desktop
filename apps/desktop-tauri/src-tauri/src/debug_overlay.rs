use std::sync::{mpsc, Arc, Mutex};

use omni_palette::{
    backend_contract::PaletteSnapshotDto,
    domain::action::{CommandPriority, ContextRoot, FocusState},
    platform::{
        platform_interface::RawWindowHandleExt, windows::context::context::get_hwnd_from_raw,
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::RuntimeSettingsResultStatusDto;

const DEBUG_WINDOW_LABEL: &str = "debug";
const MAX_DEBUG_BACKGROUND_WINDOWS: usize = 12;
const MAX_DEBUG_COMMAND_ROWS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugOverlayStatusDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub visible: bool,
    pub show_count: u64,
    pub hide_count: u64,
    pub focus_count: u64,
    pub last_error: Option<String>,
}

impl Default for DebugOverlayStatusDto {
    fn default() -> Self {
        Self {
            status: RuntimeSettingsResultStatusDto::Succeeded,
            message: "Debug window idle".to_string(),
            visible: false,
            show_count: 0,
            hide_count: 0,
            focus_count: 0,
            last_error: None,
        }
    }
}

pub(crate) trait DebugOverlayController: Send + Sync {
    fn show(&self) -> Result<(), String>;
    fn hide(&self) -> Result<(), String>;
    fn focus(&self) -> Result<(), String>;
}

pub(crate) struct DebugOverlay {
    status: Mutex<DebugOverlayStatusDto>,
    controller: Arc<dyn DebugOverlayController>,
}

impl DebugOverlay {
    pub(crate) fn new(controller: Arc<dyn DebugOverlayController>) -> Self {
        Self {
            status: Mutex::new(DebugOverlayStatusDto::default()),
            controller,
        }
    }

    pub(crate) fn for_tauri(app: AppHandle) -> Self {
        Self::new(Arc::new(TauriDebugOverlayController::new(app)))
    }

    pub(crate) fn status(&self) -> DebugOverlayStatusDto {
        self.status
            .lock()
            .expect("debug status should lock")
            .clone()
    }

    pub(crate) fn show_debug_overlay(&self) -> DebugOverlayStatusDto {
        if let Err(err) = self.controller.show() {
            return self.record_error(format!("Failed to show debug window: {err}"));
        }

        {
            let mut status = self.status.lock().expect("debug status should lock");
            status.visible = true;
            status.show_count += 1;
        }

        if let Err(err) = self.controller.focus() {
            return self.record_error(format!("Failed to focus debug window: {err}"));
        }

        let mut status = self.status.lock().expect("debug status should lock");
        status.status = RuntimeSettingsResultStatusDto::Succeeded;
        status.message = "Debug window shown".to_string();
        status.focus_count += 1;
        status.last_error = None;
        status.clone()
    }

    pub(crate) fn close_debug_overlay(&self) -> DebugOverlayStatusDto {
        if let Err(err) = self.controller.hide() {
            return self.record_error(format!("Failed to hide debug window: {err}"));
        }

        let mut status = self.status.lock().expect("debug status should lock");
        status.status = RuntimeSettingsResultStatusDto::Succeeded;
        status.message = "Debug window hidden".to_string();
        status.visible = false;
        status.hide_count += 1;
        status.last_error = None;
        status.clone()
    }

    fn record_error(&self, message: String) -> DebugOverlayStatusDto {
        let mut status = self.status.lock().expect("debug status should lock");
        status.status = RuntimeSettingsResultStatusDto::Failed;
        status.message = message.clone();
        status.last_error = Some(message);
        status.clone()
    }
}

struct TauriDebugOverlayController {
    app: AppHandle,
}

impl TauriDebugOverlayController {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn with_window<T, F>(&self, operation_name: &'static str, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(tauri::WebviewWindow) -> Result<T, String> + Send + 'static,
    {
        let app = self.app.clone();
        let (tx, rx) = mpsc::channel();

        self.app
            .run_on_main_thread(move || {
                let result = app
                    .get_webview_window(DEBUG_WINDOW_LABEL)
                    .ok_or_else(|| format!("Missing Tauri window '{DEBUG_WINDOW_LABEL}'"))
                    .and_then(operation);
                let _ = tx.send(result);
            })
            .map_err(|err| format!("Failed to schedule {operation_name}: {err}"))?;

        rx.recv()
            .map_err(|err| format!("Failed to receive {operation_name} result: {err}"))?
    }
}

impl DebugOverlayController for TauriDebugOverlayController {
    fn show(&self) -> Result<(), String> {
        self.with_window("debug window show", |window| {
            window.show().map_err(|err| err.to_string())
        })
    }

    fn hide(&self) -> Result<(), String> {
        self.with_window("debug window hide", |window| {
            window.hide().map_err(|err| err.to_string())
        })
    }

    fn focus(&self) -> Result<(), String> {
        self.with_window("debug window focus", |window| {
            if window.is_minimized().map_err(|err| err.to_string())? {
                window.unminimize().map_err(|err| err.to_string())?;
            }
            window.set_focus().map_err(|err| err.to_string())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugWindowSummaryDto {
    pub process_name: Option<String>,
    pub hwnd: Option<isize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugCommandCandidateDto {
    pub focus_state: FocusState,
    pub priority: CommandPriority,
    pub favorite: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugCommandSummaryDto {
    pub total: usize,
    pub focused: usize,
    pub background: usize,
    pub global: usize,
    pub favorites: usize,
    pub suppressed_priority: usize,
    pub low_priority: usize,
    pub medium_priority: usize,
    pub high_priority: usize,
}

impl DebugCommandSummaryDto {
    fn from_candidates(candidates: impl IntoIterator<Item = DebugCommandCandidateDto>) -> Self {
        let mut summary = Self::default();
        for candidate in candidates {
            summary.total += 1;
            if candidate.favorite {
                summary.favorites += 1;
            }
            match candidate.focus_state {
                FocusState::Focused => summary.focused += 1,
                FocusState::Background => summary.background += 1,
                FocusState::Global => summary.global += 1,
            }
            match candidate.priority {
                CommandPriority::Suppressed => summary.suppressed_priority += 1,
                CommandPriority::Low => summary.low_priority += 1,
                CommandPriority::Medium => summary.medium_priority += 1,
                CommandPriority::High => summary.high_priority += 1,
            }
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugCommandRowDto {
    pub label: String,
    pub focus_state: FocusState,
    pub priority: CommandPriority,
    pub favorite: bool,
    pub score: i32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugPaletteStateDto {
    pub query: String,
    pub filtered_count: usize,
    pub top_rows: Vec<DebugCommandRowDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugSnapshotDto {
    pub foreground_window: Option<DebugWindowSummaryDto>,
    pub background_windows: Vec<DebugWindowSummaryDto>,
    pub background_total: usize,
    pub active_tags: Vec<String>,
    pub text_input_active: bool,
    pub ignored_process_name: Option<String>,
    pub command_summary: DebugCommandSummaryDto,
    pub palette_state: DebugPaletteStateDto,
}

#[derive(Default)]
pub(crate) struct DebugDiagnosticsState {
    latest_palette_snapshot: Mutex<Option<PaletteSnapshotDto>>,
}

impl DebugDiagnosticsState {
    pub(crate) fn record_palette_snapshot(&self, snapshot: &PaletteSnapshotDto) {
        *self
            .latest_palette_snapshot
            .lock()
            .expect("debug palette snapshot should lock") = Some(snapshot.clone());
    }

    pub(crate) fn snapshot_from_context(
        &self,
        context_root: ContextRoot,
        command_candidates: Vec<DebugCommandCandidateDto>,
        ignored_process_name: Option<String>,
    ) -> DebugSnapshotDto {
        let background_total = context_root.bg_context.len();
        let background_windows = context_root
            .bg_context
            .iter()
            .take(MAX_DEBUG_BACKGROUND_WINDOWS)
            .copied()
            .map(|handle| DebugWindowSummaryDto {
                process_name: handle.get_app_process_name(),
                hwnd: get_hwnd_from_raw(handle).map(|hwnd| hwnd.0 as isize),
            })
            .collect();
        let active_tags = context_root.active_interaction.tags.clone();
        let text_input_active = context_root.active_interaction.has_tag("ui.text_input");

        DebugSnapshotDto {
            foreground_window: context_root.get_active().copied().map(|handle| {
                DebugWindowSummaryDto {
                    process_name: handle.get_app_process_name(),
                    hwnd: get_hwnd_from_raw(handle).map(|hwnd| hwnd.0 as isize),
                }
            }),
            background_windows,
            background_total,
            active_tags,
            text_input_active,
            ignored_process_name,
            command_summary: DebugCommandSummaryDto::from_candidates(command_candidates),
            palette_state: self.latest_palette_state(),
        }
    }

    fn latest_palette_state(&self) -> DebugPaletteStateDto {
        let Some(snapshot) = self
            .latest_palette_snapshot
            .lock()
            .expect("debug palette snapshot should lock")
            .clone()
        else {
            return DebugPaletteStateDto::default();
        };

        DebugPaletteStateDto {
            query: snapshot.query,
            filtered_count: snapshot.commands.len(),
            top_rows: snapshot
                .commands
                .into_iter()
                .take(MAX_DEBUG_COMMAND_ROWS)
                .map(|command| DebugCommandRowDto {
                    label: command.label,
                    focus_state: command.focus_state,
                    priority: command.priority,
                    favorite: command.favorite,
                    score: command.score,
                    tags: command.tags,
                })
                .collect(),
        }
    }
}
