use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use omni_palette::{
    backend_contract::{CommandExecutionResultDto, GuideCommand},
    domain::hotkey::KeyboardShortcut,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::window_lifecycle::WindowLifecycle;

pub const GUIDE_EVENT_NAME: &str = "omni://palette-guide";
pub const GUIDE_DURATION: Duration = Duration::from_secs(8);
const GUIDE_WINDOW_LABEL: &str = "guide";
const GUIDE_WINDOW_WIDTH: u32 = 560;
const GUIDE_WINDOW_HEIGHT: u32 = 218;
const GUIDE_VERTICAL_POSITION_FACTOR: f32 = 0.45;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuideStatusDto {
    pub active: bool,
    pub command_label: Option<String>,
    pub shortcut_text: Option<String>,
    pub activation_hint: String,
    pub start_count: u64,
    pub complete_count: u64,
    pub cancel_count: u64,
    pub expire_count: u64,
    pub last_action: Option<GuideActionDto>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuideEventPayloadDto {
    pub action: GuideActionDto,
    pub active: bool,
    pub command_label: Option<String>,
    pub shortcut_text: Option<String>,
    pub activation_hint: String,
    pub start_count: u64,
    pub complete_count: u64,
    pub cancel_count: u64,
    pub expire_count: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuideActionDto {
    Started,
    Completed,
    Cancelled,
    Expired,
    Error,
}

#[derive(Clone)]
struct GuideStatusStore {
    inner: Arc<Mutex<GuideStatusState>>,
}

struct GuideStatusState {
    active: Option<ActiveGuide>,
    activation_hint: String,
    start_count: u64,
    complete_count: u64,
    cancel_count: u64,
    expire_count: u64,
    last_action: Option<GuideActionDto>,
    last_error: Option<String>,
    generation: u64,
}

#[derive(Clone)]
struct ActiveGuide {
    command: Arc<dyn GuideRuntimeCommand>,
    generation: u64,
}

impl GuideStatusStore {
    fn new(activation_hint: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GuideStatusState {
                active: None,
                activation_hint,
                start_count: 0,
                complete_count: 0,
                cancel_count: 0,
                expire_count: 0,
                last_action: None,
                last_error: None,
                generation: 0,
            })),
        }
    }

    fn snapshot(&self) -> GuideStatusDto {
        let state = self.inner.lock().expect("guide status should lock");
        status_from_state(&state)
    }

    fn record_started(&self, command: Arc<dyn GuideRuntimeCommand>) -> GuideEventPayloadDto {
        let mut state = self.inner.lock().expect("guide status should lock");
        state.generation += 1;
        let generation = state.generation;
        state.active = Some(ActiveGuide {
            command,
            generation,
        });
        state.start_count += 1;
        state.last_action = Some(GuideActionDto::Started);
        state.last_error = None;
        event_from_state(&state, GuideActionDto::Started, None)
    }

    fn take_active_for_completion(&self) -> Option<ActiveGuide> {
        let mut state = self.inner.lock().expect("guide status should lock");
        let active = state.active.take()?;
        state.complete_count += 1;
        state.last_action = Some(GuideActionDto::Completed);
        state.last_error = None;
        Some(active)
    }

    fn record_completed_event(&self) -> GuideEventPayloadDto {
        let state = self.inner.lock().expect("guide status should lock");
        event_from_state(&state, GuideActionDto::Completed, None)
    }

    fn record_cancelled(&self) -> Option<GuideEventPayloadDto> {
        let mut state = self.inner.lock().expect("guide status should lock");
        state.active.take()?;
        state.cancel_count += 1;
        state.last_action = Some(GuideActionDto::Cancelled);
        state.last_error = None;
        Some(event_from_state(&state, GuideActionDto::Cancelled, None))
    }

    fn record_expired(&self, generation: u64) -> Option<GuideEventPayloadDto> {
        let mut state = self.inner.lock().expect("guide status should lock");
        if state
            .active
            .as_ref()
            .is_none_or(|active| active.generation != generation)
        {
            return None;
        }
        state.active.take();
        state.expire_count += 1;
        state.last_action = Some(GuideActionDto::Expired);
        state.last_error = None;
        Some(event_from_state(&state, GuideActionDto::Expired, None))
    }

    fn record_error(&self, message: String) -> GuideEventPayloadDto {
        let mut state = self.inner.lock().expect("guide status should lock");
        state.active = None;
        state.last_action = Some(GuideActionDto::Error);
        state.last_error = Some(message.clone());
        event_from_state(&state, GuideActionDto::Error, Some(message))
    }

    fn active(&self) -> Option<ActiveGuide> {
        self.inner
            .lock()
            .expect("guide status should lock")
            .active
            .clone()
    }
}

fn status_from_state(state: &GuideStatusState) -> GuideStatusDto {
    let active = state.active.as_ref();
    GuideStatusDto {
        active: active.is_some(),
        command_label: active.map(|guide| guide.command.label()),
        shortcut_text: active.map(|guide| guide.command.shortcut_text()),
        activation_hint: state.activation_hint.clone(),
        start_count: state.start_count,
        complete_count: state.complete_count,
        cancel_count: state.cancel_count,
        expire_count: state.expire_count,
        last_action: state.last_action,
        last_error: state.last_error.clone(),
    }
}

fn event_from_state(
    state: &GuideStatusState,
    action: GuideActionDto,
    message: Option<String>,
) -> GuideEventPayloadDto {
    let status = status_from_state(state);
    GuideEventPayloadDto {
        action,
        active: status.active,
        command_label: status.command_label,
        shortcut_text: status.shortcut_text,
        activation_hint: status.activation_hint,
        start_count: status.start_count,
        complete_count: status.complete_count,
        cancel_count: status.cancel_count,
        expire_count: status.expire_count,
        message,
    }
}

pub trait GuideRuntimeCommand: Send + Sync {
    fn label(&self) -> String;
    fn shortcut_text(&self) -> String;
    fn captured_shortcut(&self) -> Option<KeyboardShortcut>;
    fn work_area(&self) -> Option<(i32, i32, i32, i32)>;
    fn focus_target_window(&self);
    fn execute(&self) -> CommandExecutionResultDto;
}

impl GuideRuntimeCommand for GuideCommand {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn shortcut_text(&self) -> String {
        self.shortcut_text.clone()
    }

    fn captured_shortcut(&self) -> Option<KeyboardShortcut> {
        GuideCommand::captured_shortcut(self)
    }

    fn work_area(&self) -> Option<(i32, i32, i32, i32)> {
        self.work_area
    }

    fn focus_target_window(&self) {
        GuideCommand::focus_target_window(self);
    }

    fn execute(&self) -> CommandExecutionResultDto {
        GuideCommand::execute(self)
    }
}

trait GuideWindowController: Send + Sync {
    fn show(&self, command: &dyn GuideRuntimeCommand) -> Result<(), String>;
    fn hide(&self) -> Result<(), String>;
}

pub trait PaletteGuideCloser: Send + Sync {
    fn hide_palette_for_guide(&self);
}

impl PaletteGuideCloser for WindowLifecycle {
    fn hide_palette_for_guide(&self) {
        self.hide_palette_window();
    }
}

trait GuideEventSink: Send + Sync {
    fn emit_guide_event(&self, payload: GuideEventPayloadDto) -> Result<(), String>;
}

struct TauriGuideWindowController {
    app: AppHandle,
}

impl TauriGuideWindowController {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn with_window<T, F>(&self, operation_name: &'static str, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(tauri::WebviewWindow) -> Result<T, String> + Send + 'static,
    {
        let app = self.app.clone();
        let (tx, rx) = std::sync::mpsc::channel();

        self.app
            .run_on_main_thread(move || {
                let result = app
                    .get_webview_window(GUIDE_WINDOW_LABEL)
                    .ok_or_else(|| format!("Missing Tauri window '{GUIDE_WINDOW_LABEL}'"))
                    .and_then(operation);
                let _ = tx.send(result);
            })
            .map_err(|err| format!("Failed to schedule {operation_name}: {err}"))?;

        rx.recv()
            .map_err(|err| format!("Failed to receive {operation_name} result: {err}"))?
    }
}

impl GuideWindowController for TauriGuideWindowController {
    fn show(&self, command: &dyn GuideRuntimeCommand) -> Result<(), String> {
        let work_area = command.work_area();
        self.with_window("guide window show", move |window| {
            let size = PhysicalSize::new(GUIDE_WINDOW_WIDTH, GUIDE_WINDOW_HEIGHT);
            window.set_size(size).map_err(|err| err.to_string())?;
            if let Some((left, top, right, bottom)) = work_area {
                let work_width = right.saturating_sub(left);
                let work_height = bottom.saturating_sub(top);
                let x = left + ((work_width - size.width as i32) / 2).max(0);
                let y = top
                    + (((work_height as f32) * GUIDE_VERTICAL_POSITION_FACTOR).round() as i32)
                    - (size.height as i32 / 2);
                window
                    .set_position(PhysicalPosition::new(x, y))
                    .map_err(|err| err.to_string())?;
            } else {
                window.center().map_err(|err| err.to_string())?;
            }
            window
                .set_always_on_top(true)
                .map_err(|err| err.to_string())?;
            window
                .set_ignore_cursor_events(true)
                .map_err(|err| err.to_string())?;
            window.show().map_err(|err| err.to_string())
        })
    }

    fn hide(&self) -> Result<(), String> {
        self.with_window("guide window hide", |window| {
            window.hide().map_err(|err| err.to_string())
        })
    }
}

struct TauriGuideEventSink {
    app: AppHandle,
}

impl TauriGuideEventSink {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl GuideEventSink for TauriGuideEventSink {
    fn emit_guide_event(&self, payload: GuideEventPayloadDto) -> Result<(), String> {
        self.app
            .emit(GUIDE_EVENT_NAME, payload)
            .map_err(|err| format!("Failed to emit guide event: {err}"))
    }
}

pub struct GuideLifecycle {
    status: GuideStatusStore,
    palette_lifecycle: Arc<dyn PaletteGuideCloser>,
    controller: Arc<dyn GuideWindowController>,
    event_sink: Arc<dyn GuideEventSink>,
}

impl GuideLifecycle {
    fn new(
        activation_hint: String,
        palette_lifecycle: Arc<dyn PaletteGuideCloser>,
        controller: Arc<dyn GuideWindowController>,
        event_sink: Arc<dyn GuideEventSink>,
    ) -> Self {
        Self {
            status: GuideStatusStore::new(activation_hint),
            palette_lifecycle,
            controller,
            event_sink,
        }
    }

    pub fn for_tauri(
        activation_hint: String,
        palette_lifecycle: Arc<WindowLifecycle>,
        app: AppHandle,
    ) -> Self {
        let palette_lifecycle: Arc<dyn PaletteGuideCloser> = palette_lifecycle;
        Self::new(
            activation_hint,
            palette_lifecycle,
            Arc::new(TauriGuideWindowController::new(app.clone())),
            Arc::new(TauriGuideEventSink::new(app)),
        )
    }

    pub fn status(&self) -> GuideStatusDto {
        self.status.snapshot()
    }

    pub fn start(&self, command: Arc<dyn GuideRuntimeCommand>) -> GuideStatusDto {
        self.palette_lifecycle.hide_palette_for_guide();
        command.focus_target_window();

        if let Err(err) = self.controller.show(command.as_ref()) {
            self.emit(
                self.status
                    .record_error(format!("Failed to show guide window: {err}")),
            );
            return self.status();
        }

        self.emit(self.status.record_started(command));
        self.status()
    }

    pub fn complete_active(&self) -> Option<CommandExecutionResultDto> {
        let active = self.status.take_active_for_completion()?;
        let result = active.command.execute();
        if let Err(err) = self.controller.hide() {
            self.emit(
                self.status
                    .record_error(format!("Failed to hide guide window: {err}")),
            );
        } else {
            self.emit(self.status.record_completed_event());
        }
        Some(result)
    }

    pub fn cancel_active(&self) -> bool {
        let Some(event) = self.status.record_cancelled() else {
            return false;
        };
        if let Err(err) = self.controller.hide() {
            self.emit(
                self.status
                    .record_error(format!("Failed to hide guide window: {err}")),
            );
        } else {
            self.emit(event);
        }
        true
    }

    pub fn captured_shortcut(&self) -> Option<KeyboardShortcut> {
        self.status
            .active()
            .and_then(|active| active.command.captured_shortcut())
    }

    pub fn active_generation(&self) -> Option<u64> {
        self.status.active().map(|active| active.generation)
    }

    pub fn expire_generation(&self, generation: u64) -> bool {
        let Some(event) = self.status.record_expired(generation) else {
            return false;
        };
        if let Err(err) = self.controller.hide() {
            self.emit(
                self.status
                    .record_error(format!("Failed to hide guide window: {err}")),
            );
        } else {
            self.emit(event);
        }
        true
    }

    pub fn record_start_error(&self, message: String) -> GuideStatusDto {
        self.emit(self.status.record_error(message));
        self.status()
    }

    fn emit(&self, payload: GuideEventPayloadDto) {
        if let Err(err) = self.event_sink.emit_guide_event(payload) {
            let _ = self.status.record_error(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use omni_palette::{
        backend_contract::{CommandExecutionResultDto, CommandExecutionStatus},
        domain::hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
    };

    use super::*;

    #[test]
    fn guide_status_starts_idle() {
        let lifecycle = test_lifecycle();

        assert_eq!(
            lifecycle.status(),
            GuideStatusDto {
                active: false,
                command_label: None,
                shortcut_text: None,
                activation_hint: "Ctrl+Shift+P".to_string(),
                start_count: 0,
                complete_count: 0,
                cancel_count: 0,
                expire_count: 0,
                last_action: None,
                last_error: None,
            }
        );
    }

    #[test]
    fn start_guide_hides_palette_focuses_target_shows_window_and_records_state() {
        let lifecycle = test_lifecycle();
        let command = Arc::new(RecordingGuideCommand::shortcut("Chrome: New tab"));

        let status = lifecycle.start(command.clone());

        assert!(status.active);
        assert_eq!(status.command_label.as_deref(), Some("Chrome: New tab"));
        assert_eq!(status.shortcut_text.as_deref(), Some("Ctrl+T"));
        assert_eq!(status.start_count, 1);
        assert_eq!(
            lifecycle.log(),
            vec!["hide_palette", "show_guide:Chrome: New tab"]
        );
        assert_eq!(lifecycle.events().len(), 1);
        assert_eq!(command.focus_count(), 1);
    }

    #[test]
    fn activation_hotkey_completes_guide_and_executes_stored_command() {
        let lifecycle = test_lifecycle();
        let command = Arc::new(RecordingGuideCommand::shortcut("Chrome: New tab"));
        lifecycle.start(command.clone());

        let result = lifecycle
            .complete_active()
            .expect("active guide should complete");

        assert_eq!(result.status, CommandExecutionStatus::Succeeded);
        assert_eq!(command.execute_count(), 1);
        assert_eq!(lifecycle.status().complete_count, 1);
        assert_eq!(lifecycle.status().active, false);
    }

    #[test]
    fn escape_cancels_without_execution() {
        let lifecycle = test_lifecycle();
        let command = Arc::new(RecordingGuideCommand::shortcut("Chrome: New tab"));
        lifecycle.start(command.clone());

        assert!(lifecycle.cancel_active());

        assert_eq!(command.execute_count(), 0);
        assert_eq!(lifecycle.status().cancel_count, 1);
        assert_eq!(lifecycle.status().active, false);
    }

    #[test]
    fn shortcut_sequence_guide_has_no_captured_shortcut() {
        let lifecycle = test_lifecycle();
        lifecycle.start(Arc::new(RecordingGuideCommand::sequence(
            "VS Code: Open recent",
        )));

        assert_eq!(lifecycle.captured_shortcut(), None);
    }

    #[test]
    fn guide_expiry_closes_active_guide() {
        let lifecycle = test_lifecycle();
        lifecycle.start(Arc::new(RecordingGuideCommand::shortcut("Chrome: New tab")));
        let generation = lifecycle
            .active_generation()
            .expect("active guide should have generation");

        assert!(lifecycle.expire_generation(generation));

        assert_eq!(lifecycle.status().expire_count, 1);
        assert_eq!(lifecycle.status().active, false);
    }

    struct TestGuideLifecycle {
        lifecycle: GuideLifecycle,
        log: Arc<Mutex<Vec<String>>>,
        events: Arc<Mutex<Vec<GuideEventPayloadDto>>>,
    }

    impl TestGuideLifecycle {
        fn start(&self, command: Arc<dyn GuideRuntimeCommand>) -> GuideStatusDto {
            self.lifecycle.start(command)
        }

        fn complete_active(&self) -> Option<CommandExecutionResultDto> {
            self.lifecycle.complete_active()
        }

        fn cancel_active(&self) -> bool {
            self.lifecycle.cancel_active()
        }

        fn captured_shortcut(&self) -> Option<KeyboardShortcut> {
            self.lifecycle.captured_shortcut()
        }

        fn active_generation(&self) -> Option<u64> {
            self.lifecycle.active_generation()
        }

        fn expire_generation(&self, generation: u64) -> bool {
            self.lifecycle.expire_generation(generation)
        }

        fn status(&self) -> GuideStatusDto {
            self.lifecycle.status()
        }

        fn log(&self) -> Vec<String> {
            self.log.lock().expect("log should lock").clone()
        }

        fn events(&self) -> Vec<GuideEventPayloadDto> {
            self.events.lock().expect("events should lock").clone()
        }
    }

    fn test_lifecycle() -> TestGuideLifecycle {
        let log = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let palette_lifecycle = Arc::new(RecordingPaletteCloser {
            log: Arc::clone(&log),
        });
        let controller = Arc::new(RecordingGuideWindowController {
            log: Arc::clone(&log),
        });
        let event_sink = Arc::new(RecordingGuideEventSink {
            events: Arc::clone(&events),
        });

        TestGuideLifecycle {
            lifecycle: GuideLifecycle::new(
                "Ctrl+Shift+P".to_string(),
                palette_lifecycle,
                controller,
                event_sink,
            ),
            log,
            events,
        }
    }

    struct RecordingPaletteCloser {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl PaletteGuideCloser for RecordingPaletteCloser {
        fn hide_palette_for_guide(&self) {
            self.log
                .lock()
                .expect("log should lock")
                .push("hide_palette".to_string());
        }
    }

    struct RecordingGuideWindowController {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl GuideWindowController for RecordingGuideWindowController {
        fn show(&self, command: &dyn GuideRuntimeCommand) -> Result<(), String> {
            self.log
                .lock()
                .expect("log should lock")
                .push(format!("show_guide:{}", command.label()));
            Ok(())
        }

        fn hide(&self) -> Result<(), String> {
            self.log
                .lock()
                .expect("log should lock")
                .push("hide_guide".to_string());
            Ok(())
        }
    }

    struct RecordingGuideEventSink {
        events: Arc<Mutex<Vec<GuideEventPayloadDto>>>,
    }

    impl GuideEventSink for RecordingGuideEventSink {
        fn emit_guide_event(&self, payload: GuideEventPayloadDto) -> Result<(), String> {
            self.events
                .lock()
                .expect("events should lock")
                .push(payload);
            Ok(())
        }
    }

    struct RecordingGuideCommand {
        label: String,
        shortcut: Option<KeyboardShortcut>,
        focus_count: AtomicUsize,
        execute_count: AtomicUsize,
    }

    impl RecordingGuideCommand {
        fn shortcut(label: &str) -> Self {
            Self {
                label: label.to_string(),
                shortcut: Some(ctrl_t()),
                focus_count: AtomicUsize::new(0),
                execute_count: AtomicUsize::new(0),
            }
        }

        fn sequence(label: &str) -> Self {
            Self {
                label: label.to_string(),
                shortcut: None,
                focus_count: AtomicUsize::new(0),
                execute_count: AtomicUsize::new(0),
            }
        }

        fn focus_count(&self) -> usize {
            self.focus_count.load(Ordering::Relaxed)
        }

        fn execute_count(&self) -> usize {
            self.execute_count.load(Ordering::Relaxed)
        }
    }

    impl GuideRuntimeCommand for RecordingGuideCommand {
        fn label(&self) -> String {
            self.label.clone()
        }

        fn shortcut_text(&self) -> String {
            self.shortcut
                .map(|shortcut| shortcut.to_string())
                .unwrap_or_else(|| "Alt+J, I".to_string())
        }

        fn captured_shortcut(&self) -> Option<KeyboardShortcut> {
            self.shortcut
        }

        fn work_area(&self) -> Option<(i32, i32, i32, i32)> {
            Some((0, 0, 1920, 1080))
        }

        fn focus_target_window(&self) {
            self.focus_count.fetch_add(1, Ordering::Relaxed);
        }

        fn execute(&self) -> CommandExecutionResultDto {
            self.execute_count.fetch_add(1, Ordering::Relaxed);
            CommandExecutionResultDto::succeeded(format!("Executed {}", self.label))
        }
    }

    fn ctrl_t() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                ..Default::default()
            },
            key: Key::KeyT,
        }
    }
}
