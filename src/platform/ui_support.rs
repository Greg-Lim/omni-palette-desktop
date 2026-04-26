use std::sync::mpsc::{self, Receiver};

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformWindowToken(isize);

impl PlatformWindowToken {
    pub(crate) const fn new(value: isize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformUiAction {
    OpenPalette,
    OpenSettings,
    ReloadExtensions,
    Quit,
}

pub struct PlatformUiRuntime {
    palette_window_token: Option<PlatformWindowToken>,
    action_rx: Receiver<PlatformUiAction>,
    #[cfg(target_os = "windows")]
    _tray: Option<crate::platform::windows::ui_support::WindowsTray>,
}

impl PlatformUiRuntime {
    pub fn new(cc: &eframe::CreationContext<'_>, egui_ctx: &egui::Context) -> Self {
        let (action_tx, action_rx) = mpsc::channel::<PlatformUiAction>();

        #[cfg(target_os = "windows")]
        let palette_window_token = crate::platform::windows::ui_support::palette_window_token(cc);
        #[cfg(not(target_os = "windows"))]
        let palette_window_token = None;

        #[cfg(target_os = "windows")]
        let tray = crate::platform::windows::ui_support::create_tray(egui_ctx, action_tx)
            .map_err(|err| {
                log::warn!("Could not create tray icon: {err}");
                err
            })
            .ok();
        #[cfg(not(target_os = "windows"))]
        let _ = (cc, egui_ctx, action_tx);

        Self {
            palette_window_token,
            action_rx,
            #[cfg(target_os = "windows")]
            _tray: tray,
        }
    }

    pub fn palette_window_token(&self) -> Option<PlatformWindowToken> {
        self.palette_window_token
    }

    pub fn try_recv_action(&self) -> Option<PlatformUiAction> {
        self.action_rx.try_recv().ok()
    }
}

pub fn foreground_window_token() -> Option<PlatformWindowToken> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::ui_support::foreground_window_token()
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
