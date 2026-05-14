# Omni Palette Tauri Migration Plan

> Canonical migration memory: before doing any Omni Palette egui-to-Tauri
> migration work, read this document first.
>
> React-to-Svelte migration work is complete. Future Tauri frontend work uses
> Svelte with Vite, TypeScript, Tailwind, and Bun. Preserve the existing egui
> app until the Svelte/Tauri app reaches functional parity.

## Migration Status

- Current phase: Phase 5B - Guide Mode And Refined Palette Positioning is
  implemented and being verified.
- Next phase after Phase 5B verification: Phase 6 - Settings And Extension
  Management.
- Completed: React-to-Svelte Phases 0-3, Phase 4A, Phase 4B, Phase 4C,
  Phase 4D, Phase 5A, and Phase 5B.
- Last updated: 2026-05-14.
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

## File And Folder Inventory

### Tauri Frontend

- `apps/desktop-tauri/src/App.svelte`
  - Main Svelte palette shell, status strip, placeholder settings view, command
    rows, keyboard navigation, execution, and lifecycle event handling.
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
  strip, settings placeholder, loading/error states, and execution result state.
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

## Remaining Phases

### Phase 6: Settings And Extension Management

Purpose:

- Rebuild the settings and extension management surface in Svelte after palette
  behavior is stable.

Scope:

- Runtime settings display/save.
- Activation shortcut display and recorder.
- Appearance/theme setting.
- Marketplace catalog refresh/install.
- Installed extension enable/disable/uninstall.
- Bundled extension enable/disable.
- Extension-specific settings panels.
- Runtime reload after settings changes where the egui app does so today.

Acceptance criteria:

- Runtime config saves to the same AppData location.
- Extension install state saves to the same AppData location.
- Extension settings save to the same AppData location.
- Saving settings refreshes runtime state where it does today.

### Phase 7: Debug Overlay And Diagnostics

Purpose:

- Restore developer diagnostics after the core user-facing path is working.

Scope:

- Port debug overlay data to Tauri events or commands.
- Preserve periodic debug snapshots in debug builds.
- Keep runtime telemetry logs.
- Keep debug UI secondary to palette and settings parity.

Acceptance criteria:

- Debug context can show active process, context tags, command ranking inputs,
  ignored-app status, and command rows.
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
- Global hotkey opens and hides the palette.
- Ignored app passthrough still works.
- Palette search result ordering is unchanged.
- Shortcut commands focus the correct target and send keys.
- Plugin commands still execute through the WASM host.
- Settings save to existing AppData paths once Phase 6 lands.
- Extension reload does not replace the last good registry on failure.
- Long-running idle behavior stays quiet in CPU, memory, and thread count.
