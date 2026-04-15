use crate::{
    models::action::{AppProcessName, ContextRoot},
    platform::windows as platwins,
};
use cfg_if::cfg_if;
use raw_window_handle::RawWindowHandle;

pub fn get_all_context() -> ContextRoot {
    dbg!("Getting all context from platform interface");
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            dbg!("Detected Windows OS, using Windows context retrieval");
            // Destructure the tuple returned by your Windows function
            let (fg, bg) = platwins::context::context::get_all_windows();
            dbg!("Retrieved context from Windows: fg has {} items, bg has {} items", fg.len(), bg.len());
        } else {
            // Fallback for other OSs
            panic!("Not valid os")
            let fg = vec![];
            let bg = vec![];
        }
    }

    dbg!("Final context");

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
