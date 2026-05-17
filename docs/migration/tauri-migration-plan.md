# Omni Palette Tauri Migration Plan

> Canonical migration memory: before doing any Omni Palette egui-to-Tauri
> migration work, read this document first.
>
> React-to-Svelte migration work is complete. Future Tauri frontend work uses
> Svelte with Vite, TypeScript, Tailwind, and Bun. Preserve the existing egui
> app until the Svelte/Tauri app reaches functional parity.

## Migration Status

- Current migration position: Phase 7 - Debug Overlay And Diagnostics
  is complete.
- Next phase: Phase 8 - Cutover And egui Removal.
- Completed: React-to-Svelte Phases 0-3, Phase 4A, Phase 4B, Phase 4C,
  Phase 4D, Phase 5A, Phase 5B, Phase 6A, Phase 6A.1, Phase 6A.2, and
  Phase 6B, Phase 6C.1, Phase 6C.2, Phase 6C.3, and Phase 7.
- Last updated: 2026-05-17.
- Update this section whenever the migration moves to a new phase.

## Goal

Migrate Omni Palette from the current egui/eframe UI to a Tauri v2 desktop app
with a Svelte frontend. The migration should happen in phases while the existing
egui app stays runnable until the Tauri version reaches functional parity.

## Current State

- `apps/desktop-tauri/` contains the Tauri v2 app with Svelte, TypeScript,
  Vite, Tailwind, and Bun.
- React dependencies and TSX entrypoints have been removed from the Tauri
  frontend.
- The Tauri shell exposes these invoke commands:
  - `health_check`
  - `get_palette_bootstrap`
  - `search_commands`
  - `execute_command`
  - `get_hotkey_status`
  - `get_window_lifecycle_status`
  - `hide_palette_window`
  - `start_guide`
  - `cancel_guide`
  - `get_guide_status`
  - `get_settings_bootstrap`
  - `save_runtime_settings`
  - `reload_runtime_state`
  - `show_settings_window`
  - `get_extensions_bootstrap`
  - `set_extension_enabled`
  - `uninstall_extension`
  - `refresh_extension_catalog`
  - `install_catalog_extension`
  - `get_extension_settings`
  - `save_extension_settings`
  - `show_debug_overlay`
  - `close_debug_overlay`
  - `get_debug_overlay_status`
  - `get_debug_snapshot`
- The temporary Phase 6A tabbed shell has been split: the Tauri `main` window is
  palette-only, and Settings renders in a distinct hidden-by-default `settings`
  window.
- Settings can record, reset, save, and immediately refresh the active Tauri
  activation shortcut through the existing hotkey listener path.
- Settings has egui-style navigation for General, Manage Extensions, and
  Marketplace.
- Manage Extensions lists bundled and downloaded extensions, supports
  enable/disable mutations, supports downloaded uninstall, and reloads runtime
  state after successful mutations.
- Marketplace contains catalog source controls, catalog refresh, reload,
  catalog search, static extension install/update/reinstall actions, and
  controlled failure states that preserve the last visible catalog or extension
  rows.
- Extension Settings buttons now open per-extension settings panels for bundled
  static extensions, downloaded static extensions, and bundled WASM plugins that
  expose settings. These panels save to the existing AppData extension settings
  TOML path and reload runtime state after successful saves.
- General Settings includes a `Pop up debugger` action that opens a separate
  hidden-by-default `debug` window with foreground context, interaction tags,
  ignored-app status, command candidate counts, palette filter rows, and
  background windows.
- The existing egui app remains the production UI until final Tauri cutover.

## Direction

- Use Svelte with Vite, not SvelteKit.
- Keep TypeScript for frontend contracts.
- Keep Tailwind for the Tauri frontend unless a later phase explicitly changes
  styling.
- Keep Bun for frontend package management and scripts.
- Keep Rust DTOs and Tauri invoke command names stable unless a later phase
  explicitly changes them.
- Do not reintroduce React dependencies or React-specific migration work.
- Do not remove egui/eframe until Phase 8 final cutover.

## egui surface behavior baseline

The Tauri migration must converge on these egui surface boundaries:

- Hotkey activation opens and hides the compact palette surface only.
- The palette surface is a hidden-by-default, decorationless, always-on-top
  viewport sized around visible command rows.
- Settings is a separate surface opened by a separate settings event/action.
- In egui, `UiSignal::Show` opens the palette, while
  `PlatformUiAction::OpenSettings` marks `SettingsState` open and shows the
  `omni_palette_settings` settings viewport.
- The palette may include an "Open settings for Omni Palette" fixed action, but
  choosing it hides the palette and opens/focuses the separate settings surface.
- Settings must not remain embedded in the hotkey palette surface for final
  parity or cutover.

Resolved temporary Tauri deviation:

- The one-window tabbed Tauri shell was allowed only as the Phase 6A temporary
  development shell.
- Phase 6A.1 split Palette and Settings into distinct Tauri surfaces before
  Phase 6B.

## egui Reference Screenshots

These screenshots are the feature and surface-structure baseline for the
Tauri/Svelte migration. Styling polish is not required in the current phase,
but visible controls, rows, commands, actions, pages, and state indicators
should not be dropped.

![egui palette reference](assets/egui-palette-reference.png)

![egui settings general reference](assets/egui-settings-general-reference.png)

![egui settings installed extensions reference](assets/egui-settings-installed-extensions-reference.png)

![egui settings marketplace reference](assets/egui-settings-marketplace-reference.png)

## Feature Parity Checklist From References

Palette reference:

- Command search input remains the primary hotkey surface.
- Visible command rows include command labels and right-aligned shortcut text
  when present.
- Backend command rows include `Omni Palette: Reload extensions`.
- Fixed footer actions include `Refresh extensions`.
- Fixed footer actions include `Open settings for Omni Palette`.
- Choosing the settings fixed action hides the palette and opens/focuses the
  separate Settings window.

General Settings reference:

- Settings uses a distinct window with sidebar navigation.
- General page includes Appearance theme selection.
- General page includes Activation shortcut display, Record, and Reset.
- General page includes Command Behavior selection for Execute and Guide.
- General page includes Debug popup access.
- General page includes Storage/config path status.
- General page includes status, Save Settings, and Discard Changes controls.

Installed Extensions reference:

- Manage Extensions page lists bundled defaults separately from downloaded
  extensions.
- Bundled extensions show enabled/disabled badges, plugin/source labels, and
  enable/disable toggles.
- Extension Settings buttons appear for extensions that expose settings.
- Downloaded extensions section shows installed items or the empty state.
- Downloaded extensions can be enabled/disabled and removed where supported.

Marketplace reference:

- Marketplace page includes GitHub catalog source enable toggle and editable
  owner, repo, branch, and catalog path fields.
- Marketplace page includes Save Source, Refresh Catalog, and Reload Extensions
  actions.
- Available Extensions section includes catalog search.
- Available extension rows include name, version, description, and Install
  action.

## File And Folder Inventory

### Tauri Frontend

- `apps/desktop-tauri/src/App.svelte`
  - Palette-only hotkey surface for the Tauri `main` window, command rows,
    keyboard navigation, execution, guide start, and the fixed settings action.
- `apps/desktop-tauri/src/Settings.svelte`
  - Runtime settings surface rendered only for the Tauri `settings` window.
- `apps/desktop-tauri/src/Guide.svelte`
  - Compact guide-mode window rendered only for the Tauri `guide` window.
- `apps/desktop-tauri/src/commands.ts`
  - TypeScript API boundary for Tauri invokes, DTO mirrors, formatting helpers,
    selection helpers, label highlight helpers, and guide helpers.
- `apps/desktop-tauri/src-tauri/**`
  - Tauri Rust crate, invoke registration, hotkey bridge, window lifecycle, and
    frontend-facing backend state, including the guide lifecycle.

### Shared Rust Backend

- `src/backend_contract.rs`
  - Shared backend-owned command discovery, filtering, command IDs, execution
    result DTOs, runtime status DTOs, and command session state.
- `src/runtime_state.rs`
  - Shared runtime loading and reload state for config, extension discovery,
    registries, plugin registry, and ignored-process config.
- `src/core/**`, `src/config/**`, `src/domain/**`, and
  `src/platform/windows/**`
  - Keep as the backbone for extension loading, plugins, fuzzy search, context
    lookup, hotkeys, and command sending.

### egui Surfaces To Preserve Until Cutover

- `src/ui/app.rs`
- `src/ui/palette.rs`
- `src/ui/settings.rs`
- `src/ui/guide.rs`
- `src/ui/debug_overlay.rs`
- `src/platform/ui_support.rs`
- `src/platform/windows/ui_support.rs`

These remain functional until the Tauri app reaches parity. Do not delete or
degrade them before Phase 8.

## Completed Phase History

### Phase 0: Documentation And Priority Reset

Status: complete.

- Created the Svelte-first plan when React-to-Svelte became the priority.
- Paused the old React-shaped Tauri plan.
- Updated agent-facing instructions to prioritize Svelte/Tauri work.

### Phase 1: Svelte Tooling Swap

Status: complete.

- Replaced React, React DOM, React types, and the React Vite plugin with
  Svelte tooling.
- Preserved Bun, TypeScript, Vite, Tailwind, Tauri scripts, and the Tauri shell.
- Verified frontend build and tests with Svelte.

### Phase 2: Port The Wireframe To Svelte

Status: complete.

- Rebuilt the previous frontend wireframe as `App.svelte`.
- Kept the palette shell, query input, command rows, selected row state, status
  strip, temporary settings tab, loading/error states, and execution result
  state.
- Kept `commands.ts` as the frontend API boundary.

### Phase 3: Svelte Parity Verification

Status: complete.

- Verified the Svelte frontend reached the previous bridge behavior.
- Confirmed React runtime dependencies and TSX entrypoints were removed.
- Resumed the broader egui-to-Tauri migration through Svelte.

### Phase 4A: Runtime State Foundation

Status: complete.

- Added shared runtime-state loading for config, extension discovery, registry
  state, plugin registry, and ignored-process names.
- Updated Tauri bootstrap status with runtime metadata.
- Preserved the egui app and deferred hotkeys/window lifecycle/execution.

### Phase 4B: Runtime Command Execution

Status: complete.

- Replaced deferred execution with real shortcut, shortcut-sequence, plugin, and
  reload command dispatch.
- Kept stale or unknown command IDs as controlled failures.
- Preserved existing Tauri invoke names and command result wire shape.

### Phase 4C: Hotkey Listener And Ignored-App Passthrough

Status: complete.

- Started the existing Windows hotkey listener from Tauri.
- Preserved ignored-app passthrough behavior.
- Added observable hotkey status without showing or hiding the Tauri window.

### Phase 4D: Tauri Window Lifecycle

Status: complete.

- Connected accepted hotkey activations to Tauri window show/hide/focus and
  basic positioning.
- Captured foreground context before focusing the Tauri window.
- Kept command search filtering against the captured open palette session.

### Phase 5A: Core Palette UX Parity

Status: complete.

- Added `hide_palette_window`.
- Added keyboard navigation, Enter execution, click-to-run, Escape close,
  focus-loss close, successful-execution hide, and highlighted label matches.
- Kept settings, tray work, guide mode, extension management, and egui removal
  out of scope.

### Phase 5B: Guide Mode Usability And Refined Palette Positioning

Status: complete.

Completed:

- Added guide hints to command rows for shortcut and shortcut-sequence commands.
- Added `start_guide`, `cancel_guide`, and `get_guide_status` invokes plus the
  `omni://palette-guide` event.
- Added a hidden-by-default `guide` Tauri window rendered by Svelte.
- Added guide lifecycle state for start, complete, cancel, captured-shortcut
  passthrough, and expiry.
- Connected guide hotkeys to the existing Windows hotkey listener and
  passthrough path.
- Refined main palette positioning and sizing toward the egui palette constants.

Out of scope, still deferred:

- Settings UI, shortcut recorder UI, extension management, tray behavior, debug
  overlay, packaging cutover, and egui removal.

### Phase 6A: Runtime Settings Foundation

Status: complete.

Completed:

- Added `get_settings_bootstrap`, `save_runtime_settings`, and
  `reload_runtime_state` invokes.
- Added a shared runtime config save path that writes the existing AppData TOML
  shape and updates in-memory runtime config only after successful saves.
- Added saveable Svelte controls for command behavior, appearance theme, GitHub
  catalog source, config status, discard, and reload.
- Kept activation shortcut display-only.
- Kept marketplace install/uninstall, extension toggles, extension-specific
  settings, tray work, diagnostics, packaging cutover, and egui removal out of
  scope.

### Phase 6A.1: Palette And Settings Surface Separation

Status: complete.

Completed:

- Split the Tauri `main` hotkey window into a palette-only surface without
  Settings tabs, settings forms, or backend/status development chrome.
- Moved the Phase 6A runtime settings UI into a separate hidden-by-default
  Tauri `settings` window/component.
- Kept existing settings DTOs and invokes unchanged.
- Added `show_settings_window` to show/focus the `settings` window.
- Restored an egui-like "Open settings for Omni Palette" fixed palette action
  that hides the palette before opening/focusing Settings.
- Kept activation shortcut recording, tray behavior, marketplace install,
  extension toggles, diagnostics, and cutover out of scope.

### Phase 6A.2: Palette Fixed Action Parity

Status: complete.

Completed:

- Added an egui-like fixed `Refresh extensions` footer action in the Tauri
  palette.
- Kept the existing backend `Omni Palette: Reload extensions` command row.
- Preserved the fixed `Open settings for Omni Palette` row and separate Settings
  window behavior.
- Reused the existing `reload_runtime_state` invoke.
- Kept activation shortcut recording, settings page work, marketplace work,
  debug overlay work, and styling polish out of scope.

### Phase 6B: Activation Shortcut Settings

Status: complete.

Completed:

- Added structured activation shortcut DTOs to settings bootstrap and save
  requests while preserving `activation_hint` display compatibility.
- Added backend save handling for activation shortcut changes, including hotkey
  listener update, controlled failure when registration fails, and rollback if
  config persistence fails after listener update.
- Updated `get_hotkey_status` activation metadata after successful shortcut
  changes.
- Replaced the read-only Svelte Settings shortcut display with Record and Reset
  controls.
- Added browser keyboard-event mapping for supported Rust shortcut keys and
  ignored modifier-only or unsupported keydown events while recording.
- Kept Settings sidebar/extension management, marketplace work, debug overlay
  UI, tray work, styling polish, packaging cutover, and egui removal out of
  scope.

### Phase 6C.1: Settings Sidebar And Installed Extensions Foundation

Status: complete.

Completed:

- Added egui-style Settings sidebar navigation for General, Manage Extensions,
  and Marketplace.
- Kept Phase 6B activation shortcut, appearance, command behavior, storage,
  save, and discard controls on General.
- Moved GitHub catalog source controls to Marketplace while deferring catalog
  refresh, search, and install to Phase 6C.2.
- Added a shared Rust extension-management snapshot path for bundled static
  extensions, bundled WASM plugins, downloaded install state, enabled/disabled
  state, settings availability flags, and install-root errors.
- Added Tauri invokes for extension bootstrap, enable/disable, and uninstall.
- Added bundled and downloaded extension sections, enabled/disabled badges,
  toggles, disabled extension Settings placeholders, and downloaded empty state
  to the Svelte Settings window.
- Reloaded runtime state after successful extension mutations and returned
  controlled failures without replacing the previous UI rows.
- Kept catalog refresh/search/install, extension-specific settings panels,
  debug overlay UI, tray work, styling polish, packaging cutover, and egui
  removal out of scope.

### Phase 6C.2: Marketplace Catalog Refresh And Install

Status: complete.

Completed:

- Reused the existing extension install service for catalog refresh and static
  package install in the Tauri backend.
- Added a test-injectable Marketplace service and cached the last successful
  catalog/source pair in Tauri state.
- Added `refresh_extension_catalog` and `install_catalog_extension` invokes.
- Added catalog entry DTOs and frontend API wrappers.
- Added Marketplace `Save Source`, `Refresh Catalog`, `Reload Extensions`,
  catalog search, available extension rows, and static install/update/reinstall
  actions.
- Preserved previous catalog rows on refresh failure and previous extension rows
  on install failure.
- Kept extension-specific settings panels, debug overlay UI, tray work, styling
  polish, packaging cutover, and egui removal out of scope.

### Phase 6C.3: Extension-Specific Settings Panels

Status: complete.

Completed:

- Added Tauri extension settings DTOs and `get_extension_settings` /
  `save_extension_settings` invokes.
- Reused the existing egui extension settings schema, default resolution, TOML
  load/save, and AppData settings paths.
- Loaded bundled static, downloaded static, and bundled WASM plugin settings
  schemas through the existing runtime paths.
- Enabled Settings buttons for extensions that expose settings and opened an
  in-window Svelte settings panel.
- Added toggle settings, entry-list settings, category grouping, category
  toggles, reset defaults, dirty state, save, close, loading, success, and
  failure states.
- Reloaded runtime state after successful extension setting saves.
- Kept debug overlay UI, tray work, styling polish, packaging cutover, and egui
  removal out of scope.

## Remaining Phases


### Phase 7: Debug Overlay And Diagnostics

Status: complete.

Purpose:

- Restore developer diagnostics after the core user-facing path is working.

Completed:

- Restored the General Settings Debug popup entry point without making runtime
  settings dirty.
- Added a separate hidden-by-default Tauri `debug` window and Svelte
  `DebugOverlay` surface.
- Added Tauri debug invokes: `show_debug_overlay`, `close_debug_overlay`,
  `get_debug_overlay_status`, and `get_debug_snapshot`.
- Added debug overlay lifecycle status with injectable controller coverage for
  show/focus/hide and controlled failures.
- Added debug snapshot DTOs for foreground window, active interaction tags,
  text-input state, ignored-app status, command candidate counts, latest
  palette filter rows, and capped background windows.
- Preserved periodic-style refresh in the Svelte debug window through polling
  while the window is open.
- Keep runtime telemetry logs.
- Keep debug UI secondary to palette and settings parity.

Acceptance criteria:

- Debug context can show active process, context tags, command candidate counts,
  ignored-app status, and latest palette rows.
- Debug telemetry remains useful for long-running stability investigations.

### Phase 8: Cutover And egui Removal

Purpose:

- Make Tauri the main app only after parity is proven.

Scope:

- Switch default run/build documentation to the Tauri app.
- Replace old tray/window support with Tauri equivalents.
- Verify Tauri packaging expectations.
- Remove egui/eframe dependencies after no production code uses them.
- Remove or archive old egui UI modules.
- Update README and developer docs.

Acceptance criteria:

- Tauri app covers the current user workflows.
- Tauri palette and Settings surfaces have been checked against the embedded
  egui reference screenshots for missing features and controls.
- `cargo test` passes.
- Frontend checks pass.
- No egui/eframe production dependency remains.
- README and docs describe the Tauri workflow.

## Non-Goals Before Cutover

- Do not rewrite the extension system.
- Do not rewrite the WASM plugin host.
- Do not rewrite fuzzy search unless serialization requires small adapters.
- Do not remove egui until the Tauri app is functionally ready.
- Do not redesign visual styling deeply before behavior parity is secure.

## Verification Checklist

Use this checklist throughout the migration:

- This file is the single canonical migration plan.
- No active migration doc points to React as the Tauri frontend.
- Existing egui app remains runnable until final cutover.
- Tauri app launches on Windows.
- Global hotkey opens and hides the palette-only surface.
- Settings opens as a distinct surface, not inside the hotkey palette.
- Palette exposes both fixed footer actions: `Refresh extensions` and
  `Open settings for Omni Palette`.
- Ignored app passthrough still works.
- Palette search result ordering is unchanged.
- Shortcut commands focus the correct target and send keys.
- Plugin commands still execute through the WASM host.
- Settings General page covers Appearance, Activation, Command Behavior, Debug,
  Storage, save, discard, and status controls.
- Settings Manage Extensions page covers bundled and downloaded extension
  enablement, status, settings, and empty states.
- Settings Marketplace page covers catalog source, refresh, reload, search, and
  install controls.
- Runtime settings save to existing AppData paths, including activation
  shortcut, command behavior, theme, and catalog source.
- Extension reload does not replace the last good registry on failure.
- Long-running idle behavior stays quiet in CPU, memory, and thread count.
