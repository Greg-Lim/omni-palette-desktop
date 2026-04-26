use std::sync::mpsc::Sender;

use eframe::egui;
use raw_window_handle::HasWindowHandle;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::platform::ui_support::{PlatformUiAction, PlatformWindowToken};
use crate::platform::windows::context::context::{
    foreground_window_handle_value, get_hwnd_from_raw,
};

pub(crate) struct WindowsTray {
    _tray: TrayIcon,
}

pub(crate) fn palette_window_token(
    cc: &eframe::CreationContext<'_>,
) -> Option<PlatformWindowToken> {
    let raw_window_handle = match cc.window_handle() {
        Ok(handle) => handle.as_raw(),
        Err(err) => {
            log::warn!("Could not obtain palette window handle: {err}");
            return None;
        }
    };
    let hwnd = match get_hwnd_from_raw(raw_window_handle) {
        Some(hwnd) => hwnd,
        None => {
            log::warn!("Palette window did not expose a Win32 window handle");
            return None;
        }
    };
    let hwnd_value = hwnd.0 as isize;
    log::debug!("Captured palette window handle: {:?}", hwnd);
    Some(PlatformWindowToken::new(hwnd_value))
}

pub(crate) fn foreground_window_token() -> Option<PlatformWindowToken> {
    foreground_window_handle_value().map(PlatformWindowToken::new)
}

pub(crate) fn create_tray(
    egui_ctx: &egui::Context,
    action_tx: Sender<PlatformUiAction>,
) -> Result<WindowsTray, String> {
    let menu = Menu::new();
    let open_palette = MenuItem::new("Open Palette", true, None);
    let settings = MenuItem::new("Settings...", true, None);
    let reload = MenuItem::new("Reload Extensions", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append(&open_palette).map_err(|err| err.to_string())?;
    menu.append(&settings).map_err(|err| err.to_string())?;
    menu.append(&reload).map_err(|err| err.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|err| err.to_string())?;
    menu.append(&quit).map_err(|err| err.to_string())?;

    let egui_ctx = egui_ctx.clone();
    let open_palette_id = open_palette.id().clone();
    let settings_id = settings.id().clone();
    let reload_id = reload.id().clone();
    let quit_id = quit.id().clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = if open_palette_id == event.id() {
            Some(PlatformUiAction::OpenPalette)
        } else if settings_id == event.id() {
            Some(PlatformUiAction::OpenSettings)
        } else if reload_id == event.id() {
            Some(PlatformUiAction::ReloadExtensions)
        } else if quit_id == event.id() {
            Some(PlatformUiAction::Quit)
        } else {
            None
        };

        if let Some(action) = action {
            let _ = action_tx.send(action);
            egui_ctx.request_repaint();
        }
    }));

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Omni Palette")
        .with_icon(tray_icon()?)
        .build()
        .map_err(|err| err.to_string())?;

    Ok(WindowsTray { _tray: tray })
}

fn tray_icon() -> Result<Icon, String> {
    let size = 16_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let border = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            let diagonal = x == y || x + y == size - 1;
            let (r, g, b) = if border {
                (230, 230, 230)
            } else if diagonal {
                (66, 153, 225)
            } else {
                (28, 32, 40)
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Icon::from_rgba(rgba, size, size).map_err(|err| err.to_string())
}
