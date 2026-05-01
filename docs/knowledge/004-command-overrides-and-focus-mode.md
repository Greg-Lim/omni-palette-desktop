---
title: Command Overrides And Focus Mode
status: proposed
tags: [commands, overrides, focus-mode, settings]
---

# Command Overrides And Focus Mode

## Summary

Extension files should remain immutable defaults. User-specific command changes
should live in AppData preference files and win over extension defaults.

## Current Understanding

Initial override scope should be shortcut remapping only. Later fields can
include labels, tags, priority, starred state, disabled state, and aliases.

User override files should use stable action keys rather than runtime `u32`
IDs:

```text
%APPDATA%\OmniPalette\extensions\overrides\<extension_id>.toml
```

Example:

```toml
version = 1
extension_id = "photoshop"

[actions.paint]
cmd = { mods = [], key = "L" }
updated_by_user = true
note = "User remapped Photoshop Paint from P to L"
```

Application command focus mode is a user preference that limits the palette to
commands from one selected application. It affects command visibility only and
should not prevent extension loading.

Suggested persistence:

```text
%APPDATA%\OmniPalette\focus_mode.toml
```

Essential built-in commands, such as reload and unlock focus mode, should stay
visible while focus mode is active.

## Testing

Useful coverage:

- Default shortcut is used when no override exists.
- User override wins over extension default.
- Broken override files do not break base extensions.
- Extension updates preserve user overrides.
- Reset removes the override and restores the default.
- Focus mode hides unrelated app commands.
- Focus mode keeps essential built-in commands visible.
- Reloading extensions reloads overrides and focus mode config.

## Follow-Up

Needed implementation slices include stable action IDs in runtime models,
override loading, shortcut recorder UI, reset-to-default behavior, focus mode
persistence, and focus mode filtering.
