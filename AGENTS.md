# Omni Palette — AGENTS.md

## Project Overview

**Omni Palette** is a Windows application launcher / command palette (think VS Code's Ctrl+Shift+P, but system-wide). It listens for a global hotkey (Ctrl+Shift+P), shows a floating egui UI, and lets users search & execute actions defined in TOML extension files.

## Tech Stack

- **Language:** Rust (edition 2021)
- **UI:** [egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) v0.33
- **Platform:** Windows-only currently (via the `windows` crate v0.60)
- **Window handle abstraction:** `raw-window-handle` v0.6
- **Config parsing:** `toml` + `serde`
- **Logging:** `log` + `env_logger`
- **Enum helpers:** `strum` / `strum_macros`

## Architecture

```
src/
├── main.rs                    # Entry point: logger, hotkey listener, registry load, UI launch
├── core/
│   ├── context.rs             # (empty placeholder)
│   ├── search.rs              # Fuzzy/sequential search scoring
│   ├── extensions/
│   │   └── extensions.rs      # Loads .toml extension configs from ./extensions/ folder
│   └── registry/
│       └── registry.rs        # MasterRegistry, Application, UnitAction
├── models/
│   ├── action.rs              # Action, FocusState, ContextRoot, type aliases
│   ├── config.rs              # Serde structs for TOML config (Config, App, KeyChord, etc.)
│   ├── hotkey.rs              # Key enum, HotkeyModifiers, KeyboardShortcut
│   └── registry.rs            # (TBD)
├── platform/
│   ├── platform_interface.rs  # get_all_context(), RawWindowHandleExt trait
│   ├── hotkey_actions.rs      # start_hotkey_listener(), returns (handle, Receiver<HotkeyEvent>)
│   └── windows/
│       ├── context/           # get_all_windows(), get_hwnd_from_raw(), get_app_process_name()
│       ├── mapper/            # Hotkey mapping
│       ├── receiver/          # Hotkey receiver
│       ├── sender/            # Hotkey sender (e.g. send_ctrl_v)
│       └── shortcuts/         # Windows shortcut registration
└── ui/
    ├── ui_main.rs             # CommandPaletteApp, App (eframe::App), ui_main()
    └── combo_box.rs           # (UI widget)
```

## Extension System

Extensions live in `./extensions/*.toml`. Each file defines an app and its actions:

```toml
version = 1

[app]
id = "chrome"
name = "Chrome"
default_focus_state = "focused"

[app.application_os_name]
windows = "chrome.exe"
macos = "com.google.Chrome"

[actions.new_tab]
name = "New tab"
focus_state = "focused"
priority = "high"
cmd.windows = { mods = ["ctrl"], key = "t" }
cmd.macos = { mods = ["cmd"], key = "t" }
```

`FocusState` controls when an action appears:
- `focused` — only when that app is the foreground window
- `background` — when that app is open (any window)
- `global` — always

## Key Data Flow

1. **Startup:** Load extensions → build `MasterRegistry` → call `get_all_context()` (enumerates open windows)
2. **Hotkey (Ctrl+Shift+P):** Hotkey thread sends `UiSignal::ToggleVisibility` to UI thread via `mpsc`
3. **UI:** `CommandPaletteApp` filters `all_commands` by `filter_text`; arrow keys navigate; Enter executes
4. **Search:** `core::search::get_score()` does sequential character matching with proximity bonuses

## Current State / Known Gaps

- `CommandPaletteApp.filtered_indices` is never populated in the current code — the filter logic is missing (search is wired in models but not connected to UI)
- `core/context.rs` is empty
- `src/models/registry.rs` is empty
- Window hiding uses a hacky approach (minimize + move off-screen to `-2000, -2000`)
- The fuzzy scorer (`do_score_fuzzy`) is `todo!()`
- Only Windows is supported; other platforms panic

## Build & Run

```sh
cargo build
cargo run
```

Set `RUST_LOG=info` (or `debug`) for logs.

## Important Notes

- The egui event loop **must** run on the main thread (winit requirement)
- Hotkey listener runs in a background thread and communicates via `mpsc`
- `MasterRegistry` uses `ApplicationID = u32` as index counter (not stable across reloads)
