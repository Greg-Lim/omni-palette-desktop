use crate::{
    models::action::{AppProcessName, ContextRoot},
    platform::windows as platwins,
};
use cfg_if::cfg_if;
use log::error;
use raw_window_handle::RawWindowHandle;

pub fn get_all_context() -> ContextRoot {
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            // Destructure the tuple returned by your Windows function
            let (fg, bg) = platwins::context::context::get_all_windows();
        } else {
            // Fallback for other OSs
            panic!("Not valid os")
            let fg = vec![];
            let bg = vec![];
        }
    }

    ContextRoot {
        fg_context: fg,
        bg_context: bg,
    }
}

pub trait RawWindowHandleExt {
    fn get_app_process_name(&self) -> Option<AppProcessName>;
    // fn get_window_name
}

impl RawWindowHandleExt for RawWindowHandle {
    fn get_app_process_name(&self) -> Option<AppProcessName> {
        match self {
            RawWindowHandle::Win32(_) => {
                use platwins::context::context as plat_win_ctx;
                let hwnd = plat_win_ctx::get_hwnd_from_raw(*self)?;
                plat_win_ctx::get_app_process_name(&hwnd)
            }
            _ => todo!("This os is not supported"),
        }
    }
}
