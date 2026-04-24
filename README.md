# Omni Palette Desktop

Omni Palette is a Windows command palette for launching application shortcuts from anywhere. This repository contains the desktop application: press the global hotkey, search for an action, and run it without leaving the keyboard.

![Omni Palette usage demo](README/Command_pallete.gif)

## Features

- Open the palette with `Ctrl+Shift+P`.
- Search actions with fuzzy matching.
- Prioritize common commands with `high`, `normal`, and `suppressed` priority buckets.
- Highlight matching text while searching.
- Execute commands with `Enter` or by clicking a row.
- Ignore selected applications with `extensions/ignore.toml` so their own `Ctrl+Shift+P` behavior still works.

## Extensions

Commands are defined in TOML files under `extensions/`. Each extension describes one application and the actions Omni Palette can run for it.

Example action:

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
focus_state = "focused"
action_priority = "high"
cmd = { mods = ["ctrl"], key = "T" }
```

Extension files are OS-specific. A Windows package installs only Windows TOML or
plugin metadata, macOS packages install macOS metadata, and Linux packages install
Linux metadata.

Static TOML extensions describe the application's known default shortcuts. If a target
application lets users customize its own keybindings, Omni Palette does not
automatically detect those app-specific changes in static extensions. Future WASM
plugins can add app-specific keybinding resolvers for applications that expose readable
settings or command APIs.

## User Config

At runtime, Omni Palette looks for user settings in:

```text
%APPDATA%\OmniPalette\config.toml
```

The repo-root `config.toml` remains a development fallback. The activation shortcut can
be configured as:

```toml
activation = { mods = ["ctrl", "shift"], key = "p" }

[extensions.github]
owner = "Greg-Lim"
repo = "omni-palette-desktop"
branch = "master"
catalog_path = "extensions/registry/catalog.v1.json"
enabled = false
```

## Development

This project is built with Rust, egui, and eframe.

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
