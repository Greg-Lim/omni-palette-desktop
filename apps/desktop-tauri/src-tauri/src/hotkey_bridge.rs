use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread::{self, JoinHandle},
};

use omni_palette::{domain::hotkey::KeyboardShortcut, runtime_state::OmniRuntimeState};
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
}

trait ActiveProcessProvider: Send + Sync {
    fn active_process_name(&self) -> Option<String>;
}

#[cfg(target_os = "windows")]
struct WindowsActiveProcessProvider;

#[cfg(target_os = "windows")]
impl ActiveProcessProvider for WindowsActiveProcessProvider {
    fn active_process_name(&self) -> Option<String> {
        get_all_context()
            .get_active()
            .and_then(|handle| handle.get_app_process_name())
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
    handle: Mutex<Option<Box<dyn StoppableHotkeyListener>>>,
    bridge_thread: Mutex<Option<JoinHandle<()>>>,
}

impl HotkeyBridge {
    pub fn start(runtime_state: OmniRuntimeState, app: AppHandle) -> Self {
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
                        Arc::new(WindowsActiveProcessProvider),
                    )
                }
                Err(err) => Self::failed(activation_hint, err),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (runtime_state, app);
            Self::failed(
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
        active_process_provider: Arc<dyn ActiveProcessProvider>,
    ) -> Self {
        let status = HotkeyStatusStore::new(activation_hint);
        status.record_running();
        let loop_status = status.clone();
        let loop_forwarder = Arc::clone(&forwarder);
        let loop_event_sink = Arc::clone(&event_sink);

        let bridge_thread = thread::spawn(move || {
            while let Ok(shortcut) = rx.recv() {
                let active_process_name = active_process_provider.active_process_name();
                handle_hotkey_event(
                    shortcut,
                    &runtime_state,
                    active_process_name,
                    Arc::clone(&loop_forwarder),
                    Arc::clone(&loop_event_sink),
                    &loop_status,
                );
            }

            let payload = loop_status.record_stopped_error("Hotkey listener stopped".to_string());
            let _ = loop_event_sink.emit_hotkey_event(payload);
        });

        Self {
            status,
            handle: Mutex::new(Some(handle)),
            bridge_thread: Mutex::new(Some(bridge_thread)),
        }
    }

    fn failed(activation_hint: String, message: String) -> Self {
        let status = HotkeyStatusStore::new(activation_hint);
        status.record_error(message);
        Self {
            status,
            handle: Mutex::new(None),
            bridge_thread: Mutex::new(None),
        }
    }

    pub fn status(&self) -> HotkeyStatusDto {
        self.status.snapshot()
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
    active_process_name: Option<String>,
    forwarder: Arc<F>,
    event_sink: Arc<E>,
    status: &HotkeyStatusStore,
) where
    F: HotkeyForwarder + ?Sized,
    E: HotkeyEventSink + ?Sized,
{
    let payload = if active_process_name
        .as_deref()
        .is_some_and(|process_name| runtime_state.is_ignored_process_name(process_name))
    {
        if let Err(err) = forwarder.forward_shortcut(shortcut) {
            let payload = status.record_error(format!("Failed to forward ignored hotkey: {err}"));
            let _ = event_sink.emit_hotkey_event(payload);
            return;
        }
        status.record_ignored_passthrough(shortcut, active_process_name)
    } else {
        status.record_activation(shortcut, active_process_name)
    };

    if let Err(err) = event_sink.emit_hotkey_event(payload) {
        status.record_error(err);
    }
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
        let shortcut = activation_shortcut();

        handle_hotkey_event(
            shortcut,
            &runtime,
            Some("notepad.exe".to_string()),
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            &status,
        );

        let snapshot = status.snapshot();
        assert_eq!(snapshot.activation_count, 1);
        assert_eq!(snapshot.ignored_passthrough_count, 0);
        assert_eq!(forwarder.forwarded_shortcuts(), Vec::<String>::new());
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
        let shortcut = activation_shortcut();

        handle_hotkey_event(
            shortcut,
            &runtime,
            Some("Code.exe".to_string()),
            Arc::clone(&forwarder),
            Arc::clone(&sink),
            &status,
        );

        let snapshot = status.snapshot();
        assert_eq!(snapshot.activation_count, 0);
        assert_eq!(snapshot.ignored_passthrough_count, 1);
        assert_eq!(
            forwarder.forwarded_shortcuts(),
            vec!["Ctrl+Shift+P".to_string()]
        );
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
    fn listener_startup_failure_records_controlled_error() {
        let bridge = HotkeyBridge::failed(
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
    struct RecordingForwarder {
        shortcuts: Mutex<Vec<String>>,
    }

    impl RecordingForwarder {
        fn forwarded_shortcuts(&self) -> Vec<String> {
            self.shortcuts
                .lock()
                .expect("shortcuts should lock")
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
