# ADR 0002: Code-Based Extension Plugin Design

## Status

Proposed

## Context

Global Palette currently supports TOML-based extensions. This is a good fit for static app shortcut mappings, such as Chrome commands that map directly to keyboard shortcuts.

In the future, some extensions may need behavior that is difficult or impossible to express in TOML:

- generating commands from state or context;
- running custom command handlers;
- calling APIs through approved host capabilities;
- storing extension state;
- integrating with workflows that are more than a fixed keyboard shortcut.

This ADR records the future direction for code-based extensions. It does not replace TOML extensions.

## Decision

Global Palette should keep TOML extensions for static shortcut definitions and add a WASM-based plugin system for code-based extensions.

WASM is preferred over raw Python/Lua subprocess scripts or in-process Rust dynamic loading because it provides:

- a stronger sandbox boundary;
- a cleaner host/plugin API;
- future multi-language support;
- less risk of crashing the main app process;
- a better foundation for enforcing permissions.

The first version of code-based extensions should support plugins that register commands and execute command handlers. Fully dynamic query-time command generation should be deferred.

## Extension Types

### TOML Extensions

TOML remains the recommended format for simple, static shortcuts.

Use TOML when an extension only needs to describe:

- an application;
- process names per OS;
- command names;
- focus state;
- action priority;
- tags;
- keyboard shortcuts.

### WASM Plugins

WASM plugins are for extensions that need executable behavior.

Use WASM when an extension needs to:

- compute or generate commands;
- run custom logic when a command is executed;
- use host-approved capabilities;
- keep plugin state;
- integrate with external workflows.

## Proposed Plugin Layout

Future code-based extensions should use a folder-based layout:

```text
extensions/
  static/
    chrome.toml
  plugins/
    plugin_id/
      plugin.toml
      plugin.wasm
```

`plugin.toml` should describe metadata and permissions.

`plugin.wasm` should contain the executable plugin module.

## Manifest Concepts

The plugin manifest should include:

- plugin id;
- display name;
- version;
- target application process names;
- requested permissions;
- optional default focus state;
- optional default tags.

Permissions should be explicit and host-enforced. Example permission categories may include:

- keyboard shortcut execution;
- window focus;
- shell command execution;
- filesystem access;
- clipboard access;
- network access.

The exact permission names should be decided when implementation begins.

## Host And Plugin Boundary

Plugins should not directly access OS APIs. The host owns all privileged behavior.

The host should provide controlled APIs for actions such as:

- sending keyboard shortcuts;
- focusing windows;
- reading palette context;
- running shell commands;
- using the clipboard;
- reading or writing files;
- making network requests, if supported later.

If a plugin calls a capability that was not declared in its manifest, the host should reject the call.

## V1 Plugin API Shape

The first plugin API should be intentionally small:

```text
register_commands(context) -> Vec<CommandDescriptor>
execute(command_id, context) -> ExecutionResult
```

`CommandDescriptor` should map into the existing palette command model:

- command id;
- label;
- priority;
- starred state;
- tags;
- focus state;
- shortcut text or custom display text.

The host should continue to own:

- fuzzy search;
- highlighting;
- sorting;
- priority interpretation;
- palette rendering;
- keyboard navigation;
- command execution routing.

Plugins should not override search ranking or draw their own UI in v1.

## Error Handling

Plugin failures should not crash Global Palette.

Expected behavior:

- malformed manifest disables only that plugin;
- WASM load failure disables only that plugin;
- command registration failure logs an error and skips that plugin;
- command execution failure logs or surfaces the error without crashing the palette;
- long-running plugin calls should time out.

## Deferred Work

The following should not be included in the first code-based extension version:

- raw Python or Lua script execution;
- in-process Rust dynamic library loading;
- plugins that render custom UI;
- plugins that override fuzzy scoring;
- fully dynamic command generation on every typed query;
- marketplace or remote plugin installation.

Python, Lua, and Rust may be supported later by compiling to WASM or through a separate subprocess adapter.

## Test Plan

When implemented, this design should be validated with tests for:

- existing TOML extensions still loading unchanged;
- valid WASM plugin manifests loading successfully;
- plugin commands appearing in the same palette list as TOML commands;
- host-controlled fuzzy search, priority sorting, and highlighting applying to plugin commands;
- plugin command execution calling the plugin handler;
- undeclared permission calls being rejected;
- malformed plugins disabling only themselves;
- plugin crashes or timeouts not crashing Global Palette;
- ignored applications bypassing the palette regardless of extension type.

## References

- https://www.youtube.com/watch?v=fvxOI0nQsTA
- https://www.reddit.com/r/rust/comments/1hvaz5f/rust_wasm_plugins_example/
