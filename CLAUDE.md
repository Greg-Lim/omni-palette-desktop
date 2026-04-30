# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Overview

Omni Palette is a Windows system-wide command palette. It intercepts a configurable global hotkey, shows an egui UI, searches available commands, and executes actions defined by static TOML extensions or bundled WASM plugins.

## Tech Stack

- Language: Rust 2021
- UI: egui and eframe `0.34.1`
- Platform: Windows-only via the `windows` crate `0.60`
- Plugins: WASM through `wasmtime` with sandboxed host capabilities
- Config parsing: `toml` plus `serde`
- JSON: plugin command, storage, settings, and registry catalog contracts
- Logging: `log` plus `env_logger`; set `RUST_LOG=info` or `debug`

## Build and Run

```sh
cargo build
cargo run
cargo test
cargo test <test_name>
cargo clippy --all-targets --all-features
```

Extension management CLI:

```sh
cargo run -- ext catalog
cargo run -- ext install <id>
```

Extension packaging:

```sh
cargo run -p xtask -- detect-changed --force-all
cargo run -p xtask -- package-extension --package-root extensions/registry/packages/chrome/windows
```

## Architecture

The application is split between a Windows hotkey receiver, the egui UI thread, and a runtime bridge in `main.rs`.

```text
Hotkey receiver -> runtime bridge -> UI signals
UI events       -> runtime bridge -> registry/plugin/extension work
```

Module map:

```text
src/
  main.rs                    # entry point, runtime bridge, reload/install/settings handling
  config/
    extension.rs             # static extension, package, platform implementation, settings schemas
    ignore.rs                # ignored process config
    runtime.rs               # activation hotkey and extension source config
  core/
    command_filter.rs        # command filtering orchestration
    search.rs                # fuzzy/word-prefix scoring
    extensions/              # discovery, install state, marketplace catalog, packaging, settings
    plugins/                 # WASM manifests, runtime, command descriptors, host capabilities
    registry/registry.rs     # MasterRegistry, Application, UnitAction
  domain/
    action.rs                # Action, ActionExecution, FocusState, CommandPriority
    hotkey.rs                # Key, shortcut, modifiers, key sequences
  platform/
    windows/                 # context enumeration, hotkey receiver, shortcut sender, UI support
  ui/
    app.rs                   # CommandPaletteApp, UiEvent, UiSignal, tray icon
    palette.rs               # palette rendering
    guide.rs                 # guide mode
    settings.rs              # preferences, extensions, marketplace, extension settings
    components/              # shared egui widgets
```

## Extension System

Static extensions can be bundled or installed. Standalone static files use schema `version = 2`:

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
cmd = { mods = ["ctrl"], key = "KeyT" }
```

Registry packages use split source files:

- `manifest.toml` with `schema_version = 1`
- `actions.toml` with `schema_version = 1`
- `windows/static/*.toml` with `version = 3`

Bundled WASM plugins live under `extensions/bundled/plugins/<plugin-id>/plugin.toml`. Plugin commands can return direct `cmd` bindings for shortcut-backed actions, or omit `cmd` and execute through the plugin path.

Plugin permissions are narrow and explicit:

- `write_text`
- `read_time`
- `read_storage`
- `read_settings`
- `write_performance_log` in debug builds

## Extension Settings

The settings API is host-persisted and extension-defined. Static TOML extensions declare:

```toml
[[setting_categories]]
key = "general"
label = "General"
toggle_key = "general.enabled"
default_collapsed = true

[[settings]]
key = "general.enabled"
label = "Enabled"
category = "general"
type = "toggle"
default = true
```

WASM plugins can declare:

```toml
[settings]
source = "wasm"
```

and export `settings_schema_json()`.

Settings values are stored at:

```text
%APPDATA%\OmniPalette\extensions\settings\<extension-id>.toml
```

Saving extension settings triggers the existing runtime reload path.

## AHK Plugin

The bundled `ahk_agent` plugin uses the generic `read_storage`, `read_settings`, and `write_text` capabilities. Scripts opt in by including `OmniPaletteAgent.ahk`, which writes JSON snapshots to:

```text
%LOCALAPPDATA%\OmniPalette\plugins\ahk_agent\scripts
```

The plugin parses plain global hotkey labels and immediate one-line hotstrings with `*`. Hotkeys become direct shortcut actions. Hotstrings remain plugin commands that type the trigger text so the running AHK script expands it. The AHK settings schema creates one collapsible category per detected script with script-level and per-command toggles.

## Key Data Flow

1. Startup loads runtime config and extension state, discovers static configs and plugin manifests, builds `MasterRegistry`, and enumerates open windows.
2. The hotkey receiver detects the activation shortcut or forwards it to ignored apps.
3. The runtime bridge refreshes window context and asks the registry for visible commands.
4. The UI filters and renders commands, including guide mode and settings windows.
5. Execution focuses the target window when needed, sends shortcut sequences, or calls the owning plugin command.

## Important Notes

- The egui event loop must run on the main thread.
- `ApplicationID = u32` is assigned during registry build and is not stable across reloads.
- `extensions/bundled/ignore.toml` lists process names where the activation shortcut is forwarded.
- `extensions/registry/catalog.v1.json` is generated publish output and should not be hand-edited.
- Use narrow plugin capabilities instead of adding general filesystem access.
