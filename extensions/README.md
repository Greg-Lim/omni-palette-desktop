# Extensions

Global Palette supports two extension shapes.

## Static Shortcut Extensions

Put TOML shortcut mappings in `extensions/static`.

These files describe commands that map directly to keyboard shortcuts, such as Chrome or Windows shortcuts. The app also reads root-level `extensions/*.toml` files for now as a compatibility fallback, but new shortcut files should go in `extensions/static`.

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

Demo plugins can also keep readable source beside the compiled artifact. `auto_typer/plugin.wat` is kept next to `plugin.wasm` so the sample is easy to inspect.

## Ignore Config

`.ignore.toml` stays at the root of `extensions`.

Applications listed there receive `Ctrl+Shift+P` normally instead of opening Global Palette.
