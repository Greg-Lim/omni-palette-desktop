use crate::{
    domain::action::{AppProcessName, ContextRoot},
    platform::windows as platwins,
};
use cfg_if::cfg_if;
use raw_window_handle::RawWindowHandle;

pub fn get_all_context() -> ContextRoot {
    log::debug!("Getting all context from platform interface");
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            log::debug!("Detected Windows OS, using Windows context retrieval");
            // Destructure the tuple returned by your Windows function
            let (fg, bg) = platwins::context::context::get_all_windows();
            let active_interaction = fg
                .first()
                .and_then(|handle| platwins::context::context::get_hwnd_from_raw(*handle))
                .map(platwins::context::interaction::detect_active_interaction)
                .unwrap_or_default();
            log::debug!(
                "Retrieved context from Windows: fg has {} items, bg has {} items",
                fg.len(),
                bg.len()
            );
        } else {
            // Fallback for other OSs
            panic!("Not valid os")
            let fg = vec![];
            let bg = vec![];
            let active_interaction = crate::domain::action::InteractionContext::default();
        }
    }

    log::debug!("Final context");

    ContextRoot {
        fg_context: fg,
        bg_context: bg,
        active_interaction,
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn get_all_context_runs() {
        let context = get_all_context();

        println!("Foreground Context:");
        for (i, item) in context.fg_context.iter().enumerate() {
            println!("  {}: {:?}", i, item);
        }
    }
}
