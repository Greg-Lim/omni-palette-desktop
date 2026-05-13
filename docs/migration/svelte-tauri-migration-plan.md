# Svelte-First Tauri Migration Plan

> Canonical migration memory: before doing any Omni Palette Tauri migration
> work, read this document first. This plan supersedes
> `docs/migration/tauri-migration-plan.md`.
>
> Stop React-specific Tauri migration work until this Svelte-first plan is
> complete or explicitly says to resume a React task. Preserve the existing
> egui app until the Svelte/Tauri app reaches functional parity.

## Migration Status

- Current priority: replace the React Tauri frontend with Svelte before any
  further Tauri parity phases.
- Current phase: Phase 0 - Documentation and priority reset.
- Last updated: 2026-05-13.
- Update this section whenever the Svelte migration moves to a new phase.

## Goal

Move the in-progress Tauri frontend from React to Svelte while keeping the
current Tauri v2 shell, TypeScript, Tailwind, Bun tooling, and Rust backend
contract. This is a migration within the larger egui-to-Tauri migration: the
frontend framework replacement comes first, then the broader Tauri parity work
continues only through the Svelte frontend.

## Current State

- `apps/desktop-tauri/` contains a Tauri v2 app created during the prior
  React-based migration.
- The frontend currently uses React, TypeScript, Vite, Tailwind, and Bun.
- The Rust side has started Phase 3 backend contract extraction and exposes
  Tauri invoke commands that the Svelte frontend should keep using:
  - `health_check`
  - `get_palette_bootstrap`
  - `search_commands`
  - `execute_command`
- The current egui app remains the functional production UI until the
  Svelte/Tauri app reaches parity.

## Direction

- Use Svelte with Vite, not SvelteKit.
- Keep TypeScript for frontend contracts.
- Keep Tailwind for the wireframe styling unless a later Svelte implementation
  plan explicitly changes styling.
- Keep Bun for frontend package management and scripts.
- Treat Rust DTOs and Tauri invoke command names as stable during this
  framework swap.
- Do not remove React dependencies until the Svelte implementation phase.
- Do not remove egui/eframe until the Tauri app reaches functional parity.

## File And Folder Inventory

### Replace First

- `apps/desktop-tauri/package.json`
  - Replace React dependencies and Vite plugin with Svelte equivalents.
  - Keep Bun, TypeScript, Vite, Tauri, Tailwind, PostCSS, and Autoprefixer.
- `apps/desktop-tauri/vite.config.ts`
  - Replace the React plugin with the Svelte Vite plugin.
- `apps/desktop-tauri/index.html`
  - Point the module script at the Svelte entrypoint.
- `apps/desktop-tauri/src/main.tsx`
  - Replace with a Svelte entrypoint, likely `src/main.ts`.
- `apps/desktop-tauri/src/App.tsx`
  - Rebuild as `src/App.svelte`.
- `apps/desktop-tauri/src/commands.test.ts`
  - Keep contract tests for the Tauri invoke wrapper and selection helper.

### Keep During The Framework Swap

- `apps/desktop-tauri/src/commands.ts`
  - Keep the DTO-shaped TypeScript contract unless Rust DTOs change.
- `apps/desktop-tauri/src/styles.css`
  - Keep Tailwind entry directives.
- `apps/desktop-tauri/src-tauri/**`
  - Keep the Tauri Rust shell and invoke handlers unchanged unless the Svelte
    frontend exposes a contract mismatch.
- `src/backend_contract.rs`
  - Keep backend-owned command discovery, filtering, command IDs, and execution
    result DTOs as the shared frontend contract.

### Do Not Touch For This Swap

- `src/ui/**`
  - Existing egui UI remains in place.
- `src/platform/windows/**`
  - Windows hotkey, context, and sender behavior remain backend work for later
    Tauri phases.
- `extensions/**`
  - Extension packages and WASM plugins are not part of this framework swap.

## Phases

### Phase 0: Documentation And Priority Reset

Purpose:

- Create this Svelte-first plan beside the old Tauri migration plan.
- Mark the old React-based Tauri plan as paused.
- Update agent-facing instructions so future work prioritizes this file.

Acceptance criteria:

- `docs/migration/svelte-tauri-migration-plan.md` exists.
- `docs/migration/tauri-migration-plan.md` points to this plan before any old
  React guidance.
- `AGENTS.md` names this Svelte plan as the active Tauri migration authority.

### Phase 1: Svelte Tooling Swap

Purpose:

- Replace the React frontend toolchain with the Svelte frontend toolchain while
  preserving the current Tauri shell.

Scope:

- Remove React, React DOM, React type packages, and the React Vite plugin.
- Add Svelte and the Svelte Vite plugin.
- Update Vite, TypeScript, Tailwind content globs, and the HTML entrypoint for
  Svelte files.
- Keep Bun scripts equivalent: `dev`, `build`, `preview`, `test`, and `tauri`.

Acceptance criteria:

- `bun install` updates the frontend lockfile.
- `bun run build` compiles the Svelte frontend.
- Tauri config still points at the same dev URL and frontend dist.

### Phase 2: Port The Current React Wireframe To Svelte

Purpose:

- Recreate the existing Phase 3 command bridge UI in Svelte before adding new
  behavior.

Scope:

- Port the current palette shell, query input, command rows, selected row state,
  health strip, settings placeholder, loading state, command error state, and
  execution result state.
- Keep `commands.ts` as the frontend API boundary unless TypeScript/Svelte
  integration requires a small type-only adjustment.
- Preserve the existing Tauri invoke command names and payload shapes.

Acceptance criteria:

- The Svelte UI renders the same wireframe states currently covered by React.
- Search calls `search_commands` with the current query.
- Run selected calls `execute_command` with the selected command ID.
- Health still calls `health_check`.

### Phase 3: Svelte Parity Verification

Purpose:

- Prove the Svelte port has reached the current React Phase 3 behavior before
  broader migration work resumes.

Scope:

- Keep or adapt frontend tests for the command API wrapper and selection helper.
- Run frontend build and tests.
- Run relevant Rust tests for the Tauri shell and backend contract.
- Manually launch the Tauri app when UI verification is needed.

Acceptance criteria:

- `bun run test` passes in `apps/desktop-tauri`.
- `bun run build` passes in `apps/desktop-tauri`.
- Relevant Rust tests pass.
- No React-specific runtime dependency remains after the Svelte port.

### Phase 4: Resume Tauri Migration Through Svelte

Purpose:

- Continue the egui-to-Tauri migration only after the Svelte app reaches the
  current React Phase 3 bridge behavior.

Scope:

- Runtime integration, palette parity, settings, diagnostics, packaging, and
  eventual egui removal should be rewritten as Svelte/Tauri phases.
- The old React plan can be mined for backend sequencing, but React component
  guidance should not be followed.

Acceptance criteria:

- Future migration plans and implementation tasks name Svelte as the frontend.
- No new React UI migration work is added.
- egui removal remains deferred until Svelte/Tauri parity is proven.

## Non-Goals For The Svelte Swap

- Do not rewrite the Rust backend contract unless Svelte exposes a concrete
  mismatch.
- Do not redesign the command palette UI beyond matching the current wireframe.
- Do not migrate settings, global hotkeys, tray behavior, guide mode, or debug
  overlay until Svelte parity with the current React bridge is verified.
- Do not remove egui/eframe during the frontend framework swap.

## Verification Checklist

Use this checklist before declaring any Svelte migration phase complete:

- This plan is still the first migration document consulted.
- The old React-based Tauri plan remains marked as paused.
- The Tauri invoke command names are unchanged unless a later plan explicitly
  changes them.
- The Svelte frontend builds with Bun.
- Frontend tests cover the command API wrapper and selection behavior.
- The existing egui app remains runnable until final Tauri cutover.
