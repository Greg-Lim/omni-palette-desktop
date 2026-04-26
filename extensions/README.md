# Extensions

Omni Palette keeps bundled defaults and downloadable registry packages separate.
This repository still carries a local copy of the registry layout. For now, the
app can point at this desktop repository as the remote catalog source; the
long-term dedicated extension repository should be `omni-palette-extensions`.

## Bundled Extensions

The app loads bundled runtime extensions from `extensions/bundled`.

```text
extensions/bundled/
  ignore.toml
  static/
    windows.toml
  plugins/
```

`windows.toml` is the bundled static shortcut pack. It provides core Windows
system shortcuts and can be disabled from the settings extension page. App
shortcut packs such as PowerPoint are registry packages rather than bundled
defaults.

```toml
version = 2
platform = "windows"

[app]
id = "windows"
name = "Windows"
process_name = "explorer.exe"
default_focus_state = "global"

[actions.open_file_explorer]
name = "Open File Explorer"
priority = "high"
cmd = { mods = ["win"], key = "KeyE" }
```

WASM plugins live in `extensions/bundled/plugins/<plugin_id>`. Downloadable WASM
plugin packages are not supported yet, so bundled plugins remain here for now.

```text
plugin.toml
plugin.wat
```

## Registry Source

Remote package catalog and source live in `extensions/registry`.

```text
extensions/registry/
  catalog.v1.json
  packages/
    chrome/
      windows/
        manifest.toml
        static/
          chrome.toml
    file_explorer/
      windows/
        manifest.toml
        static/
          file_explorer.toml
    powerpoint/
      windows/
        manifest.toml
        static/
          powerpoint.toml
```

Package source folders do not include version numbers. Git tags and GitHub
Releases identify published versions. For example:

```text
tag: chrome-v0.1.0
asset: chrome-0.1.0-windows.gpext
```

The generated `.gpext` file is an installable artifact and should stay out of
Git. Build artifacts into `target/extensions/`, then upload them to the matching
GitHub Release before catalog installs can succeed from GitHub.

The catalog points to release assets, not raw source files. Catalog signing is
paused in the current v1 settings flow; only `catalog.v1.json` is fetched.

## Publishing Registry Packages

Extension package publishing is automated through GitHub Actions. The workflow
detects changed package folders under `extensions/registry/packages/<id>/<platform>/`,
builds only those packages, uploads matching `.gpext` release assets, and commits
the updated catalog hash and URL.

For local verification, use the shared packaging task:

```sh
cargo run -p xtask -- detect-changed --force-all
cargo run -p xtask -- package-extension --package-root extensions/registry/packages/chrome/windows
```

Use `--update-catalog` when intentionally preparing catalog metadata for a newly
published artifact.

Static shortcut extensions represent known default shortcuts. They do not
automatically track user-customized keybindings inside the target application.
App-specific dynamic shortcut discovery should be implemented later as WASM
plugin logic when the target app stores keybindings in a readable config file or
exposes a command API.

## Ignore Config

`extensions/bundled/ignore.toml` lists applications that should receive
`Ctrl+Shift+P` normally instead of opening Omni Palette.
