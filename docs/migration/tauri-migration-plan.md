# Tauri Migration Plan

> Canonical migration memory: before doing any Omni Palette Tauri migration work,
> read this document first and align the task with the phases below.

## Migration Status

- Current phase: Phase 3 - Backend Contract Extraction (completed)
- Last updated: 2026-05-13
- Update this section whenever the migration moves to a new phase.

## Goal

Migrate Omni Palette from the current egui/eframe UI to a Tauri desktop app with
React, TypeScript, and Tailwind. The migration should happen in phases while the
existing egui app stays runnable until the Tauri version reaches functional
parity.

## Current Direction

- Use Tauri v2 as the desktop shell.
- Use React and TypeScript for the frontend.
- Use Tailwind for styling, starting with wireframe-level UI only.
- Use Bun for Phase 2 frontend package management and scripts.
- Keep the Rust runtime, Windows integration, extension loading, plugin host,
  search, and command execution logic as the system backend.
- Build the new Tauri app beside the current egui app first, then cut over only
  after parity is proven.

Relevant Tauri references:

- https://v2.tauri.app/start/create-project/
- https://v2.tauri.app/start/frontend/vite/
- https://v2.tauri.app/develop/calling-rust/
- https://v2.tauri.app/develop/calling-frontend/
- https://v2.tauri.app/plugin/global-shortcut/
- https://v2.tauri.app/learn/system-tray/

## File And Folder Inventory

### Migrate First

These files define the current UI/runtime boundary or contain egui-specific
desktop shell behavior. They should be examined first when planning actual code
changes.

- `src/main.rs`
  - Current entrypoint, runtime bridge, hotkey loop, UI signal handling, settings
    side effects, extension reload/install events, and command execution
    closures.
  - Needs to be split so Tauri can own the window/event surface while reusable
    runtime logic remains in Rust modules.
- `src/ui/app.rs`
  - Current egui app shell, `UiSignal`, `UiEvent`, `Command`, visibility state,
    palette window control, guide state, debug overlay wiring, and settings
    wiring.
  - Needs to become a Tauri-safe typed backend/frontend contract rather than an
    egui app.
- `src/platform/ui_support.rs`
  - Current platform UI action facade, palette window token, tray action channel,
    and egui-dependent runtime construction.
  - Needs a Tauri-compatible equivalent for tray actions, foreground checks, and
    window focus behavior.
- `src/platform/windows/ui_support.rs`
  - Current Windows tray creation and egui/raw-window-handle integration.
  - Needs to be replaced or adapted to Tauri window handles and Tauri tray APIs.
- `src/theme/mod.rs`
  - Current theme tokens are tied to egui color/style types.
  - Needs either frontend CSS/Tailwind tokens or serializable theme data if theme
    settings remain Rust-owned.

### Rebuild In React

These are UI implementations that should not be ported line-for-line. They
should be recreated as React components after the backend contract is stable.

- `src/ui/palette.rs`
  - Rebuild as command palette React components: input, rows, selection,
    highlighted matches, fixed actions, and empty state.
- `src/ui/settings.rs`
  - Rebuild as React settings screens after palette/runtime contracts are stable.
- `src/ui/guide.rs`
  - Rebuild as a lightweight always-on-top guide window or overlay.
- `src/ui/debug_overlay.rs`
  - Rebuild after core palette and settings behavior are working.
- `src/ui/components/**`
  - Replace with React/Tailwind components.

### Keep Mostly Unchanged

These folders should remain the backbone of the system. Changes should be small
and driven by serialization or API boundary needs.

- `src/core/**`
  - Keep extension discovery, install state, registry, plugin runtime, command
    filtering, fuzzy search, and performance helpers.
- `src/config/**`
  - Keep runtime config, extension schemas, and ignored app config.
- `src/domain/**`
  - Keep command, action, hotkey, focus, and OS domain types.
- `src/platform/windows/context/**`
  - Keep foreground/background window discovery and active interaction detection.
- `src/platform/windows/mapper/**`
  - Keep keyboard mapping.
- `src/platform/windows/receiver/**`
  - Keep current global hotkey receiver at first. Evaluate Tauri global shortcut
    only after existing passthrough and guide behavior are preserved.
- `src/platform/windows/sender/**`
  - Keep shortcut sending and sequence execution.
- `extensions/**`
  - Keep bundled static extensions, bundled WASM plugins, registry packages, and
    catalog data.
- `xtask/**`
  - Keep packaging utilities unless Tauri packaging later requires integration.

### Add New

- `apps/desktop-tauri/`
  - New Tauri app root.
- `apps/desktop-tauri/src-tauri/`
  - New Tauri Rust crate, likely added as a workspace member.
- `apps/desktop-tauri/src/`
  - React, TypeScript, and Tailwind frontend.

## Phases

### Phase 1: Planning And Inventory

Status: this document.

Purpose:

- Establish the migration direction.
- Mark which files and folders move first, which are rebuilt, and which remain.
- Keep future migration work anchored to a shared written plan.

Acceptance criteria:

- `docs/migration/tauri-migration-plan.md` exists.
- The document identifies the first migration targets and protected backend
  areas.
- Future Tauri migration tasks refer back to this document before making code
  changes.

### Phase 2: Minimal Tauri Wireframe

Purpose:

- Create a Tauri v2 shell with React, TypeScript, Vite, and Tailwind.
- Prove the frontend/backend toolchain works without migrating real behavior yet.

Scope:

- Add a minimal Tauri app under `apps/desktop-tauri/`.
- Add a simple backend health command exposed through Tauri invoke.
- Add a wireframe palette screen with:
  - Search input.
  - Static command rows.
  - Selected row styling.
  - Empty state.
  - Settings placeholder view.
- Keep styling intentionally plain and Tailwind-based.
- Use Bun for dependency installation and frontend scripts.

Out of scope:

- Real global hotkeys.
- Real command execution.
- Real extension settings.
- egui removal.

Acceptance criteria:

- The Tauri app launches.
- React can call a Rust command and render the result.
- The wireframe makes the intended palette structure visible.
- The existing egui app still builds and runs.

### Phase 3: Backend Contract Extraction

Purpose:

- Build the first real bridge between the Tauri React frontend and the existing
  Rust runtime without migrating hotkeys, settings, or full command execution.
- Split the current binary-only Rust crate into a reusable library boundary so
  the Tauri crate can call shared Omni Palette backend code instead of
  duplicating runtime logic.
- Replace Phase 2 static frontend commands with backend-provided serializable
  command DTOs and backend-owned command IDs.

Scope:

- Add a root library target with `src/lib.rs`.
  - Export reusable modules such as `config`, `core`, `domain`, `platform`, and
    `theme` from the library.
  - Keep egui-specific UI modules and the egui runtime bridge in `src/main.rs`.
  - Update `src/main.rs` to import shared modules through the library so the
    existing egui app continues to run unchanged.
- Add a backend contract module for serializable frontend communication:
  - `CommandId`
  - `PaletteSessionId`
  - `CommandDto`
  - `MatchRangeDto`
  - `PaletteSnapshotDto`
  - `PaletteBootstrapDto`
  - `CommandExecutionResultDto`
- Add a Rust command-session service that:
  - Queries `MasterRegistry` using the current Windows context.
  - Converts `UnitAction` values into serializable command DTOs.
  - Stores Rust-only executable command records behind generated opaque command
    IDs.
  - Uses existing `core::command_filter` behavior for query filtering and match
    ranges.
  - Includes a built-in reload-extensions command.
- Add Tauri commands:
  - `get_palette_bootstrap`
  - `search_commands`
  - `execute_command`
- In Phase 3, `execute_command` should support safe built-in commands such as
  reload extensions. Shortcut-backed and plugin-backed actions may return a
  clear "deferred until runtime integration" result; full execution belongs to
  Phase 4.
- Update React to load commands from `search_commands` instead of
  `sampleCommands`, while preserving the Phase 2 wireframe layout and selection
  behavior.

Interfaces:

- Backend owns command discovery, filtering, sorting, match ranges, and command
  ID generation.
- Frontend owns query text, selected command ID, rendering, and invoking backend
  commands.
- DTO fields use serde-compatible snake_case names on the wire, and TypeScript
  types match those names exactly.
- Command IDs are opaque strings to React; React must not infer runtime behavior
  from them.
- Keep `health_check` as a simple smoke-test command.

Acceptance criteria:

- The root crate exposes reusable backend modules through `src/lib.rs`.
- The existing egui app remains functional after the library split.
- The Tauri frontend receives backend-generated command DTOs instead of static
  sample rows.
- Empty and non-empty searches are served by Rust using existing command filter
  behavior.
- Unknown or stale command IDs return a controlled result instead of panicking.
- Built-in reload can be represented and dispatched without frontend-owned
  closures.
- Global hotkeys, palette show/hide events, guide mode, settings save,
  extension settings UI, and full shortcut/plugin execution remain deferred to
  later phases.

### Phase 4: Runtime Integration

Purpose:

- Connect the Tauri shell to the existing Omni Palette runtime.

Scope:

- Reuse current startup loading:
  - Runtime config.
  - Extension discovery.
  - Bundled static extensions.
  - Installed extensions.
  - WASM plugins.
  - Ignored foreground app config.
- Reuse current Windows hotkey receiver initially.
- Wire hotkey activation to show/hide the Tauri palette window.
- Preserve ignored-app passthrough behavior.
- Preserve shortcut-backed and plugin-backed command execution.
- Keep extension reload behavior available.

Acceptance criteria:

- Pressing the configured hotkey opens the Tauri palette.
- Ignored foreground apps still receive the hotkey.
- A selected command can execute through the existing Rust runtime.
- Reload extensions still refreshes available commands.

### Phase 5: Palette Parity

Purpose:

- Make the React palette match the current functional behavior before migrating
  the larger settings surface.

Scope:

- Port fuzzy search display and highlighted matches.
- Port keyboard navigation.
- Port enter-to-run and click-to-run.
- Port fixed actions such as reload/settings/quit as appropriate.
- Port close-on-escape and close-on-focus-loss behavior.
- Port palette positioning using monitor work area data.
- Preserve guide-mode behavior for shortcut-backed commands.

Acceptance criteria:

- Normal command search feels equivalent to egui.
- Focused/background/global command priority remains correct.
- Favorite, priority, tag, and original order sorting remain correct.
- Guide mode remains usable for shortcut-backed commands.

### Phase 6: Settings And Extension Management

Purpose:

- Rebuild the settings window in React after core palette behavior works.

Scope:

- Port general runtime settings.
- Port activation shortcut display and recorder.
- Port appearance/theme setting.
- Port marketplace catalog refresh/install.
- Port installed extension enable/disable/uninstall.
- Port bundled extension enable/disable.
- Port extension-specific settings panels.
- Preserve save/reload behavior after settings changes.

Acceptance criteria:

- Runtime config saves to the same AppData location.
- Extension install state saves to the same AppData location.
- Extension settings save to the same AppData location.
- Saving settings refreshes runtime state where it does today.

### Phase 7: Debug Overlay And Diagnostics

Purpose:

- Restore developer diagnostics once the core user-facing path is working.

Scope:

- Port debug overlay data to Tauri events or commands.
- Preserve periodic debug snapshots in debug builds.
- Keep runtime telemetry logs.
- Make the debug UI secondary to palette and settings parity.

Acceptance criteria:

- Debug context can show active process, context tags, command ranking inputs,
  ignored-app status, and command rows.
- Debug telemetry remains useful for long-running stability investigations.

### Phase 8: Cutover And egui Removal

Purpose:

- Make Tauri the main app only after parity is proven.

Scope:

- Switch default run/build documentation to the Tauri app.
- Remove egui/eframe dependencies after no production code uses them.
- Remove or archive old egui UI modules.
- Replace old tray/window support with Tauri equivalents.
- Update README and developer docs.
- Verify packaging expectations for Tauri.

Acceptance criteria:

- Tauri app covers the current user workflows.
- `cargo test` passes.
- Frontend checks pass.
- No egui/eframe production dependency remains.
- README and docs describe the Tauri workflow.

## Non-Goals For Early Phases

- Do not rewrite the extension system.
- Do not rewrite the WASM plugin host.
- Do not rewrite fuzzy search unless serialization requires small adapters.
- Do not remove egui until the Tauri app is functionally ready.
- Do not redesign visual styling deeply in the first Tauri phase; start with a
  wireframe.

## Verification Checklist

Use this checklist throughout the migration:

- Existing egui app remains runnable until final cutover.
- Tauri app launches on Windows.
- Global hotkey opens the palette.
- Ignored app passthrough still works.
- Palette search result ordering is unchanged.
- Shortcut commands focus the correct target and send keys.
- Plugin commands still execute through the WASM host.
- Settings save to existing AppData paths.
- Extension reload does not replace the last good registry on failure.
- Long-running idle behavior stays quiet in CPU, memory, and thread count.
