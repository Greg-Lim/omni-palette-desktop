---
title: Extension System
status: active
tags: [extensions, plugins, wasm, permissions]
---

# Extension System

## Summary

Omni Palette supports static TOML extensions and WASM plugins. TOML remains the
best format for static app shortcuts. WASM plugins are used when an extension
needs executable behavior.

## Current Understanding

Static extensions describe applications, process names, commands, focus state,
priority, tags, and keyboard shortcuts.

WASM plugins are for behavior that TOML cannot express well, such as command
handlers, stateful workflows, context-aware commands, or host-approved
capability calls. Plugins should not access OS APIs directly. The host owns all
privileged behavior and exposes narrow permissions.

Current permission concepts include:

- `write_text`
- `read_time`
- `read_storage`
- `read_settings`
- debug-only `write_performance_log`

Bundled plugins live under:

```text
extensions/bundled/plugins/<plugin-id>/plugin.toml
```

Downloaded static registry packages live under the user extension root in
AppData.

## Design Notes

Plugin failures should not crash Omni Palette. A malformed manifest, WASM load
failure, command registration failure, execution failure, or timeout should
disable or skip only the affected plugin or command.

The host should continue to own search, highlighting, sorting, priority
interpretation, palette rendering, keyboard navigation, and execution routing.
Plugins should not draw custom UI or override search ranking in the initial
plugin model.

## Testing

Useful coverage:

- TOML extensions still load unchanged.
- Valid plugin manifests and modules load successfully.
- Plugin commands appear in the same command list as static commands.
- Undeclared permission calls are rejected.
- Plugin crashes or timeouts do not crash Omni Palette.
- Ignored foreground applications bypass the palette regardless of extension
  type.

## Follow-Up

Deferred ideas include custom plugin UI, fully dynamic query-time command
generation, remote plugin installation, and broader host capabilities.
