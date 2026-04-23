# Extensions

Global Palette keeps bundled extensions and remote-package registry files separate.
This repository is still the temporary registry host, but it is not using a Git
submodule or a separate extension repository yet.

## Bundled Extensions

The app loads bundled runtime extensions from `extensions/bundled`.

```text
extensions/bundled/
  ignore.toml
  static/
  plugins/
```

Static shortcut mappings live in `extensions/bundled/static`. Each file is
OS-specific and uses `version = 2` with a single `platform`.

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
priority = "high"
cmd = { mods = ["ctrl"], key = "T" }
```

WASM plugins live in `extensions/bundled/plugins/<plugin_id>`.

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

Static shortcut extensions represent known default shortcuts. They do not
automatically track user-customized keybindings inside the target application.
App-specific dynamic shortcut discovery should be implemented later as WASM
plugin logic when the target app stores keybindings in a readable config file or
exposes a command API.

## Registry Source

Remote package catalog and source live in `extensions/registry`.

```text
extensions/registry/
  catalog.v1.json
  catalog.v1.json.sig
  packages/
    downloaded_test/
      windows/
        manifest.toml
        static/
          downloaded_test.toml
```

Package source folders do not include version numbers. Git tags and GitHub
Releases identify published versions. For example:

```text
tag: downloaded_test-v0.1.0
asset: downloaded_test-0.1.0-windows.gpext
```

The generated `.gpext` file is an installable artifact and should stay out of
Git. Build it into `target/` and upload it to the matching GitHub Release.

The catalog points to release assets, not raw source files. The signature file
verifies the catalog contents, so moving `catalog.v1.json` and
`catalog.v1.json.sig` together does not invalidate the signature.

## Ignore Config

`extensions/bundled/ignore.toml` lists applications that should receive
`Ctrl+Shift+P` normally instead of opening Global Palette.
