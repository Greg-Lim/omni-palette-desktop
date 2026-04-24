#![cfg(target_os = "windows")]

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    domain::hotkey::KeyboardShortcut,
    platform::windows::mapper::hotkey_mapper::{
        map_key, map_key_back, map_modifier, map_modifier_back,
    },
    platform::windows::sender::hotkey_sender::send_shortcut,
};

use log::warn;
use windows::{
    core::Result,
    Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{
                GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
                WM_QUIT,
            },
        },
    },
};

const PALETTE_HOTKEY_ID: i32 = 1;
const WM_FORWARD_SHORTCUT: u32 = WM_APP + 1;

enum HotkeyThreadCommand {
    ForwardShortcut(KeyboardShortcut),
    UpdateShortcut(KeyboardShortcut, Sender<std::result::Result<(), String>>),
}

#[derive(Clone)]
pub struct HotkeyPassthrough {
    thread_id: u32,
    command_tx: Sender<HotkeyThreadCommand>,
}

impl HotkeyPassthrough {
    pub fn forward_shortcut(&self, shortcut: KeyboardShortcut) {
        if self
            .command_tx
            .send(HotkeyThreadCommand::ForwardShortcut(shortcut))
            .is_ok()
        {
            unsafe {
                let _ = PostThreadMessageW(
                    self.thread_id,
                    WM_FORWARD_SHORTCUT,
                    Default::default(),
                    Default::default(),
                );
            }
        }
    }

    pub fn update_shortcut(&self, shortcut: KeyboardShortcut) -> std::result::Result<(), String> {
        let (result_tx, result_rx) = mpsc::channel();
        self.command_tx
            .send(HotkeyThreadCommand::UpdateShortcut(shortcut, result_tx))
            .map_err(|err| format!("Could not send hotkey update request: {err}"))?;
        unsafe {
            PostThreadMessageW(
                self.thread_id,
                WM_FORWARD_SHORTCUT,
                Default::default(),
                Default::default(),
            )
            .map_err(|err| format!("Could not wake hotkey thread: {err:?}"))?;
        }
        result_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|err| format!("Timed out waiting for hotkey update: {err}"))?
    }
}

pub struct HotkeyHandle {
    thread_id: u32,
    command_tx: Sender<HotkeyThreadCommand>,
    hotkey_thread_handle: Option<JoinHandle<()>>,
}

impl HotkeyHandle {
    pub fn passthrough_sender(&self) -> HotkeyPassthrough {
        HotkeyPassthrough {
            thread_id: self.thread_id,
            command_tx: self.command_tx.clone(),
        }
    }

    pub fn stop(mut self) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_QUIT,
                Default::default(),
                Default::default(),
            );
        }
        if let Some(j) = self.hotkey_thread_handle.take() {
            let _ = j.join();
        }
    }
}

pub fn start_hotkey_listener(
    activation_shortcut: KeyboardShortcut,
) -> (HotkeyHandle, Receiver<KeyboardShortcut>) {
    let (hk_event_tx, hk_event_rx) = mpsc::channel();
    let (command_tx, command_rx) = mpsc::channel();
    let (id_tx, id_rx) = mpsc::channel();

    let hotkey_thread_handle = thread::spawn(move || {
        unsafe {
            let _ = id_tx.send(GetCurrentThreadId());
        }
        if let Err(e) = hotkey_thread_main(hk_event_tx, command_rx, activation_shortcut) {
            eprintln!("hotkey thread error: {e:?}");
        }
    });

    let thread_id = id_rx.recv().expect("hotkey thread died early");
    (
        HotkeyHandle {
            thread_id,
            command_tx,
            hotkey_thread_handle: Some(hotkey_thread_handle),
        },
        hk_event_rx,
    )
}

fn register_palette_hotkey(shortcut: KeyboardShortcut) -> Result<()> {
    unsafe {
        let modifiers = map_modifier(&shortcut.modifier) | MOD_NOREPEAT;
        let key = map_key(shortcut.key);
        RegisterHotKey(None, PALETTE_HOTKEY_ID, modifiers, key.0 as u32)
    }
}

fn hotkey_thread_main(
    tx: Sender<KeyboardShortcut>,
    command_rx: Receiver<HotkeyThreadCommand>,
    mut activation_shortcut: KeyboardShortcut,
) -> Result<()> {
    unsafe {
        // Ensure this thread has a message queue before RegisterHotKey
        let mut msg = MSG::default();

        // Keep this line! It explicitly ensures the message queue exists
        // before RegisterHotKey is called, making the code reliable.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);

        register_palette_hotkey(activation_shortcut)?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_HOTKEY {
                let _id = msg.wParam.0 as i32;
                let lp = msg.lParam.0 as u32;
                let modifiers = lp & 0xFFFF;
                let vk: VIRTUAL_KEY = VIRTUAL_KEY(((lp >> 16) & 0xFFFF) as u16);
                let shortcut = map_key_back(vk).map(|k| KeyboardShortcut {
                    key: k,
                    modifier: map_modifier_back(HOT_KEY_MODIFIERS(modifiers)),
                });
                if shortcut.is_none() {
                    warn!("Mapping Fail {vk:?}");
                    break;
                }
                let shortcut = shortcut.unwrap();
                let _ = tx.send(shortcut);
            } else if msg.message == WM_FORWARD_SHORTCUT {
                while let Ok(command) = command_rx.try_recv() {
                    match command {
                        HotkeyThreadCommand::ForwardShortcut(shortcut) => {
                            let _ = UnregisterHotKey(None, PALETTE_HOTKEY_ID);
                            send_shortcut(&shortcut);
                            thread::sleep(Duration::from_millis(50));

                            if let Err(err) = register_palette_hotkey(activation_shortcut) {
                                warn!("Failed to re-register palette hotkey: {err:?}");
                            }
                        }
                        HotkeyThreadCommand::UpdateShortcut(shortcut, result_tx) => {
                            let old_shortcut = activation_shortcut;
                            let _ = UnregisterHotKey(None, PALETTE_HOTKEY_ID);
                            match register_palette_hotkey(shortcut) {
                                Ok(()) => {
                                    activation_shortcut = shortcut;
                                    let _ = result_tx.send(Ok(()));
                                }
                                Err(err) => {
                                    let rollback_result = register_palette_hotkey(old_shortcut);
                                    let message = match rollback_result {
                                        Ok(()) => {
                                            format!("Failed to register palette hotkey: {err:?}")
                                        }
                                        Err(rollback_err) => format!(
                                            "Failed to register palette hotkey: {err:?}; also failed to restore previous hotkey: {rollback_err:?}"
                                        ),
                                    };
                                    let _ = result_tx.send(Err(message));
                                }
                            }
                        }
                    }
                }
            }
        }

        let _ = UnregisterHotKey(None, PALETTE_HOTKEY_ID);
        Ok(())
    }
}
