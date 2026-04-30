# Extensions

Omni Palette keeps bundled defaults and downloadable registry packages separate. This repository still carries a local copy of the registry layout. For now, the app can point at this desktop repository as the remote catalog source; the long-term dedicated extension repository should be `omni-palette-extensions`.

## Bundled Extensions

The app loads bundled runtime extensions from `extensions/bundled`.

```text
extensions/bundled/
  ignore.toml
  static/
    dummy_popout_box.toml
  plugins/
    ahk_agent/
      plugin.toml
      plugin.wasm
      OmniPaletteAgent.ahk
    auto_typer/
      plugin.toml
      plugin.wat
    performance_tracker/
      plugin.toml
      plugin.wat
```

Bundled static extensions use the same schema as installed static extensions:

```toml
version = 2
platform = "windows"

[app]
id = "dummy_popout_box"
name = "Dummy"
process_name = "dummy.exe"
default_focus_state = "global"
default_tags = ["dummy"]

[actions.pop_out_box]
name = "Pop-out box"
focus_state = "global"
tags = ["test"]
cmd = { mods = ["ctrl"], key = "KeyH" }
```

Bundled WASM plugins use `plugin.toml` manifests:

```toml
id = "auto_typer"
name = "Auto Typer"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wat"
permissions = ["write_text", "read_time"]

[app]
default_focus_state = "global"
default_tags = ["wasm", "demo", "typing"]
```

Supported plugin permissions are:

- `write_text`: type text into the active app.
- `read_time`: read host-provided time text.
- `read_storage`: list and read text files under the plugin-owned storage root.
- `read_settings`: read persisted extension setting toggles for the current plugin.
- `write_performance_log`: debug-build-only performance diagnostics.

Bundled plugin storage is rooted at:

```text
%LOCALAPPDATA%\OmniPalette\plugins\<plugin-id>
```

## Extension Settings

Static extensions can declare toggle settings directly in TOML:

```toml
[[setting_categories]]
key = "general"
label = "General"
description = "Optional category description"
toggle_key = "general.enabled"
default_collapsed = true

[[settings]]
key = "general.enabled"
label = "Enabled"
category = "general"
type = "toggle"
default = true
```

Category keys and setting keys must be unique. A setting that declares `category` must point at an existing category. If a category declares `toggle_key`, that setting must exist in the same category. Settings without a category are shown in a synthetic `General` group.

WASM plugins can declare extension-defined settings by adding:

```toml
[settings]
source = "wasm"
```

and exporting `settings_schema_json()`, which returns:

```json
{
  "categories": [
    {
      "key": "script",
      "label": "Script",
      "description": "Optional description",
      "toggle_key": "script.enabled",
      "default_collapsed": true
    }
  ],
  "items": [
    {
      "key": "script.enabled",
      "label": "Enabled",
      "category": "script",
      "type": "toggle",
      "default": true
    }
  ]
}
```

The host owns persistence and UI. Extension code owns the schema and reads saved values through `read_settings` when needed.

## AHK Plugin

The bundled `ahk_agent` plugin discovers instrumented AutoHotkey v2 scripts. Add this include near the top of each script you want Omni Palette to see:

```ahk
#Requires AutoHotkey v2.0
#Include "C:\path\to\global_palette\extensions\bundled\plugins\ahk_agent\OmniPaletteAgent.ahk"
```

The helper writes snapshots into:

```text
%LOCALAPPDATA%\OmniPalette\plugins\ahk_agent\scripts
```

Snapshot files contain `schema_version`, `script_path`, `script_text`, `updated_at_unix`, and `agent_version`. The WASM plugin lists `scripts/*.json`, reads each snapshot, and skips malformed files.

In v1, AHK discovery supports:

- Plain global hotkey labels such as `^h::MsgBox "Hello"`.
- One-line literal hotstrings with immediate expansion, such as `:?*:up;::⬆️`.

In v1, AHK discovery ignores conditional hotkey blocks, runtime `Hotkey()` registrations, body-style hotstrings, menus, and hotstrings without `*`.

Generated command labels are:

- `AHK: <script name> : <normalized hotkey>` for hotkeys.
- `AHK: <script name> : <trigger> -> <replacement preview>` for hotstrings.

Hotkeys are registered as direct shortcut actions. Hotstrings execute through the plugin path by typing the trigger text so the running AHK script performs the replacement. The extension settings UI shows one collapsible category per detected script, with a script-level toggle and per-command toggles.

## Registry Source

Remote package catalog and source live in `extensions/registry`.

```text
extensions/registry/
  catalog.v1.json
  packages/
    chrome/
      manifest.toml
      actions.toml
      windows/
        static/
          chrome.toml
```

`catalog.v1.json` is generated publish output. Do not hand-edit it when adding or updating packages. Edit package `manifest.toml`, `actions.toml`, and the platform implementation files; the GitHub Actions publish workflow packages the changed extension, uploads the `.gpext` release asset, and commits catalog URL, hash, and size changes.

Each static package has three source files:

```text
manifest.toml         # package identity, marketplace metadata, permissions
actions.toml          # global action names, priority, tags, focus, settings
windows/static/*.toml # Windows process name plus command bindings or pass
```

Platform implementation files use `version = 3` and should not contain action names, priority, tags, favorites, settings, or `when` conditions. Those live in `actions.toml`. A platform action may use `implementation = "pass"` to explicitly acknowledge an action that has no implementation on that platform.

Package source folders do not include version numbers. Git tags and GitHub Releases identify published versions. For example:

```text
tag: chrome-v0.1.0
asset: chrome-0.1.0-windows.gpext
```

The generated `.gpext` file is an installable artifact and should stay out of Git. Build artifacts into `target/extensions/`, then upload them to the matching GitHub Release before catalog installs can succeed from GitHub.

The catalog points to release assets, not raw source files. Catalog signing is paused in the current v1 settings flow; only `catalog.v1.json` is fetched.

## Publishing Registry Packages

Extension package publishing is automated through GitHub Actions. The workflow detects changed package folders under `extensions/registry/packages/<id>/<platform>/`, builds only those packages, uploads matching `.gpext` release assets, and commits the updated catalog hash and URL.

For local verification, use the shared packaging task:

```sh
cargo run -p xtask -- detect-changed --force-all
cargo run -p xtask -- package-extension --package-root extensions/registry/packages/chrome/windows
```

Use `--update-catalog` when intentionally preparing catalog metadata for a newly published artifact.

Static shortcut extensions represent known default shortcuts. They do not automatically track user-customized keybindings inside the target application. App-specific dynamic shortcut discovery should be implemented as WASM plugin logic when the target app stores keybindings in a readable config file or exposes a command API.

## Ignore Config

`extensions/bundled/ignore.toml` lists applications that should receive `Ctrl+Shift+P` normally instead of opening Omni Palette.
