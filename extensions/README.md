# Extensions

Global Palette supports two extension shapes.

## Static Shortcut Extensions

Put TOML shortcut mappings in `extensions/static`.

These files describe commands that map directly to keyboard shortcuts, such as Chrome or Windows shortcuts. Each file is OS-specific and uses `version = 2` with a single `platform`.

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

Static shortcut extensions represent known default shortcuts. They do not automatically
track user-customized keybindings inside the target application. App-specific dynamic
shortcut discovery should be implemented later as WASM plugin logic when the target app
stores keybindings in a readable config file or exposes a command API.

## WASM Plugins

Put executable plugins in `extensions/plugins/<plugin_id>`.

Each plugin folder should contain:

```text
plugin.toml
plugin.wasm
```

Plugin manifests are also OS-specific:

```toml
id = "auto_typer"
name = "Auto Typer"
platform = "windows"
version = "0.1.0"
wasm = "plugin.wasm"
permissions = ["type_text"]
```

Remote packages should be published per OS, for example `chrome-1.0.0-windows.gpext`.

Demo plugins can also keep readable source beside the compiled artifact. `auto_typer/plugin.wat` is kept next to `plugin.wasm` so the sample is easy to inspect.

## Ignore Config

`.ignore.toml` stays at the root of `extensions`.

Applications listed there receive `Ctrl+Shift+P` normally instead of opening Global Palette.
