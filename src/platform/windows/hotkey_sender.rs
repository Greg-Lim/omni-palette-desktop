#![cfg(target_os = "windows")]

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use windows::{
    core::Result,
    Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_P,
                VK_SPACE,
            },
            WindowsAndMessaging::{
                GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_QUIT,
            },
        },
    },
};

#[derive(Debug, Clone, Copy)]
pub struct HotkeyEvent {
    pub id: i32,
    pub vk: u32,
    pub modifiers: u32,
}

pub struct HotkeyHandle {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl HotkeyHandle {
    pub fn stop(mut self) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_QUIT,
                Default::default(),
                Default::default(),
            );
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn start_hotkey_listener() -> (HotkeyHandle, Receiver<HotkeyEvent>) {
    let (tx, rx) = mpsc::channel();
    let (id_tx, id_rx) = mpsc::channel();

    let join = thread::spawn(move || {
        unsafe {
            let _ = id_tx.send(GetCurrentThreadId());
        }
        if let Err(e) = hotkey_thread_main(tx) {
            eprintln!("hotkey thread error: {e:?}");
        }
    });

    let thread_id = id_rx.recv().expect("hotkey thread died early");
    (
        HotkeyHandle {
            thread_id,
            join: Some(join),
        },
        rx,
    )
}

fn hotkey_thread_main(tx: Sender<HotkeyEvent>) -> Result<()> {
    unsafe {
        // Ensure this thread has a message queue before RegisterHotKey
        let mut msg = MSG::default();

        // Keep this line! It explicitly ensures the message queue exists
        // before RegisterHotKey is called, making the code reliable.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);

        let hotkey_id = 1;
        // TODO: this needs to be moved out to have a mapping not in windows module
        RegisterHotKey(
            None,
            hotkey_id,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            VK_P.0 as u32,
        )?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_HOTKEY {
                let id = msg.wParam.0 as i32;
                let lp = msg.lParam.0 as u32;
                let modifiers = lp & 0xFFFF;
                let vk = (lp >> 16) & 0xFFFF;
                let _ = tx.send(HotkeyEvent { id, vk, modifiers });
            }
        }

        let _ = UnregisterHotKey(None, hotkey_id);
        Ok(())
    }
}
