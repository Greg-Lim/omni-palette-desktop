# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Omni Palette** is a Windows system-wide command palette (think VS Code's Ctrl+Shift+P, but global). It intercepts a global hotkey, shows a floating egui UI, and executes actions defined in TOML extension files or WASM plugins.

## Tech Stack

- **Language:** Rust (edition 2021)
- **UI:** egui + eframe v0.33 (immediate-mode GUI)
- **Platform:** Windows-only (via the `windows` crate v0.60); other OS panics
- **Plugins:** WASM via `wasmtime` with sandboxed capabilities
- **Config parsing:** `toml` + `serde`
- **Logging:** `log` + `env_logger` — set `RUST_LOG=info` or `debug`

## Build & Run

```sh
cargo build
cargo run
cargo test                        # run all unit tests
cargo test <test_name>            # run a single test by name
cargo clippy --all-targets --all-features
```

**Extension management CLI:**
```sh
cargo run -- ext catalog          # list available extensions from GitHub
cargo run -- ext install <id>     # download and install an extension
```

## Architecture

Three threads communicate via `mpsc` channels:

```
Hotkey Thread                UI Thread (main)           Runtime Bridge
(RegisterHotKey)             (egui/eframe)              (main.rs loop)
        │                          │                          │
        │──KeyboardShortcut──────► │                          │
        │                          │──UiEvent──────────────► │
        │                          │ ◄──UiSignal─────────────│
```

- **Hotkey thread** (`platform/windows/receiver/`) — registers Ctrl+Shift+P via Windows API, forwards events or passthroughs to ignored apps
- **UI thread** (`ui/app.rs`) — runs the egui event loop (winit requirement); renders palette, handles keyboard nav, sends `UiEvent` back to runtime
- **Runtime bridge** (`main.rs`) — owns `MasterRegistry`, responds to `UiEvent` (install, reload, settings save), pushes `UiSignal` to UI

## Module Map

```
src/
├── main.rs                    # Entry point, runtime bridge, thread wiring
├── core/
│   ├── search.rs              # Fuzzy/word-prefix scoring (MatchResult, PreparedQuery)
│   ├── extensions/            # TOML file discovery and parsing
│   └── registry/registry.rs  # MasterRegistry, Application, UnitAction, get_actions()
├── domain/
│   ├── action.rs              # Action, ActionExecution, FocusState, CommandPriority
│   └── hotkey.rs              # Key enum, HotkeyModifiers, KeyboardShortcut, SequenceKey
├── config/
│   ├── extension.rs           # Serde structs for extension TOML schema (version=2)
│   └── runtime.rs             # RuntimeConfig (hotkey, startup) stored in %APPDATA%\OmniPalette\
├── platform/
│   ├── platform_interface.rs  # get_all_context() façade, RawWindowHandleExt
│   ├── hotkey_actions.rs      # HotkeyHandle, HotkeyPassthrough (re-exports Windows impl)
│   └── windows/
│       ├── context/           # Enumerate open windows, detect active interaction
│       ├── receiver/          # WM_HOTKEY message loop, dynamic hotkey re-registration
│       └── sender/            # send_shortcut(), focus_window(), key sequence dispatch
└── ui/
    ├── app.rs                 # CommandPaletteApp, Command, UiSignal, UiEvent, tray icon
    └── settings.rs            # Settings panel (extension mgmt, hotkey config)
```

## Extension System

Extensions live in `extensions/bundled/static/` (built-in) or are installed by the user. Each `.toml` uses **schema version 2**:

```toml
version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"
process_name = "chrome.exe"
default_focus_state = "focused"

[actions.new_tab]
name = "New tab"
priority = "high"
cmd = { mods = ["ctrl"], key = "t" }
```

`FocusState` controls visibility:
- `focused` — foreground window matches this app
- `background` — app is open anywhere
- `global` — always shown

WASM plugins (in `extensions/bundled/plugins/`) can provide dynamic actions with sandboxed capabilities (read time, write text, performance logging).

## Key Data Flow

1. **Startup:** Discover TOML + WASM extensions → `MasterRegistry::build()` → enumerate open windows via `get_all_context()`
2. **Hotkey press:** Hotkey thread detects Ctrl+Shift+P → runtime calls `get_all_context()` → `registry.get_actions(context)` filters by process name and focus state → commands sent to UI as `UiSignal::Show`
3. **Search:** `core::search::get_score()` — fuzzy DP with word-start bonuses; multi-word queries scored per-piece then combined
4. **Execute:** Selected command's action closure is called; runtime focuses target window then dispatches shortcut or WASM plugin command

## Important Notes

- `ApplicationID = u32` is a counter assigned at registry build time — not stable across reloads; never persist it
- Schema version 1 is rejected; always use version 2 in extension TOML files
- `ignore.toml` in bundled extensions lists process names where Ctrl+Shift+P is forwarded (not intercepted)
- Before executing a shortcut, the target window must be focused via `focus_window()`; action closures in `main.rs` handle this automatically
- `MasterRegistry::build()` is best-effort (swallows errors); `build_strict()` fails on any invalid extension
