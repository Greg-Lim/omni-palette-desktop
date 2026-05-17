# Tauri Parity Checklist

Use this document as the manual egui-to-Tauri parity gate before Phase 8
cutover and egui removal. Keep every item as `Unchecked`, `Pass`, `Fail`,
`Blocked`, or `N/A`, and add notes as you verify behavior.

Phase 8 must not start until every required item is `Pass` or has a named
follow-up migration phase.

## Reference Screenshots

![egui palette reference](assets/egui-palette-reference.png)

![egui settings general reference](assets/egui-settings-general-reference.png)

![egui settings installed extensions reference](assets/egui-settings-installed-extensions-reference.png)

![egui settings marketplace reference](assets/egui-settings-marketplace-reference.png)

## Setup

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Record the current branch and dirty state with `git status --short`. | Unchecked |  |  |
| Run the egui baseline from repo root with `cargo run`. | Unchecked |  |  |
| Close egui before running Tauri to avoid hotkey and tray conflicts. | Unchecked |  |  |
| Run the Tauri path with `cd apps/desktop-tauri && bun run tauri dev`. | Unchecked |  |  |
| Use the same `%APPDATA%\OmniPalette` config and extension state for both apps. | Unchecked |  |  |
| Compare the palette and settings surfaces against the reference screenshots above. | Unchecked |  |  |

## Palette And Hotkey Parity

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| App starts without showing the palette immediately. | Unchecked |  |  |
| Activation shortcut opens the palette-only surface. | Unchecked |  |  |
| Second activation hides the palette. | Unchecked |  |  |
| Ignored foreground apps receive shortcut passthrough instead of opening the palette. | Unchecked |  |  |
| Palette captures the foreground target before focusing itself. | Unchecked |  |  |
| Search input focuses automatically when palette opens. | Unchecked |  |  |
| Palette omits the temporary command-count and `Run selected` action bar; Enter and click execute selected commands. | Unchecked | Code fix added; manual recheck pending against current Tauri window. | 2026-05-17 |
| Typing filters command rows. | Unchecked |  |  |
| Command ordering matches egui for the same query and context. | Unchecked |  |  |
| Shortcut text appears on command rows where egui shows it. | Unchecked |  |  |
| Label match highlighting appears for filtered results. | Unchecked |  |  |
| Arrow Up and Arrow Down move selection with wraparound. | Unchecked |  |  |
| Enter executes the selected command. | Unchecked |  |  |
| Clicking a command executes it. | Unchecked |  |  |
| Escape hides the palette. | Unchecked |  |  |
| Focus loss hides the palette. | Unchecked |  |  |
| Successful execution hides the palette. | Unchecked |  |  |
| Failed execution keeps the palette visible and shows an error. | Unchecked |  |  |
| Palette uses one hidden command-results scroller, avoids header overlap, smooth-scrolls arrow-key selection near one third from the top, and fades both top and bottom edges. | Unchecked | Code fix added; manual recheck pending against current Tauri window. | 2026-05-17 |
| Palette window height remains fixed while filtering from many results down to few results. | Unchecked | Code fix added; manual recheck pending against current Tauri window. | 2026-05-17 |
| Backend command `Omni Palette: Reload extensions` appears with normal backend rows. | Unchecked |  |  |
| Fixed footer action `Refresh extensions` appears below command results. | Unchecked |  |  |
| Fixed footer action `Open settings for Omni Palette` appears below `Refresh extensions`. | Unchecked |  |  |
| `Refresh extensions` reloads extensions and hides only on success. | Unchecked |  |  |
| `Open settings for Omni Palette` hides the palette and opens Settings. | Unchecked |  |  |

## Command Execution

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Static shortcut commands focus the original target window and send keys. | Unchecked |  |  |
| Shortcut-sequence commands execute in order. | Unchecked |  |  |
| WASM plugin commands execute through the plugin host. | Unchecked |  |  |
| Plugin failure returns a controlled error. | Unchecked |  |  |
| Stale command IDs fail cleanly after the palette session closes. | Unchecked |  |  |
| Reloading extensions refreshes available commands. | Unchecked |  |  |
| Downloaded and bundled enablement changes affect command availability after reload. | Unchecked |  |  |

## Guide Mode

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Command behavior `Guide` causes eligible shortcut commands to start guide mode. | Unchecked |  |  |
| Guide window is separate from the palette and shown only during guide. | Unchecked |  |  |
| Palette hides when guide starts. | Unchecked |  |  |
| Guide focuses the captured target window. | Unchecked |  |  |
| Guide displays command label and shortcut keycaps. | Unchecked |  |  |
| Pressing activation shortcut while guide is active runs the stored command. | Unchecked |  |  |
| Escape cancels guide. | Unchecked |  |  |
| Pressing the shown captured shortcut cancels guide and forwards it to the target app. | Unchecked |  |  |
| Shortcut-sequence guide shows fallback or sequence text without invalid hotkey capture. | Unchecked |  |  |
| Guide expires automatically after the expected timeout. | Unchecked |  |  |

## Settings Surface

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Settings opens as a separate surface, not inside the palette. | Unchecked |  |  |
| Hotkey still opens the palette only while Settings is open. | Unchecked |  |  |
| Settings sidebar has General, Manage Extensions, and Marketplace. | Unchecked |  |  |
| General page has Appearance theme options: System, Light, Dark. | Unchecked |  |  |
| General page has Activation shortcut display. | Unchecked |  |  |
| Record captures the next non-modifier shortcut. | Unchecked |  |  |
| Reset restores the backend default activation shortcut. | Unchecked |  |  |
| Saving activation shortcut updates the active hotkey listener. | Unchecked |  |  |
| Command Behavior supports Execute and Guide. | Unchecked |  |  |
| Debug section has `Pop up debugger`. | Unchecked |  |  |
| Storage/config path is visible. | Unchecked |  |  |
| Save, Discard, dirty state, success state, and failure state behave like egui. | Unchecked |  |  |

## Extension Management

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Manage Extensions shows Bundled Defaults. | Unchecked |  |  |
| Manage Extensions shows Downloaded Extensions. | Unchecked |  |  |
| Bundled static extensions are listed. | Unchecked |  |  |
| Bundled WASM plugins are listed. | Unchecked |  |  |
| Downloaded extensions are listed when installed. | Unchecked |  |  |
| Empty downloaded state appears when none are installed. | Unchecked |  |  |
| Extension rows show a separate Enabled/Disabled status pill beside the enablement toggle, matching the egui reference. | Unchecked | Code fix added; manual recheck pending against egui reference. | 2026-05-17 |
| Bundled extensions can be enabled and disabled. | Unchecked |  |  |
| Bundled extensions cannot be uninstalled. | Unchecked |  |  |
| Downloaded extensions can be enabled and disabled. | Unchecked |  |  |
| Downloaded extensions can be uninstalled. | Unchecked |  |  |
| Extensions with settings expose enabled Settings buttons. | Unchecked |  |  |
| Extensions without settings do not open settings panels. | Unchecked |  |  |
| Extension settings panels load current and default values. | Unchecked |  |  |
| Toggle settings save and persist. | Unchecked |  |  |
| Entry-list settings add, edit, remove, save, and persist. | Unchecked |  |  |
| Saving extension settings reloads runtime state. | Unchecked |  |  |

## Marketplace

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| Marketplace shows catalog source enable toggle. | Unchecked |  |  |
| Owner, repo, branch, and catalog path fields are editable. | Unchecked |  |  |
| Save Source persists catalog source settings. | Unchecked |  |  |
| Refresh Catalog fetches catalog entries. | Unchecked |  |  |
| Refresh failure preserves the last good visible catalog. | Unchecked |  |  |
| Reload Extensions action works from Marketplace. | Unchecked |  |  |
| Catalog search matches name, id, description, and keywords. | Unchecked |  |  |
| Available extension rows show name, version, description, and action. | Unchecked |  |  |
| Install installs a supported static catalog extension. | Unchecked |  |  |
| Install failure preserves previous extension state. | Unchecked |  |  |
| Installed, update, and reinstall status is understandable. | Unchecked |  |  |

## Debug Diagnostics

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| `Pop up debugger` opens a separate debug window. | Unchecked |  |  |
| Debug window can close independently. | Unchecked |  |  |
| Debug shows foreground window and process info. | Unchecked |  |  |
| Debug shows interaction tags and text-input state. | Unchecked |  |  |
| Debug shows ignored-app status. | Unchecked |  |  |
| Debug shows command candidate counts. | Unchecked |  |  |
| Debug shows latest palette query and filter rows. | Unchecked |  |  |
| Debug shows capped background windows. | Unchecked |  |  |
| Manual refresh updates the snapshot. | Unchecked |  |  |
| Opening debug does not dirty settings or alter palette session state. | Unchecked |  |  |

## Tray, Quit, And Packaging Gate

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| egui tray menu baseline is documented: Open Palette, Settings, Reload Extensions, Quit. | Unchecked |  |  |
| Tauri has equivalent tray menu behavior before cutover. | Unchecked |  |  |
| Tauri tray Open Palette opens and focuses the palette. | Unchecked |  |  |
| Tauri tray Settings opens and focuses Settings. | Unchecked |  |  |
| Tauri tray Reload Extensions reloads runtime state. | Unchecked |  |  |
| Tauri tray Quit exits cleanly and stops background listeners. | Unchecked |  |  |
| Tauri app packages successfully with expected metadata. | Unchecked |  |  |
| Installed or bundled Tauri build launches hidden and responds to hotkey. | Unchecked |  |  |
| README and AGENTS instructions point to Tauri as primary only after this passes. | Unchecked |  |  |

## Cutover Decision

| Item | Status | Notes | Verified on |
| --- | --- | --- | --- |
| All required parity checks pass. | Unchecked |  |  |
| Any failed checks have a named follow-up phase. | Unchecked |  |  |
| No egui-only production behavior remains except items explicitly accepted for removal. | Unchecked |  |  |
| Phase 8 cutover and egui removal are approved to begin. | Unchecked |  |  |
