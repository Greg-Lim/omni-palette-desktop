# Omni Palette Desktop

Omni Palette is a Windows command palette for launching application shortcuts and extension-provided commands from anywhere. Press the global hotkey, search for an action, and run it without leaving the keyboard.

![Omni Palette usage demo](README/Command_pallete.gif)

## Features

- Open the palette with `Ctrl+Shift+P` by default.
- Search commands with fuzzy matching and highlighted matches.
- Execute commands with `Enter` or by clicking a row.
- Filter commands by application focus: `focused`, `background`, or `global`.
- Prioritize commands with `high`, `medium`, `low`, and `suppressed` priority buckets.
- Use Guide Mode for shortcut-backed commands.
- Manage bundled and downloaded extensions from the settings window.
- Install static shortcut packs from the configured marketplace catalog.
- Load bundled WASM plugins with sandboxed host capabilities.
- Let extensions expose their own toggle settings, including per-script AHK controls.
- Ignore selected applications with `extensions/bundled/ignore.toml` so their own `Ctrl+Shift+P` behavior still works.

## Extensions

Omni Palette supports static TOML extensions and bundled WASM plugins.

Static shortcut packs can be bundled under `extensions/bundled/static/` or installed under the user extension root:

```text
%APPDATA%\OmniPalette\extensions
```

A standalone static extension uses schema `version = 2`:

```toml
version = 2
platform = "windows"

[app]
id = "chrome"
name = "Chrome"
process_name = "chrome.exe"
default_focus_state = "focused"
default_tags = ["browser"]

[actions.new_tab]
name = "New tab"
focus_state = "focused"
priority = "high"
tags = ["tabs"]
cmd = { mods = ["ctrl"], key = "KeyT" }
```

Registry packages use a split source layout:

```text
manifest.toml
actions.toml
windows/static/<package>.toml
```

`manifest.toml` and `actions.toml` use `schema_version = 1`. Platform implementation files use `version = 3` and contain only process and command bindings.

Bundled WASM plugins live in:

```text
extensions/bundled/plugins/<plugin_id>/plugin.toml
```

Plugin manifests declare their platform, WASM file, default app behavior, optional extension settings source, and sandbox permissions such as `write_text`, `read_time`, `read_storage`, and `read_settings`.

## Extension Settings

Extensions can expose toggle settings that are persisted by Omni Palette. Static TOML extensions declare settings with `[[setting_categories]]` and `[[settings]]`. WASM plugins can declare:

```toml
[settings]
source = "wasm"
```

and export `settings_schema_json()`.

Settings values are saved under the user extension root, separate from install state:

```text
%APPDATA%\OmniPalette\extensions\settings\<extension-id>.toml
```

Saving extension settings reloads runtime state so command visibility updates immediately.

## AHK Plugin

The bundled `ahk_agent` plugin discovers instrumented AutoHotkey v2 scripts. Add the helper include near the top of an AHK script:

```ahk
#Requires AutoHotkey v2.0
#Include "C:\path\to\global_palette\extensions\bundled\plugins\ahk_agent\OmniPaletteAgent.ahk"
```

The helper writes one JSON snapshot per script into plugin-owned storage:

```text
%LOCALAPPDATA%\OmniPalette\plugins\ahk_agent\scripts
```

The plugin reads those snapshots on app start or when you run `Omni Palette: Reload extensions`.

AHK hotkeys become shortcut-backed palette commands:

```text
AHK: <script name> : <normalized hotkey>
```

Immediate one-line hotstrings that include `*` become plugin commands:

```text
AHK: <script name> : <trigger> -> <replacement preview>
```

For example, `:?*:up;::⬆️` registers a command that types `up;`, letting the running AHK script expand it. The AHK settings panel exposes one collapsible category per detected script with a script-level toggle and individual command toggles.

## User Config

At runtime, Omni Palette looks for user settings in:

```text
%APPDATA%\OmniPalette\config.toml
```

The repo-root `config.toml` remains a development fallback. The activation shortcut can be configured as:

```toml
activation = { mods = ["ctrl", "shift"], key = "KeyP" }

[extensions.github]
owner = "Greg-Lim"
repo = "omni-palette-desktop"
branch = "master"
catalog_path = "extensions/registry/catalog.v1.json"
enabled = false
```

## Development

This project is built with Rust 2021, egui `0.34.1`, eframe `0.34.1`, and the Windows crate.

```sh
cargo build
cargo run
```

Useful checks:

```sh
cargo fmt
cargo check
cargo test
cargo clippy --all-targets --all-features
```
