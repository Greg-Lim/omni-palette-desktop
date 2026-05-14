use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread::{self, JoinHandle},
};

use omni_palette::{
    domain::{
        action::ContextRoot,
        hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
    },
    runtime_state::OmniRuntimeState,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use omni_palette::platform::{
    hotkey_actions::{start_hotkey_listener, HotkeyHandle, HotkeyPassthrough},
    platform_interface::{get_all_context, RawWindowHandleExt},
};

pub const HOTKEY_EVENT_NAME: &str = "omni://palette-activation-requested";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyStatusDto {
    pub running: bool,
    pub activation_hint: String,
    pub activation_count: u64,
    pub ignored_passthrough_count: u64,
    pub last_event: Option<HotkeyEventPayloadDto>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyEventPayloadDto {
    pub kind: HotkeyEventKindDto,
    pub shortcut: String,
    pub process_name: Option<String>,
    pub activation_count: u64,
    pub ignored_passthrough_count: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyEventKindDto {
    ActivationRequested,
    IgnoredPassthrough,
    ListenerError,
}

#[derive(Clone)]
struct HotkeyStatusStore {
    inner: Arc<Mutex<HotkeyStatusState>>,
}

#[derive(Debug)]
struct HotkeyStatusState {
    running: bool,
    activation_hint: String,
    activation_count: u64,
    ignored_passthrough_count: u64,
    last_event: Option<HotkeyEventPayloadDto>,
    last_error: Option<String>,
}

impl HotkeyStatusStore {
    fn new(activation_hint: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HotkeyStatusState {
                running: false,
                activation_hint,
                activation_count: 0,
                ignored_passthrough_count: 0,
                last_event: None,
                last_error: None,
            })),
        }
    }

    fn snapshot(&self) -> HotkeyStatusDto {
        let state = self.inner.lock().expect("hotkey status should lock");
        HotkeyStatusDto {
            running: state.running,
            activation_hint: state.activation_hint.clone(),
            activation_count: state.activation_count,
            ignored_passthrough_count: state.ignored_passthrough_count,
            last_event: state.last_event.clone(),
            last_error: state.last_error.clone(),
        }
    }

    fn record_running(&self) {
        let mut state = self.inner.lock().expect("hotkey status should lock");
        state.running = true;
        state.last_error = None;
    }

    fn record_activation(
        &self,
        shortcut: KeyboardShortcut,
        process_name: Option<String>,
    ) -> HotkeyEventPayloadDto {
        let mut state = self.inner.lock().expect("hotkey status should lock");
        state.activation_count += 1;
        state.last_error = None;
        let payload = HotkeyEventPayloadDto {
            kind: HotkeyEventKindDto::ActivationRequested,
            shortcut: shortcut.to_string(),
            process_name,
            activation_count: state.activation_count,
            ignored_passthrough_count: state.ignored_passthrough_count,
            message: None,
        };
        state.last_event = Some(payload.clone());
        payload
    }

    fn record_ignored_passthrough(
        &self,
        shortcut: KeyboardShortcut,
        process_name: Option<String>,
    ) -> HotkeyEventPayloadDto {
        let mut state = self.inner.lock().expect("hotkey status should lock");
        state.ignored_passthrough_count += 1;
        state.last_error = None;
        let payload = HotkeyEventPayloadDto {
            kind: HotkeyEventKindDto::IgnoredPassthrough,
            shortcut: shortcut.to_string(),
            process_name,
            activation_count: state.activation_count,
            ignored_passthrough_count: state.ignored_passthrough_count,
            message: None,
        };
        state.last_event = Some(payload.clone());
        payload
    }

    fn record_error(&self, message: String) -> HotkeyEventPayloadDto {
        let mut state = self.inner.lock().expect("hotkey status should lock");
        state.last_error = Some(message.clone());
        let payload = HotkeyEventPayloadDto {
            kind: HotkeyEventKindDto::ListenerError,
            shortcut: state.activation_hint.clone(),
            process_name: None,
            activation_count: state.activation_count,
            ignored_passthrough_count: state.ignored_passthrough_count,
            message: Some(message),
        };
        state.last_event = Some(payload.clone());
        payload
    }

    fn record_stopped_error(&self, message: String) -> HotkeyEventPayloadDto {
        {
            let mut state = self.inner.lock().expect("hotkey status should lock");
            state.running = false;
        }
        self.record_error(message)
    }
}

trait HotkeyEventSink: Send + Sync {
    fn emit_hotkey_event(&self, payload: HotkeyEventPayloadDto) -> Result<(), String>;
}

pub trait PaletteActivationHandler: Send + Sync {
    fn handle_palette_activation(&self, context: ContextRoot);
    fn handle_guide_activation(&self) -> bool {
        false
    }
    fn handle_guide_cancel(&self, _shortcut: KeyboardShortcut) -> bool {
        false
    }
    fn handle_guide_shortcut(&self, _shortcut: KeyboardShortcut) -> bool {
        false
    }
}

struct TauriHotkeyEventSink {
    app: AppHandle,
}

impl TauriHotkeyEventSink {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl HotkeyEventSink for TauriHotkeyEventSink {
    fn emit_hotkey_event(&self, payload: HotkeyEventPayloadDto) -> Result<(), String> {
        self.app
            .emit(HOTKEY_EVENT_NAME, payload)
            .map_err(|err| format!("Failed to emit hotkey event: {err}"))
    }
}

trait HotkeyForwarder: Send + Sync {
    fn forward_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String>;
    fn forward_guide_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
        self.forward_shortcut(shortcut)
    }
    fn set_guide_cancel_hotkey(&self, _enabled: bool) -> Result<(), String> {
        Ok(())
    }
    fn set_guide_shortcut_hotkey(&self, _shortcut: Option<KeyboardShortcut>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
struct WindowsHotkeyForwarder {
    passthrough: HotkeyPassthrough,
}

#[cfg(target_os = "windows")]
impl HotkeyForwarder for WindowsHotkeyForwarder {
    fn forward_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
        self.passthrough.forward_shortcut(shortcut);
        Ok(())
    }

    fn forward_guide_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
        self.passthrough.forward_guide_shortcut(shortcut);
        Ok(())
    }

    fn set_guide_cancel_hotkey(&self, enabled: bool) -> Result<(), String> {
        self.passthrough.set_guide_cancel_hotkey(enabled);
        Ok(())
    }

    fn set_guide_shortcut_hotkey(&self, shortcut: Option<KeyboardShortcut>) -> Result<(), String> {
        self.passthrough.set_guide_shortcut_hotkey(shortcut);
        Ok(())
    }
}

struct ActiveWindowContext {
    context: ContextRoot,
    process_name: Option<String>,
}

trait ActiveProcessProvider: Send + Sync {
    fn active_window_context(&self) -> ActiveWindowContext;
}

#[cfg(target_os = "windows")]
struct WindowsActiveProcessProvider;

#[cfg(target_os = "windows")]
impl ActiveProcessProvider for WindowsActiveProcessProvider {
    fn active_window_context(&self) -> ActiveWindowContext {
        let context = get_all_context();
        let process_name = context
            .get_active()
            .and_then(|handle| handle.get_app_process_name());
        ActiveWindowContext {
            context,
            process_name,
        }
    }
}

trait StoppableHotkeyListener: Send {
    fn stop(self: Box<Self>);
}

#[cfg(target_os = "windows")]
struct WindowsHotkeyListenerHandle {
    handle: HotkeyHandle,
}

#[cfg(target_os = "windows")]
impl StoppableHotkeyListener for WindowsHotkeyListenerHandle {
    fn stop(self: Box<Self>) {
        self.handle.stop();
    }
}

pub struct HotkeyBridge {
    status: HotkeyStatusStore,
    activation_shortcut: KeyboardShortcut,
    forwarder: Option<Arc<dyn HotkeyForwarder>>,
    handle: Mutex<Option<Box<dyn StoppableHotkeyListener>>>,
    bridge_thread: Mutex<Option<JoinHandle<()>>>,
}

impl HotkeyBridge {
    pub fn start(
        runtime_state: OmniRuntimeState,
        app: AppHandle,
        activation_handler: Arc<dyn PaletteActivationHandler>,
    ) -> Self {
        let activation_shortcut = runtime_state.config().activation;
        let activation_hint = activation_shortcut.to_string();

        #[cfg(target_os = "windows")]
        {
            let start_result =
                std::panic::catch_unwind(|| start_hotkey_listener(activation_shortcut))
                    .map_err(|_| "Hotkey listener panicked during startup".to_string());

            match start_result {
                Ok((handle, rx)) => {
                    let forwarder = Arc::new(WindowsHotkeyForwarder {
                        passthrough: handle.passthrough_sender(),
                    });
                    let listener_handle = Box::new(WindowsHotkeyListenerHandle { handle });
                    Self::from_started(
                        activation_hint,
                        listener_handle,
                        rx,
                        runtime_state,
                        forwarder,
                        Arc::new(TauriHotkeyEventSink::new(app)),
                        activation_handler,
                        Arc::new(WindowsActiveProcessProvider),
                    )
                }
                Err(err) => Self::failed(activation_shortcut, activation_hint, err),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (runtime_state, app, activation_handler);
            Self::failed(
                activation_shortcut,
                activation_hint,
                "Global hotkey listener is only available on Windows".to_string(),
            )
        }
    }

    fn from_started(
        activation_hint: String,
        handle: Box<dyn StoppableHotkeyListener>,
        rx: Receiver<KeyboardShortcut>,
        runtime_state: OmniRuntimeState,
        forwarder: Arc<dyn HotkeyForwarder>,
        event_sink: Arc<dyn HotkeyEventSink>,
        activation_handler: Arc<dyn PaletteActivationHandler>,
        active_process_provider: Arc<dyn ActiveProcessProvider>,
    ) -> Self {
        let status = HotkeyStatusStore::new(activation_hint);
        status.record_running();
        let activation_shortcut = runtime_state.config().activation;
        let loop_status = status.clone();
        let loop_forwarder = Arc::clone(&forwarder);
        let loop_event_sink = Arc::clone(&event_sink);
        let loop_activation_handler = Arc::clone(&activation_handler);

        let bridge_thread = thread::spawn(move || {
            while let Ok(shortcut) = rx.recv() {
                let active_context = active_process_provider.active_window_context();
                handle_hotkey_event(
                    shortcut,
                    &runtime_state,
                    active_context,
                    Arc::clone(&loop_forwarder),
                    Arc::clone(&loop_event_sink),
                    Arc::clone(&loop_activation_handler),
                    &loop_status,
                );
            }

            let payload = loop_status.record_stopped_error("Hotkey listener stopped".to_string());
            let _ = loop_event_sink.emit_hotkey_event(payload);
        });

        Self {
            status,
            activation_shortcut,
            forwarder: Some(forwarder),
            handle: Mutex::new(Some(handle)),
            bridge_thread: Mutex::new(Some(bridge_thread)),
        }
    }

    fn failed(
        activation_shortcut: KeyboardShortcut,
        activation_hint: String,
        message: String,
    ) -> Self {
        let status = HotkeyStatusStore::new(activation_hint);
        status.record_error(message);
        Self {
            status,
            activation_shortcut,
            forwarder: None,
            handle: Mutex::new(None),
            bridge_thread: Mutex::new(None),
        }
    }

    pub fn status(&self) -> HotkeyStatusDto {
        self.status.snapshot()
    }

    pub fn enable_guide_hotkeys(
        &self,
        captured_shortcut: Option<KeyboardShortcut>,
    ) -> Result<(), String> {
        let Some(forwarder) = &self.forwarder else {
            return Ok(());
        };
        let captured_shortcut =
            captured_shortcut.filter(|shortcut| *shortcut != self.activation_shortcut);
        forwarder.set_guide_cancel_hotkey(true)?;
        forwarder.set_guide_shortcut_hotkey(captured_shortcut)
    }

    pub fn clear_guide_hotkeys(&self) -> Result<(), String> {
        let Some(forwarder) = &self.forwarder else {
            return Ok(());
        };
        forwarder.set_guide_shortcut_hotkey(None)?;
        forwarder.set_guide_cancel_hotkey(false)
    }
}

impl Drop for HotkeyBridge {
    fn drop(&mut self) {
        if let Some(handle) = self
            .handle
            .lock()
            .expect("hotkey handle should lock")
            .take()
        {
            handle.stop();
        }

        if let Some(thread) = self
            .bridge_thread
            .lock()
            .expect("hotkey bridge thread should lock")
            .take()
        {
            let _ = thread.join();
        }
    }
}

fn handle_hotkey_event<F, E>(
    shortcut: KeyboardShortcut,
    runtime_state: &OmniRuntimeState,
    active_context: ActiveWindowContext,
    forwarder: Arc<F>,
    event_sink: Arc<E>,
    activation_handler: Arc<dyn PaletteActivationHandler>,
    status: &HotkeyStatusStore,
) where
    F: HotkeyForwarder + ?Sized,
    E: HotkeyEventSink + ?Sized,
{
    if is_guide_cancel_shortcut(shortcut) && activation_handler.handle_guide_cancel(shortcut) {
        let _ = forwarder.set_guide_shortcut_hotkey(None);
        let _ = forwarder.set_guide_cancel_hotkey(false);
        return;
    }

    if activation_handler.handle_guide_shortcut(shortcut) {
        let _ = forwarder.set_guide_shortcut_hotkey(None);
        let _ = forwarder.set_guide_cancel_hotkey(false);
        if let Err(err) = forwarder.forward_guide_shortcut(shortcut) {
            let payload = status.record_error(format!("Failed to forward guide shortcut: {err}"));
            let _ = event_sink.emit_hotkey_event(payload);
        }
        return;
    }

    if activation_handler.handle_guide_activation() {
        let _ = forwarder.set_guide_shortcut_hotkey(None);
        let _ = forwarder.set_guide_cancel_hotkey(false);
        return;
    }

    let payload = if active_context
        .process_name
        .as_deref()
        .is_some_and(|process_name| runtime_state.is_ignored_process_name(process_name))
    {
        if let Err(err) = forwarder.forward_shortcut(shortcut) {
            let payload = status.record_error(format!("Failed to forward ignored hotkey: {err}"));
            let _ = event_sink.emit_hotkey_event(payload);
            return;
        }
        status.record_ignored_passthrough(shortcut, active_context.process_name)
    } else {
        let payload = status.record_activation(shortcut, active_context.process_name);
        activation_handler.handle_palette_activation(active_context.context);
        payload
    };

    if let Err(err) = event_sink.emit_hotkey_event(payload) {
        status.record_error(err);
    }
}

fn is_guide_cancel_shortcut(shortcut: KeyboardShortcut) -> bool {
    shortcut.modifier == HotkeyModifiers::default() && shortcut.key == Key::Escape
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use omni_palette::{
        config::runtime::RuntimePaths,
        domain::{
            action::Os,
            hotkey::{HotkeyModifiers, Key, KeyboardShortcut},
        },
        runtime_state::{OmniRuntimeState, RuntimeStateLoadOptions},
    };

    use super::*;

    #[test]
    fn hotkey_status_starts_not_running() {
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());

        assert_eq!(
            status.snapshot(),
            HotkeyStatusDto {
                running: false,
                activation_hint: "Ctrl+Shift+P".to_string(),
                activation_count: 0,
                ignored_passthrough_count: 0,
                last_event: None,
                last_error: None,
            }
        );
    }

    #[test]
    fn starting_bridge_records_listener_running_status() {
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());

        status.record_running();

        assert!(status.snapshot().running);
    }

    #[test]
    fn non_ignored_activation_records_event_without_forwarding() {
        let runtime = runtime_with_ignored_processes(&["code.exe"]);
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());
        status.record_running();
        let sink = Arc::new(RecordingEventSink::default());
        let forwarder = Arc::new(RecordingForwarder::default());
        let activation_handler = Arc::new(RecordingActivationHandler::default());
        let activation_handler_trait: Arc<dyn PaletteActivationHandler> =
            activation_handler.clone();
        let shortcut = activation_shortcut();

        handle_hotkey_event(
            shortcut,
            &runtime,
            ActiveWindowContext {
                context: empty_context(),
                process_name: Some("notepad.exe".to_string()),
            },
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            activation_handler_trait,
            &status,
        );

        let snapshot = status.snapshot();
        assert_eq!(snapshot.activation_count, 1);
        assert_eq!(snapshot.ignored_passthrough_count, 0);
        assert_eq!(forwarder.forwarded_shortcuts(), Vec::<String>::new());
        assert_eq!(activation_handler.activation_count(), 1);
        assert_eq!(
            sink.events(),
            vec![HotkeyEventPayloadDto {
                kind: HotkeyEventKindDto::ActivationRequested,
                shortcut: "Ctrl+Shift+P".to_string(),
                process_name: Some("notepad.exe".to_string()),
                activation_count: 1,
                ignored_passthrough_count: 0,
                message: None,
            }]
        );
    }

    #[test]
    fn ignored_foreground_process_records_passthrough_and_forwards_shortcut() {
        let runtime = runtime_with_ignored_processes(&["code.exe"]);
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());
        status.record_running();
        let sink = Arc::new(RecordingEventSink::default());
        let forwarder = Arc::new(RecordingForwarder::default());
        let activation_handler = Arc::new(RecordingActivationHandler::default());
        let activation_handler_trait: Arc<dyn PaletteActivationHandler> =
            activation_handler.clone();
        let shortcut = activation_shortcut();

        handle_hotkey_event(
            shortcut,
            &runtime,
            ActiveWindowContext {
                context: empty_context(),
                process_name: Some("Code.exe".to_string()),
            },
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            activation_handler_trait,
            &status,
        );

        let snapshot = status.snapshot();
        assert_eq!(snapshot.activation_count, 0);
        assert_eq!(snapshot.ignored_passthrough_count, 1);
        assert_eq!(
            forwarder.forwarded_shortcuts(),
            vec!["Ctrl+Shift+P".to_string()]
        );
        assert_eq!(activation_handler.activation_count(), 0);
        assert_eq!(
            sink.events(),
            vec![HotkeyEventPayloadDto {
                kind: HotkeyEventKindDto::IgnoredPassthrough,
                shortcut: "Ctrl+Shift+P".to_string(),
                process_name: Some("Code.exe".to_string()),
                activation_count: 0,
                ignored_passthrough_count: 1,
                message: None,
            }]
        );
    }

    #[test]
    fn guide_activation_executes_active_guide_without_palette_activation() {
        let runtime = runtime_with_ignored_processes(&[]);
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());
        status.record_running();
        let sink = Arc::new(RecordingEventSink::default());
        let forwarder = Arc::new(RecordingForwarder::default());
        let activation_handler = Arc::new(RecordingActivationHandler::with_guide_activation());
        let activation_handler_trait: Arc<dyn PaletteActivationHandler> =
            activation_handler.clone();

        handle_hotkey_event(
            activation_shortcut(),
            &runtime,
            ActiveWindowContext {
                context: empty_context(),
                process_name: Some("notepad.exe".to_string()),
            },
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            activation_handler_trait,
            &status,
        );

        assert_eq!(activation_handler.guide_activation_count(), 1);
        assert_eq!(activation_handler.activation_count(), 0);
        assert_eq!(status.snapshot().activation_count, 0);
        assert_eq!(
            forwarder.guide_control_calls(),
            vec!["set_guide_shortcut:none", "set_guide_cancel:false"]
        );
        assert_eq!(sink.events(), Vec::new());
    }

    #[test]
    fn captured_guide_shortcut_cancels_guide_and_forwards_shortcut() {
        let runtime = runtime_with_ignored_processes(&[]);
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());
        status.record_running();
        let sink = Arc::new(RecordingEventSink::default());
        let forwarder = Arc::new(RecordingForwarder::default());
        let activation_handler = Arc::new(RecordingActivationHandler::with_guide_shortcut());
        let activation_handler_trait: Arc<dyn PaletteActivationHandler> =
            activation_handler.clone();
        let shortcut = ctrl_t_shortcut();

        handle_hotkey_event(
            shortcut,
            &runtime,
            ActiveWindowContext {
                context: empty_context(),
                process_name: Some("notepad.exe".to_string()),
            },
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            activation_handler_trait,
            &status,
        );

        assert_eq!(activation_handler.guide_shortcut_count(), 1);
        assert_eq!(
            forwarder.guide_control_calls(),
            vec![
                "set_guide_shortcut:none",
                "set_guide_cancel:false",
                "forward_guide:Ctrl+T",
            ]
        );
        assert_eq!(status.snapshot().activation_count, 0);
        assert_eq!(sink.events(), Vec::new());
    }

    #[test]
    fn guide_escape_cancels_without_forwarding() {
        let runtime = runtime_with_ignored_processes(&[]);
        let status = HotkeyStatusStore::new("Ctrl+Shift+P".to_string());
        status.record_running();
        let sink = Arc::new(RecordingEventSink::default());
        let forwarder = Arc::new(RecordingForwarder::default());
        let activation_handler = Arc::new(RecordingActivationHandler::with_guide_cancel());
        let activation_handler_trait: Arc<dyn PaletteActivationHandler> =
            activation_handler.clone();

        handle_hotkey_event(
            escape_shortcut(),
            &runtime,
            ActiveWindowContext {
                context: empty_context(),
                process_name: Some("notepad.exe".to_string()),
            },
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            activation_handler_trait,
            &status,
        );

        assert_eq!(activation_handler.guide_cancel_count(), 1);
        assert_eq!(
            forwarder.guide_control_calls(),
            vec!["set_guide_shortcut:none", "set_guide_cancel:false"]
        );
        assert_eq!(forwarder.forwarded_shortcuts(), Vec::<String>::new());
        assert_eq!(sink.events(), Vec::new());
    }

    #[test]
    fn listener_startup_failure_records_controlled_error() {
        let bridge = HotkeyBridge::failed(
            activation_shortcut(),
            "Ctrl+Shift+P".to_string(),
            "failed to register hotkey".to_string(),
        );

        assert_eq!(
            bridge.status(),
            HotkeyStatusDto {
                running: false,
                activation_hint: "Ctrl+Shift+P".to_string(),
                activation_count: 0,
                ignored_passthrough_count: 0,
                last_event: Some(HotkeyEventPayloadDto {
                    kind: HotkeyEventKindDto::ListenerError,
                    shortcut: "Ctrl+Shift+P".to_string(),
                    process_name: None,
                    activation_count: 0,
                    ignored_passthrough_count: 0,
                    message: Some("failed to register hotkey".to_string()),
                }),
                last_error: Some("failed to register hotkey".to_string()),
            }
        );
    }

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<HotkeyEventPayloadDto>>,
    }

    impl RecordingEventSink {
        fn events(&self) -> Vec<HotkeyEventPayloadDto> {
            self.events.lock().expect("events should lock").clone()
        }
    }

    impl HotkeyEventSink for RecordingEventSink {
        fn emit_hotkey_event(&self, payload: HotkeyEventPayloadDto) -> Result<(), String> {
            self.events
                .lock()
                .expect("events should lock")
                .push(payload);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingActivationHandler {
        count: Mutex<u64>,
        guide_activation_count: Mutex<u64>,
        guide_cancel_count: Mutex<u64>,
        guide_shortcut_count: Mutex<u64>,
        guide_activation_result: bool,
        guide_cancel_result: bool,
        guide_shortcut_result: bool,
    }

    impl RecordingActivationHandler {
        fn with_guide_activation() -> Self {
            Self {
                guide_activation_result: true,
                ..Default::default()
            }
        }

        fn with_guide_cancel() -> Self {
            Self {
                guide_cancel_result: true,
                ..Default::default()
            }
        }

        fn with_guide_shortcut() -> Self {
            Self {
                guide_shortcut_result: true,
                ..Default::default()
            }
        }

        fn activation_count(&self) -> u64 {
            *self.count.lock().expect("count should lock")
        }

        fn guide_activation_count(&self) -> u64 {
            *self
                .guide_activation_count
                .lock()
                .expect("guide count should lock")
        }

        fn guide_cancel_count(&self) -> u64 {
            *self
                .guide_cancel_count
                .lock()
                .expect("guide count should lock")
        }

        fn guide_shortcut_count(&self) -> u64 {
            *self
                .guide_shortcut_count
                .lock()
                .expect("guide count should lock")
        }
    }

    impl PaletteActivationHandler for RecordingActivationHandler {
        fn handle_palette_activation(&self, _context: omni_palette::domain::action::ContextRoot) {
            *self.count.lock().expect("count should lock") += 1;
        }

        fn handle_guide_activation(&self) -> bool {
            *self
                .guide_activation_count
                .lock()
                .expect("guide count should lock") += 1;
            self.guide_activation_result
        }

        fn handle_guide_cancel(&self, _shortcut: KeyboardShortcut) -> bool {
            *self
                .guide_cancel_count
                .lock()
                .expect("guide count should lock") += 1;
            self.guide_cancel_result
        }

        fn handle_guide_shortcut(&self, _shortcut: KeyboardShortcut) -> bool {
            *self
                .guide_shortcut_count
                .lock()
                .expect("guide count should lock") += 1;
            self.guide_shortcut_result
        }
    }

    #[derive(Default)]
    struct RecordingForwarder {
        shortcuts: Mutex<Vec<String>>,
        guide_calls: Mutex<Vec<String>>,
    }

    impl RecordingForwarder {
        fn forwarded_shortcuts(&self) -> Vec<String> {
            self.shortcuts
                .lock()
                .expect("shortcuts should lock")
                .clone()
        }

        fn guide_control_calls(&self) -> Vec<String> {
            self.guide_calls
                .lock()
                .expect("guide calls should lock")
                .clone()
        }
    }

    impl HotkeyForwarder for RecordingForwarder {
        fn forward_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
            self.shortcuts
                .lock()
                .expect("shortcuts should lock")
                .push(shortcut.to_string());
            Ok(())
        }

        fn forward_guide_shortcut(&self, shortcut: KeyboardShortcut) -> Result<(), String> {
            self.guide_calls
                .lock()
                .expect("guide calls should lock")
                .push(format!("forward_guide:{shortcut}"));
            Ok(())
        }

        fn set_guide_cancel_hotkey(&self, enabled: bool) -> Result<(), String> {
            self.guide_calls
                .lock()
                .expect("guide calls should lock")
                .push(format!("set_guide_cancel:{enabled}"));
            Ok(())
        }

        fn set_guide_shortcut_hotkey(
            &self,
            shortcut: Option<KeyboardShortcut>,
        ) -> Result<(), String> {
            let shortcut = shortcut
                .map(|shortcut| shortcut.to_string())
                .unwrap_or_else(|| "none".to_string());
            self.guide_calls
                .lock()
                .expect("guide calls should lock")
                .push(format!("set_guide_shortcut:{shortcut}"));
            Ok(())
        }
    }

    fn empty_context() -> omni_palette::domain::action::ContextRoot {
        omni_palette::domain::action::ContextRoot {
            fg_context: Vec::new(),
            bg_context: Vec::new(),
            active_interaction: omni_palette::domain::action::InteractionContext::default(),
        }
    }

    fn activation_shortcut() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            key: Key::KeyP,
        }
    }

    fn ctrl_t_shortcut() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers {
                control: true,
                ..Default::default()
            },
            key: Key::KeyT,
        }
    }

    fn escape_shortcut() -> KeyboardShortcut {
        KeyboardShortcut {
            modifier: HotkeyModifiers::default(),
            key: Key::Escape,
        }
    }

    fn runtime_with_ignored_processes(process_names: &[&str]) -> OmniRuntimeState {
        let root = runtime_test_root("hotkey-bridge-ignored");
        let names = process_names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        std::fs::write(root.join("ignore.toml"), format!("windows = [{names}]"))
            .expect("ignore config should be written");

        OmniRuntimeState::load(RuntimeStateLoadOptions {
            bundled_extensions_root: root.clone(),
            user_extensions_root: None,
            dev_config_path: root.join("config.toml"),
            runtime_paths: RuntimePaths {
                config_path: None,
                local_cache_root: None,
            },
            current_os: Os::Windows,
        })
    }

    fn runtime_test_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from("target")
            .join("tauri-hotkey-bridge-tests")
            .join(format!("{name}-{nanos}"));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("runtime test root should reset");
        }
        std::fs::create_dir_all(root.join("static")).expect("static dir should be created");
        root
    }
}
