use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::RuntimeSettingsResultStatusDto;

const SETTINGS_WINDOW_LABEL: &str = "settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsWindowStatusDto {
    pub status: RuntimeSettingsResultStatusDto,
    pub message: String,
    pub visible: bool,
    pub show_count: u64,
    pub focus_count: u64,
    pub last_error: Option<String>,
}

impl Default for SettingsWindowStatusDto {
    fn default() -> Self {
        Self {
            status: RuntimeSettingsResultStatusDto::Succeeded,
            message: "Settings window idle".to_string(),
            visible: false,
            show_count: 0,
            focus_count: 0,
            last_error: None,
        }
    }
}

pub(crate) trait SettingsWindowController: Send + Sync {
    fn show(&self) -> Result<(), String>;
    fn focus(&self) -> Result<(), String>;
}

pub(crate) struct SettingsWindow {
    status: Mutex<SettingsWindowStatusDto>,
    controller: Arc<dyn SettingsWindowController>,
}

impl SettingsWindow {
    pub(crate) fn new(controller: Arc<dyn SettingsWindowController>) -> Self {
        Self {
            status: Mutex::new(SettingsWindowStatusDto::default()),
            controller,
        }
    }

    pub(crate) fn for_tauri(app: AppHandle) -> Self {
        Self::new(Arc::new(TauriSettingsWindowController::new(app)))
    }

    pub(crate) fn show_settings_window(&self) -> SettingsWindowStatusDto {
        if let Err(err) = self.controller.show() {
            return self.record_error(format!("Failed to show settings window: {err}"));
        }

        {
            let mut status = self.status.lock().expect("settings status should lock");
            status.visible = true;
            status.show_count += 1;
        }

        if let Err(err) = self.controller.focus() {
            return self.record_error(format!("Failed to focus settings window: {err}"));
        }

        let mut status = self.status.lock().expect("settings status should lock");
        status.status = RuntimeSettingsResultStatusDto::Succeeded;
        status.message = "Settings window shown".to_string();
        status.focus_count += 1;
        status.last_error = None;
        status.clone()
    }

    fn record_error(&self, message: String) -> SettingsWindowStatusDto {
        let mut status = self.status.lock().expect("settings status should lock");
        status.status = RuntimeSettingsResultStatusDto::Failed;
        status.message = message.clone();
        status.last_error = Some(message);
        status.clone()
    }
}

struct TauriSettingsWindowController {
    app: AppHandle,
}

impl TauriSettingsWindowController {
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
                    .get_webview_window(SETTINGS_WINDOW_LABEL)
                    .ok_or_else(|| format!("Missing Tauri window '{SETTINGS_WINDOW_LABEL}'"))
                    .and_then(operation);
                let _ = tx.send(result);
            })
            .map_err(|err| format!("Failed to schedule {operation_name}: {err}"))?;

        rx.recv()
            .map_err(|err| format!("Failed to receive {operation_name} result: {err}"))?
    }
}

impl SettingsWindowController for TauriSettingsWindowController {
    fn show(&self) -> Result<(), String> {
        self.with_window("settings window show", |window| {
            window.show().map_err(|err| err.to_string())
        })
    }

    fn focus(&self) -> Result<(), String> {
        self.with_window("settings window focus", |window| {
            if window.is_minimized().map_err(|err| err.to_string())? {
                window.unminimize().map_err(|err| err.to_string())?;
            }
            window.set_focus().map_err(|err| err.to_string())
        })
    }
}
