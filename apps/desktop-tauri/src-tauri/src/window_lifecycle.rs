use std::sync::{mpsc, Arc, Mutex};

use omni_palette::{
    backend_contract::{PaletteBackend, PaletteSnapshotDto},
    domain::action::ContextRoot,
    platform::windows::context::context::{get_hwnd_from_raw, monitor_work_area_from_window},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::hotkey_bridge::PaletteActivationHandler;

pub const WINDOW_LIFECYCLE_EVENT_NAME: &str = "omni://palette-window-lifecycle";
const MAIN_WINDOW_LABEL: &str = "main";
const PALETTE_WINDOW_WIDTH: u32 = 780;
const PALETTE_WINDOW_HEIGHT: u32 = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLifecycleStatusDto {
    pub visible: bool,
    pub show_count: u64,
    pub hide_count: u64,
    pub focus_count: u64,
    pub position_count: u64,
    pub last_action: Option<WindowLifecycleActionDto>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLifecycleEventPayloadDto {
    pub action: WindowLifecycleActionDto,
    pub visible: bool,
    pub show_count: u64,
    pub hide_count: u64,
    pub focus_count: u64,
    pub position_count: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowLifecycleActionDto {
    Shown,
    Hidden,
    Error,
}

#[derive(Clone)]
struct WindowLifecycleStatusStore {
    inner: Arc<Mutex<WindowLifecycleStatusState>>,
}

#[derive(Debug)]
struct WindowLifecycleStatusState {
    visible: bool,
    show_count: u64,
    hide_count: u64,
    focus_count: u64,
    position_count: u64,
    last_action: Option<WindowLifecycleActionDto>,
    last_error: Option<String>,
}

impl WindowLifecycleStatusStore {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WindowLifecycleStatusState {
                visible: false,
                show_count: 0,
                hide_count: 0,
                focus_count: 0,
                position_count: 0,
                last_action: None,
                last_error: None,
            })),
        }
    }

    fn snapshot(&self) -> WindowLifecycleStatusDto {
        let state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        WindowLifecycleStatusDto {
            visible: state.visible,
            show_count: state.show_count,
            hide_count: state.hide_count,
            focus_count: state.focus_count,
            position_count: state.position_count,
            last_action: state.last_action,
            last_error: state.last_error.clone(),
        }
    }

    fn record_positioned(&self) {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.position_count += 1;
        state.last_error = None;
    }

    fn record_focused(&self) {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.focus_count += 1;
        state.last_error = None;
    }

    fn record_show_succeeded(&self) {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.visible = true;
        state.show_count += 1;
        state.last_error = None;
    }

    fn record_shown(&self) -> WindowLifecycleEventPayloadDto {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.last_action = Some(WindowLifecycleActionDto::Shown);
        state.last_error = None;
        event_from_state(&state, WindowLifecycleActionDto::Shown, None)
    }

    fn record_hidden(&self) -> WindowLifecycleEventPayloadDto {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.visible = false;
        state.hide_count += 1;
        state.last_action = Some(WindowLifecycleActionDto::Hidden);
        state.last_error = None;
        event_from_state(&state, WindowLifecycleActionDto::Hidden, None)
    }

    fn record_error(&self, message: String) -> WindowLifecycleEventPayloadDto {
        let mut state = self
            .inner
            .lock()
            .expect("window lifecycle status should lock");
        state.last_action = Some(WindowLifecycleActionDto::Error);
        state.last_error = Some(message.clone());
        event_from_state(&state, WindowLifecycleActionDto::Error, Some(message))
    }
}

fn event_from_state(
    state: &WindowLifecycleStatusState,
    action: WindowLifecycleActionDto,
    message: Option<String>,
) -> WindowLifecycleEventPayloadDto {
    WindowLifecycleEventPayloadDto {
        action,
        visible: state.visible,
        show_count: state.show_count,
        hide_count: state.hide_count,
        focus_count: state.focus_count,
        position_count: state.position_count,
        message,
    }
}

trait PaletteSessionManager: Send + Sync {
    fn open_palette_session(&self, context: ContextRoot) -> Result<PaletteSnapshotDto, String>;
    fn close_palette_session(&self);
}

impl PaletteSessionManager for PaletteBackend {
    fn open_palette_session(&self, context: ContextRoot) -> Result<PaletteSnapshotDto, String> {
        PaletteBackend::open_palette_session(self, context)
    }

    fn close_palette_session(&self) {
        PaletteBackend::close_palette_session(self);
    }
}

trait PaletteWindowController: Send + Sync {
    fn is_visible(&self) -> Result<bool, String>;
    fn position(&self, context: &ContextRoot, visible_command_count: usize) -> Result<(), String>;
    fn show(&self) -> Result<(), String>;
    fn hide(&self) -> Result<(), String>;
    fn unminimize_if_needed(&self) -> Result<(), String>;
    fn focus(&self) -> Result<(), String>;
}

trait WindowLifecycleEventSink: Send + Sync {
    fn emit_window_lifecycle_event(
        &self,
        payload: WindowLifecycleEventPayloadDto,
    ) -> Result<(), String>;
}

struct TauriWindowLifecycleEventSink {
    app: AppHandle,
}

impl TauriWindowLifecycleEventSink {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl WindowLifecycleEventSink for TauriWindowLifecycleEventSink {
    fn emit_window_lifecycle_event(
        &self,
        payload: WindowLifecycleEventPayloadDto,
    ) -> Result<(), String> {
        self.app
            .emit(WINDOW_LIFECYCLE_EVENT_NAME, payload)
            .map_err(|err| format!("Failed to emit window lifecycle event: {err}"))
    }
}

struct TauriPaletteWindowController {
    app: AppHandle,
    label: String,
}

impl TauriPaletteWindowController {
    fn new(app: AppHandle, label: impl Into<String>) -> Self {
        Self {
            app,
            label: label.into(),
        }
    }

    fn with_window<T, F>(&self, operation_name: &'static str, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(tauri::WebviewWindow) -> Result<T, String> + Send + 'static,
    {
        let app = self.app.clone();
        let label = self.label.clone();
        let (tx, rx) = mpsc::channel();

        self.app
            .run_on_main_thread(move || {
                let result = app
                    .get_webview_window(&label)
                    .ok_or_else(|| format!("Missing Tauri window '{label}'"))
                    .and_then(operation);
                let _ = tx.send(result);
            })
            .map_err(|err| format!("Failed to schedule {operation_name}: {err}"))?;

        rx.recv()
            .map_err(|err| format!("Failed to receive {operation_name} result: {err}"))?
    }
}

impl PaletteWindowController for TauriPaletteWindowController {
    fn is_visible(&self) -> Result<bool, String> {
        self.with_window("palette visibility check", |window| {
            window.is_visible().map_err(|err| err.to_string())
        })
    }

    fn position(&self, context: &ContextRoot, visible_command_count: usize) -> Result<(), String> {
        let work_area = active_work_area(context);
        self.with_window("palette window positioning", move |window| {
            let size = palette_window_size(visible_command_count);
            window.set_size(size).map_err(|err| err.to_string())?;
            if let Some((left, top, right, bottom)) = work_area {
                let work_width = right.saturating_sub(left);
                let work_height = bottom.saturating_sub(top);
                let x = left + ((work_width - size.width as i32) / 2).max(0);
                let y = top + ((work_height as f32) * 0.10).round() as i32;
                window
                    .set_position(PhysicalPosition::new(x, y))
                    .map_err(|err| err.to_string())
            } else {
                window.center().map_err(|err| err.to_string())
            }
        })
    }

    fn show(&self) -> Result<(), String> {
        self.with_window("palette window show", |window| {
            window.show().map_err(|err| err.to_string())
        })
    }

    fn hide(&self) -> Result<(), String> {
        self.with_window("palette window hide", |window| {
            window.hide().map_err(|err| err.to_string())
        })
    }

    fn unminimize_if_needed(&self) -> Result<(), String> {
        self.with_window("palette window unminimize", |window| {
            if window.is_minimized().map_err(|err| err.to_string())? {
                window.unminimize().map_err(|err| err.to_string())?;
            }
            Ok(())
        })
    }

    fn focus(&self) -> Result<(), String> {
        self.with_window("palette window focus", |window| {
            window.set_focus().map_err(|err| err.to_string())
        })
    }
}

fn active_work_area(context: &ContextRoot) -> Option<(i32, i32, i32, i32)> {
    context
        .get_active()
        .and_then(|handle| get_hwnd_from_raw(*handle))
        .and_then(monitor_work_area_from_window)
}

pub fn palette_window_size(_visible_command_count: usize) -> PhysicalSize<u32> {
    PhysicalSize::new(PALETTE_WINDOW_WIDTH, PALETTE_WINDOW_HEIGHT)
}

pub struct WindowLifecycle {
    status: WindowLifecycleStatusStore,
    session_manager: Arc<dyn PaletteSessionManager>,
    controller: Arc<dyn PaletteWindowController>,
    event_sink: Arc<dyn WindowLifecycleEventSink>,
}

impl WindowLifecycle {
    fn new(
        session_manager: Arc<dyn PaletteSessionManager>,
        controller: Arc<dyn PaletteWindowController>,
        event_sink: Arc<dyn WindowLifecycleEventSink>,
    ) -> Self {
        Self {
            status: WindowLifecycleStatusStore::new(),
            session_manager,
            controller,
            event_sink,
        }
    }

    pub fn for_tauri(backend: Arc<PaletteBackend>, app: AppHandle) -> Self {
        let session_manager: Arc<dyn PaletteSessionManager> = backend;
        Self::new(
            session_manager,
            Arc::new(TauriPaletteWindowController::new(
                app.clone(),
                MAIN_WINDOW_LABEL,
            )),
            Arc::new(TauriWindowLifecycleEventSink::new(app)),
        )
    }

    pub fn status(&self) -> WindowLifecycleStatusDto {
        self.status.snapshot()
    }

    pub fn handle_activation(&self, context: ContextRoot) {
        let visible = match self.controller.is_visible() {
            Ok(visible) => visible,
            Err(err) => {
                self.emit(
                    self.status
                        .record_error(format!("Failed to read palette window visibility: {err}")),
                );
                return;
            }
        };

        if visible {
            self.hide_palette();
        } else {
            self.show_palette(context);
        }
    }

    pub fn hide_palette_window(&self) -> WindowLifecycleStatusDto {
        match self.controller.is_visible() {
            Ok(true) => self.hide_palette(),
            Ok(false) => self.session_manager.close_palette_session(),
            Err(err) => {
                self.emit(
                    self.status
                        .record_error(format!("Failed to read palette window visibility: {err}")),
                );
            }
        }

        self.status()
    }

    pub fn hide_palette_for_command_execution(&self) -> bool {
        match self.controller.is_visible() {
            Ok(true) => self.hide_palette_without_closing_session(),
            Ok(false) => false,
            Err(err) => {
                self.emit(
                    self.status
                        .record_error(format!("Failed to read palette window visibility: {err}")),
                );
                false
            }
        }
    }

    pub fn restore_palette_after_command_failure(&self) -> WindowLifecycleStatusDto {
        match self.controller.is_visible() {
            Ok(true) => return self.status(),
            Ok(false) => {}
            Err(err) => {
                self.emit(
                    self.status
                        .record_error(format!("Failed to read palette window visibility: {err}")),
                );
                return self.status();
            }
        }

        if let Err(err) = self.controller.show() {
            self.emit(
                self.status
                    .record_error(format!("Failed to restore palette window: {err}")),
            );
            return self.status();
        }
        self.status.record_show_succeeded();

        if let Err(err) = self.controller.unminimize_if_needed() {
            self.emit(
                self.status
                    .record_error(format!("Failed to unminimize palette window: {err}")),
            );
            return self.status();
        }

        if let Err(err) = self.controller.focus() {
            self.emit(
                self.status
                    .record_error(format!("Failed to focus palette window: {err}")),
            );
            return self.status();
        }
        self.status.record_focused();

        self.emit(self.status.record_shown());
        self.status()
    }

    pub fn close_palette_session(&self) {
        self.session_manager.close_palette_session();
    }

    fn show_palette(&self, context: ContextRoot) {
        let snapshot = match self.session_manager.open_palette_session(context.clone()) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                self.emit(
                    self.status
                        .record_error(format!("Failed to prepare palette command session: {err}")),
                );
                return;
            }
        };

        if let Err(err) = self.controller.position(&context, snapshot.commands.len()) {
            self.session_manager.close_palette_session();
            self.emit(
                self.status
                    .record_error(format!("Failed to position palette window: {err}")),
            );
            return;
        }
        self.status.record_positioned();

        if let Err(err) = self.controller.show() {
            self.session_manager.close_palette_session();
            self.emit(
                self.status
                    .record_error(format!("Failed to show palette window: {err}")),
            );
            return;
        }
        self.status.record_show_succeeded();

        if let Err(err) = self.controller.unminimize_if_needed() {
            self.emit(
                self.status
                    .record_error(format!("Failed to unminimize palette window: {err}")),
            );
            return;
        }

        if let Err(err) = self.controller.focus() {
            self.emit(
                self.status
                    .record_error(format!("Failed to focus palette window: {err}")),
            );
            return;
        }
        self.status.record_focused();

        self.emit(self.status.record_shown());
    }

    fn hide_palette(&self) {
        if let Err(err) = self.controller.hide() {
            self.emit(
                self.status
                    .record_error(format!("Failed to hide palette window: {err}")),
            );
            return;
        }

        self.session_manager.close_palette_session();
        self.emit(self.status.record_hidden());
    }

    fn hide_palette_without_closing_session(&self) -> bool {
        if let Err(err) = self.controller.hide() {
            self.emit(
                self.status
                    .record_error(format!("Failed to hide palette window: {err}")),
            );
            return false;
        }

        self.emit(self.status.record_hidden());
        true
    }

    fn emit(&self, payload: WindowLifecycleEventPayloadDto) {
        if let Err(err) = self.event_sink.emit_window_lifecycle_event(payload) {
            let _ = self.status.record_error(err);
        }
    }
}

impl PaletteActivationHandler for WindowLifecycle {
    fn handle_palette_activation(&self, context: ContextRoot) {
        self.handle_activation(context);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use omni_palette::{
        backend_contract::{CommandDto, CommandId, MatchRangeDto, PaletteSessionId},
        domain::action::{CommandPriority, ContextRoot, FocusState, InteractionContext},
    };

    use super::*;

    #[test]
    fn lifecycle_status_starts_hidden() {
        let lifecycle = test_lifecycle(false, None, Ok(()));

        assert_eq!(
            lifecycle.status(),
            WindowLifecycleStatusDto {
                visible: false,
                show_count: 0,
                hide_count: 0,
                focus_count: 0,
                position_count: 0,
                last_action: None,
                last_error: None,
            }
        );
    }

    #[test]
    fn hidden_activation_prepares_session_before_position_show_and_focus() {
        let lifecycle = test_lifecycle(false, None, Ok(()));

        lifecycle.handle_activation(empty_context());

        assert_eq!(
            lifecycle.log(),
            vec!["prepare_session", "position", "show", "focus",]
        );
        assert_eq!(
            lifecycle.status(),
            WindowLifecycleStatusDto {
                visible: true,
                show_count: 1,
                hide_count: 0,
                focus_count: 1,
                position_count: 1,
                last_action: Some(WindowLifecycleActionDto::Shown),
                last_error: None,
            }
        );
        assert_eq!(
            lifecycle.events(),
            vec![WindowLifecycleEventPayloadDto {
                action: WindowLifecycleActionDto::Shown,
                visible: true,
                show_count: 1,
                hide_count: 0,
                focus_count: 1,
                position_count: 1,
                message: None,
            }]
        );
    }

    #[test]
    fn hidden_activation_positions_palette_with_session_command_count() {
        let lifecycle = test_lifecycle_with_visible_command_count(false, 2);

        lifecycle.handle_activation(empty_context());

        assert_eq!(
            lifecycle.log(),
            vec!["prepare_session:2", "position:2", "show", "focus"]
        );
    }

    #[test]
    fn palette_window_size_stays_fixed_for_any_command_count() {
        let fixed_size = PhysicalSize::new(PALETTE_WINDOW_WIDTH, PALETTE_WINDOW_HEIGHT);

        assert_eq!(palette_window_size(0), fixed_size);
        assert_eq!(palette_window_size(1), fixed_size);
        assert_eq!(palette_window_size(20), fixed_size);
    }

    #[test]
    fn visible_activation_hides_window_and_closes_session_without_rebuilding_commands() {
        let lifecycle = test_lifecycle(true, None, Ok(()));

        lifecycle.handle_activation(empty_context());

        assert_eq!(lifecycle.log(), vec!["hide", "close_session"]);
        assert_eq!(
            lifecycle.status(),
            WindowLifecycleStatusDto {
                visible: false,
                show_count: 0,
                hide_count: 1,
                focus_count: 0,
                position_count: 0,
                last_action: Some(WindowLifecycleActionDto::Hidden),
                last_error: None,
            }
        );
    }

    #[test]
    fn window_operation_failure_records_error_and_emits_error_event() {
        let lifecycle = test_lifecycle(false, Some("show"), Ok(()));

        lifecycle.handle_activation(empty_context());

        assert_eq!(
            lifecycle.log(),
            vec!["prepare_session", "position", "show", "close_session"]
        );
        let status = lifecycle.status();
        assert_eq!(status.visible, false);
        assert_eq!(status.show_count, 0);
        assert_eq!(status.position_count, 1);
        assert_eq!(status.last_action, Some(WindowLifecycleActionDto::Error));
        assert_eq!(
            status.last_error,
            Some("Failed to show palette window: boom".to_string())
        );
        assert_eq!(
            lifecycle.events(),
            vec![WindowLifecycleEventPayloadDto {
                action: WindowLifecycleActionDto::Error,
                visible: false,
                show_count: 0,
                hide_count: 0,
                focus_count: 0,
                position_count: 1,
                message: Some("Failed to show palette window: boom".to_string()),
            }]
        );
    }

    #[test]
    fn focus_failure_preserves_visible_window_status() {
        let lifecycle = test_lifecycle(false, Some("focus"), Ok(()));

        lifecycle.handle_activation(empty_context());

        let status = lifecycle.status();
        assert_eq!(status.visible, true);
        assert_eq!(status.show_count, 1);
        assert_eq!(status.focus_count, 0);
        assert_eq!(
            status.last_error,
            Some("Failed to focus palette window: boom".to_string())
        );
    }

    #[test]
    fn hide_request_hides_visible_window_closes_session_and_returns_status() {
        let lifecycle = test_lifecycle(true, None, Ok(()));

        let status = lifecycle.hide_palette_window();

        assert_eq!(lifecycle.log(), vec!["hide", "close_session"]);
        assert_eq!(
            status,
            WindowLifecycleStatusDto {
                visible: false,
                show_count: 0,
                hide_count: 1,
                focus_count: 0,
                position_count: 0,
                last_action: Some(WindowLifecycleActionDto::Hidden),
                last_error: None,
            }
        );
        assert_eq!(
            lifecycle.events(),
            vec![WindowLifecycleEventPayloadDto {
                action: WindowLifecycleActionDto::Hidden,
                visible: false,
                show_count: 0,
                hide_count: 1,
                focus_count: 0,
                position_count: 0,
                message: None,
            }]
        );
    }

    #[test]
    fn hide_request_when_already_hidden_closes_session_and_returns_status() {
        let lifecycle = test_lifecycle(false, None, Ok(()));

        let status = lifecycle.hide_palette_window();

        assert_eq!(lifecycle.log(), vec!["close_session"]);
        assert_eq!(
            status,
            WindowLifecycleStatusDto {
                visible: false,
                show_count: 0,
                hide_count: 0,
                focus_count: 0,
                position_count: 0,
                last_action: None,
                last_error: None,
            }
        );
        assert_eq!(lifecycle.events(), Vec::new());
    }

    #[test]
    fn command_execution_hide_hides_visible_window_without_closing_session() {
        let lifecycle = test_lifecycle(true, None, Ok(()));

        let hid_window = lifecycle.hide_palette_for_command_execution();

        assert!(hid_window);
        assert_eq!(lifecycle.log(), vec!["hide"]);
        assert_eq!(
            lifecycle.status(),
            WindowLifecycleStatusDto {
                visible: false,
                show_count: 0,
                hide_count: 1,
                focus_count: 0,
                position_count: 0,
                last_action: Some(WindowLifecycleActionDto::Hidden),
                last_error: None,
            }
        );
    }

    #[test]
    fn command_execution_failure_can_restore_hidden_palette_without_rebuilding_session() {
        let lifecycle = test_lifecycle(false, None, Ok(()));

        let status = lifecycle.restore_palette_after_command_failure();

        assert_eq!(lifecycle.log(), vec!["show", "focus"]);
        assert_eq!(
            status,
            WindowLifecycleStatusDto {
                visible: true,
                show_count: 1,
                hide_count: 0,
                focus_count: 1,
                position_count: 0,
                last_action: Some(WindowLifecycleActionDto::Shown),
                last_error: None,
            }
        );
    }

    struct TestLifecycle {
        lifecycle: WindowLifecycle,
        log: Arc<Mutex<Vec<String>>>,
        events: Arc<Mutex<Vec<WindowLifecycleEventPayloadDto>>>,
    }

    impl TestLifecycle {
        fn handle_activation(&self, context: ContextRoot) {
            self.lifecycle.handle_activation(context);
        }

        fn hide_palette_window(&self) -> WindowLifecycleStatusDto {
            self.lifecycle.hide_palette_window()
        }

        fn hide_palette_for_command_execution(&self) -> bool {
            self.lifecycle.hide_palette_for_command_execution()
        }

        fn restore_palette_after_command_failure(&self) -> WindowLifecycleStatusDto {
            self.lifecycle.restore_palette_after_command_failure()
        }

        fn status(&self) -> WindowLifecycleStatusDto {
            self.lifecycle.status()
        }

        fn log(&self) -> Vec<String> {
            self.log.lock().expect("log should lock").clone()
        }

        fn events(&self) -> Vec<WindowLifecycleEventPayloadDto> {
            self.events.lock().expect("events should lock").clone()
        }
    }

    fn test_lifecycle(
        visible: bool,
        fail_on: Option<&'static str>,
        open_result: Result<(), String>,
    ) -> TestLifecycle {
        test_lifecycle_inner(
            visible,
            fail_on,
            open_result.map(|_| snapshot_with_command_count(1)),
            false,
        )
    }

    fn test_lifecycle_with_visible_command_count(
        visible: bool,
        visible_command_count: usize,
    ) -> TestLifecycle {
        test_lifecycle_inner(
            visible,
            None,
            Ok(snapshot_with_command_count(visible_command_count)),
            true,
        )
    }

    fn test_lifecycle_inner(
        visible: bool,
        fail_on: Option<&'static str>,
        open_result: Result<PaletteSnapshotDto, String>,
        record_command_count: bool,
    ) -> TestLifecycle {
        let log = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let session = Arc::new(RecordingSessionManager {
            log: Arc::clone(&log),
            open_result,
            record_command_count,
        });
        let controller = Arc::new(RecordingWindowController {
            visible: Mutex::new(visible),
            fail_on,
            log: Arc::clone(&log),
            record_command_count,
        });
        let event_sink = Arc::new(RecordingWindowLifecycleEventSink {
            events: Arc::clone(&events),
        });

        TestLifecycle {
            lifecycle: WindowLifecycle::new(session, controller, event_sink),
            log,
            events,
        }
    }

    struct RecordingSessionManager {
        log: Arc<Mutex<Vec<String>>>,
        open_result: Result<PaletteSnapshotDto, String>,
        record_command_count: bool,
    }

    impl PaletteSessionManager for RecordingSessionManager {
        fn open_palette_session(
            &self,
            _context: ContextRoot,
        ) -> Result<PaletteSnapshotDto, String> {
            let count = self
                .open_result
                .as_ref()
                .map(|snapshot| snapshot.commands.len())
                .unwrap_or_default();
            let label = if self.record_command_count {
                format!("prepare_session:{count}")
            } else {
                "prepare_session".to_string()
            };
            self.log.lock().expect("log should lock").push(label);
            self.open_result.clone()
        }

        fn close_palette_session(&self) {
            self.log
                .lock()
                .expect("log should lock")
                .push("close_session".to_string());
        }
    }

    struct RecordingWindowController {
        visible: Mutex<bool>,
        fail_on: Option<&'static str>,
        log: Arc<Mutex<Vec<String>>>,
        record_command_count: bool,
    }

    impl RecordingWindowController {
        fn maybe_fail(&self, operation: &'static str) -> Result<(), String> {
            self.log
                .lock()
                .expect("log should lock")
                .push(operation.to_string());
            if self.fail_on == Some(operation) {
                Err("boom".to_string())
            } else {
                Ok(())
            }
        }
    }

    impl PaletteWindowController for RecordingWindowController {
        fn is_visible(&self) -> Result<bool, String> {
            Ok(*self.visible.lock().expect("visible should lock"))
        }

        fn position(
            &self,
            _context: &ContextRoot,
            visible_command_count: usize,
        ) -> Result<(), String> {
            if self.record_command_count {
                self.log
                    .lock()
                    .expect("log should lock")
                    .push(format!("position:{visible_command_count}"));
                if self.fail_on == Some("position") {
                    Err("boom".to_string())
                } else {
                    Ok(())
                }
            } else {
                self.maybe_fail("position")
            }
        }

        fn show(&self) -> Result<(), String> {
            self.maybe_fail("show")?;
            *self.visible.lock().expect("visible should lock") = true;
            Ok(())
        }

        fn hide(&self) -> Result<(), String> {
            self.maybe_fail("hide")?;
            *self.visible.lock().expect("visible should lock") = false;
            Ok(())
        }

        fn unminimize_if_needed(&self) -> Result<(), String> {
            Ok(())
        }

        fn focus(&self) -> Result<(), String> {
            self.maybe_fail("focus")
        }
    }

    struct RecordingWindowLifecycleEventSink {
        events: Arc<Mutex<Vec<WindowLifecycleEventPayloadDto>>>,
    }

    impl WindowLifecycleEventSink for RecordingWindowLifecycleEventSink {
        fn emit_window_lifecycle_event(
            &self,
            payload: WindowLifecycleEventPayloadDto,
        ) -> Result<(), String> {
            self.events
                .lock()
                .expect("events should lock")
                .push(payload);
            Ok(())
        }
    }

    fn empty_context() -> ContextRoot {
        ContextRoot {
            fg_context: Vec::new(),
            bg_context: Vec::new(),
            active_interaction: InteractionContext::default(),
        }
    }

    fn snapshot_with_command_count(command_count: usize) -> PaletteSnapshotDto {
        PaletteSnapshotDto {
            session_id: PaletteSessionId::new("test-session"),
            query: String::new(),
            commands: (0..command_count)
                .map(|index| CommandDto {
                    id: CommandId::new(format!("cmd-{index}")),
                    label: format!("Command {index}"),
                    shortcut_text: String::new(),
                    guide_hint: None,
                    focus_state: FocusState::Global,
                    priority: CommandPriority::Medium,
                    favorite: false,
                    tags: Vec::new(),
                    original_order: index,
                    score: 0,
                    label_matches: Vec::<MatchRangeDto>::new(),
                })
                .collect(),
        }
    }
}
