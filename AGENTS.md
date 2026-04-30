# Omni Palette - AGENTS.md

## Project Overview

Omni Palette is a Windows system-wide command palette. It listens for a global hotkey, shows an egui UI, searches available commands, and executes static shortcut actions or WASM plugin actions.

## Tech Stack

- Language: Rust 2021
- UI: egui and eframe `0.34.1`
- Platform: Windows-only currently, using the `windows` crate `0.60`
- Plugins: wasmtime-hosted WASM with explicit sandbox permissions
- Config and data: TOML plus serde, with JSON used for plugin host contracts
- Logging: `log` plus `env_logger`
- Packaging: `xtask` builds extension packages and catalog metadata

## Architecture

```text
src/
  main.rs                       # entry point, runtime bridge, extension reload/install events
  config/
    extension.rs                # static extension, package, implementation, and settings schemas
    ignore.rs                   # ignored foreground apps for hotkey passthrough
    runtime.rs                  # user runtime config
  core/
    command_filter.rs           # command search/filter orchestration
    search.rs                   # fuzzy scoring
    extensions/                 # discovery, install state, catalog, package, settings
    plugins/                    # WASM manifest loading, runtime, commands, host capabilities
    registry/registry.rs        # MasterRegistry, Application, UnitAction
  domain/
    action.rs                   # Action, ActionExecution, FocusState, CommandPriority
    hotkey.rs                   # Key, modifiers, shortcuts, sequences
  platform/
    hotkey_actions.rs           # platform hotkey facade
    platform_interface.rs       # context lookup facade
    windows/                    # context, receiver, sender, key mapping, UI support
  ui/
    app.rs                      # CommandPaletteApp, UiEvent, UiSignal, tray integration
    palette.rs                  # command palette UI
    guide.rs                    # guide mode UI
    settings.rs                 # settings window, extensions, marketplace, extension settings
    components/                 # shared widgets such as the sliding toggle switch
```

## Runtime Flow

1. Startup loads runtime config, extension discovery, bundled defaults, installed extensions, WASM plugins, and ignored app config.
2. The Windows hotkey receiver registers the activation shortcut and forwards events to the runtime bridge.
3. The runtime bridge refreshes window context, queries `MasterRegistry`, and sends visible commands to the UI.
4. The UI filters commands as the user types and sends execution events back to the runtime bridge.
5. Shortcut-backed actions focus the target window and send keys. Plugin commands call the owning WASM plugin.

The egui event loop must stay on the main thread.

## Extension System

Omni Palette supports three active extension shapes:

- Bundled static TOML files under `extensions/bundled/static/`
- Bundled WASM plugins under `extensions/bundled/plugins/<plugin-id>/plugin.toml`
- Downloaded static registry packages installed under `%APPDATA%\OmniPalette\extensions`

Standalone static TOML files use `version = 2`. Registry packages use `manifest.toml` plus `actions.toml` with `schema_version = 1`, and platform implementations with `version = 3`.

Static extension settings use `[[setting_categories]]` and `[[settings]]` with `category`. WASM plugins can declare `[settings] source = "wasm"` and export `settings_schema_json()`.

Plugin permissions are intentionally narrow. Current permissions include `write_text`, `read_time`, `read_storage`, `read_settings`, and debug-only `write_performance_log`.

## AHK Plugin

The bundled `ahk_agent` plugin is implemented in `extensions/bundled/plugins/ahk_agent/`. It discovers instrumented AutoHotkey v2 scripts via `OmniPaletteAgent.ahk`.

The helper writes per-script JSON snapshots into:

```text
%LOCALAPPDATA%\OmniPalette\plugins\ahk_agent\scripts
```

The plugin reads those snapshots through generic plugin storage, parses plain global hotkeys and immediate one-line hotstrings, and exposes script and command toggles through the generic extension settings API.

## Settings and Install State

- Runtime config: `%APPDATA%\OmniPalette\config.toml`
- User extension root: `%APPDATA%\OmniPalette\extensions`
- Extension install state: `%APPDATA%\OmniPalette\extensions\installed.toml`
- Extension settings: `%APPDATA%\OmniPalette\extensions\settings\<extension-id>.toml`
- Plugin storage: `%LOCALAPPDATA%\OmniPalette\plugins\<plugin-id>`

Bundled extensions can be disabled from settings but not uninstalled. Downloaded extensions can be disabled or uninstalled.

## Build and Test

```sh
cargo build
cargo run
cargo test
cargo clippy --all-targets --all-features
```

Useful extension packaging commands:

```sh
cargo run -p xtask -- detect-changed --force-all
cargo run -p xtask -- package-extension --package-root extensions/registry/packages/chrome/windows
```

## Important Notes

- `ApplicationID = u32` is assigned during registry builds and should not be persisted.
- `MasterRegistry::build()` is best-effort; use strict loading paths in tests when invalid input should fail.
- Generated catalog output in `extensions/registry/catalog.v1.json` should not be hand-edited.
- Do not broaden plugin capabilities when a narrow host import is enough.
- Preserve user or uncommitted work in this repository; inspect before editing files that may already be dirty.
