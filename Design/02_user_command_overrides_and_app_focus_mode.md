# User Command Overrides And App Focus Mode

## Status

Future plan. This document records product and implementation direction only.
It should not be treated as implemented behavior.

## Summary

Omni Palette extension files should remain immutable defaults. User-specific
changes, such as fixing a shortcut that the target application has remapped,
should live in AppData override/config files.

Two related future features are planned:

- user command overrides for remapping extension commands;
- application command focus mode for showing commands from one app only.

These features should be local preference behavior. They should not require
editing bundled extension TOML files or downloaded package source files.

## Feature 1: User Command Overrides

Users should be able to fix or customize a command when the target application
shortcut changes.

Example:

```text
Photoshop extension default:
Paint -> P

User changes Photoshop shortcut:
Paint -> L

Omni Palette should allow:
Edit command / Command is wrong -> set shortcut to L
```

The raw extension file should not be edited. Instead, user overrides should be
stored beside installed extension state in AppData:

```text
%APPDATA%\OmniPalette\extensions\overrides\<extension_id>.toml
```

Example override file:

```toml
version = 1
extension_id = "photoshop"

[actions.paint]
cmd = { mods = [], key = "L" }
updated_by_user = true
note = "User remapped Photoshop Paint from P to L"
```

### Rules

- Extension TOML defines default command behavior.
- User override TOML wins over extension defaults.
- Updating or reinstalling an extension package must not delete user remaps.
- Overrides must use stable action keys, not runtime `u32` action IDs.
- Registry data should preserve `{app_id}.{action_id}` so overrides can target
  the correct command.
- Initial override scope should be shortcut remapping only.
- Labels, tags, priority, starred state, disabled state, and aliases can be
  future override fields after shortcut remapping is stable.

### UI Ideas

Possible command-row actions:

- `Edit command`
- `Command is wrong`
- `Reset to extension default`

The edit flow should include a shortcut recorder UI where the user presses the
desired shortcut and Omni Palette saves the resolved key chord into the
override file.

## Feature 2: Application Command Focus Mode

Application command focus mode is a user preference that limits the palette to
commands from one selected application.

Example:

```text
Focus mode: Photoshop
Palette shows Photoshop commands only.
Chrome, Windows, Explorer, and unrelated global commands are hidden.
```

Recommended built-in commands:

```text
Omni Palette: Lock commands to current app
Omni Palette: Unlock command focus mode
```

Persist the lock in AppData:

```text
%APPDATA%\OmniPalette\focus_mode.toml
```

Example:

```toml
version = 1
locked_app_id = "photoshop"
enabled = true
```

### Filtering Rules

- If focus mode is off, the palette behaves normally.
- If focus mode is on, show only commands belonging to `locked_app_id`.
- Essential built-in commands must remain visible, including:
  - `Reload extensions`
  - `Unlock command focus mode`
- Focus mode affects command visibility only.
- Focus mode should not prevent extension loading.
- Focus mode should be user preference state, not extension metadata.

## Development TODOs

1. Preserve stable app/action IDs in runtime models.
2. Load user override TOML files from AppData.
3. Apply shortcut overrides during registry build and reload.
4. Add command-row UI actions for editing shortcuts.
5. Add shortcut recorder UI.
6. Add reset-to-default behavior.
7. Add focus mode persistence.
8. Add palette filtering for focus mode.
9. Keep essential built-in commands visible while focus mode is active.
10. Add tests for override precedence and focus mode filtering.

## Future Test Plan

- Extension default shortcut is used when no override exists.
- User override shortcut wins over extension default.
- Broken override file is ignored or reported without breaking the base
  extension.
- Extension update preserves user override.
- Reset removes override and restores extension default.
- Focus mode hides unrelated app commands.
- Focus mode keeps `Reload extensions` and `Unlock command focus mode` visible.
- Reloading extensions also reloads override files and focus mode config.
- Tests cover bundled extensions and AppData-installed extensions.

## Assumptions

- This is documentation and planning only.
- Shortcut editing UI is not implemented yet.
- Extension schema should not change for this first planning step.
- Raw extension files should not be mutated for user preferences.
- User preference files live under `%APPDATA%\OmniPalette`.
- Stable action keys should use `{app_id}.{action_id}`.
