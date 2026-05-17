<script lang="ts">
  import { onMount } from "svelte";

  import type { DebugOverlayStatus, DebugSnapshot } from "./commands";
  import { formatDebugOverlayStatus, paletteApi } from "./commands";

  let status: DebugOverlayStatus | null = null;
  let snapshot: DebugSnapshot | null = null;
  let message: string | null = null;
  let failed = false;
  let loading = true;

  onMount(() => {
    refreshDebugSnapshot();
    const interval = window.setInterval(refreshDebugSnapshot, 1000);

    return () => window.clearInterval(interval);
  });

  function refreshDebugSnapshot() {
    loading = true;
    Promise.all([paletteApi.getDebugOverlayStatus(), paletteApi.getDebugSnapshot()])
      .then(([nextStatus, nextSnapshot]) => {
        status = nextStatus;
        snapshot = nextSnapshot;
        message = null;
        failed = false;
      })
      .catch((error: unknown) => {
        message = errorMessage(error);
        failed = true;
      })
      .finally(() => {
        loading = false;
      });
  }

  function closeDebugOverlay() {
    paletteApi
      .closeDebugOverlay()
      .then((nextStatus) => {
        status = nextStatus;
        message = nextStatus.message;
        failed = nextStatus.status === "failed";
      })
      .catch((error: unknown) => {
        message = errorMessage(error);
        failed = true;
      });
  }

  function windowLabel(window: { process_name: string | null; hwnd: number | null } | null) {
    if (!window) {
      return "None";
    }
    return `${window.process_name ?? "Unknown process"} (${window.hwnd ?? "no hwnd"})`;
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<main class="min-h-screen bg-zinc-950 p-4 text-zinc-100">
  <header class="flex items-start justify-between gap-4">
    <div>
      <h1 class="text-lg font-semibold">Debug Overlay</h1>
      {#if status}
        <p class="mt-1 text-sm text-zinc-400">{formatDebugOverlayStatus(status)}</p>
      {/if}
    </div>
    <div class="flex gap-2">
      <button
        class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100"
        onclick={refreshDebugSnapshot}
        type="button"
      >
        {loading ? "Refreshing..." : "Refresh"}
      </button>
      <button
        class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100"
        onclick={closeDebugOverlay}
        type="button"
      >
        Close
      </button>
    </div>
  </header>

  {#if message}
    <p
      class={[
        "mt-4 rounded border px-3 py-2 text-sm",
        failed
          ? "border-red-800 bg-red-950 text-red-200"
          : "border-emerald-800 bg-emerald-950 text-emerald-200",
      ].join(" ")}
    >
      {message}
    </p>
  {/if}

  {#if snapshot}
    <div class="mt-4 grid gap-4">
      <section class="rounded border border-zinc-800 bg-zinc-900 p-3">
        <h2 class="font-medium">Foreground</h2>
        <p class="mt-2 text-sm text-zinc-300">{windowLabel(snapshot.foreground_window)}</p>
        <p class="mt-1 text-sm text-zinc-400">
          Ignored: {snapshot.ignored_process_name ?? "No"}
        </p>
      </section>

      <section class="rounded border border-zinc-800 bg-zinc-900 p-3">
        <h2 class="font-medium">Interaction</h2>
        <p class="mt-2 text-sm text-zinc-300">
          Text input: {snapshot.text_input_active ? "active" : "inactive"}
        </p>
        <p class="mt-1 text-sm text-zinc-400">
          Tags: {snapshot.active_tags.length > 0 ? snapshot.active_tags.join(", ") : "None"}
        </p>
      </section>

      <section class="rounded border border-zinc-800 bg-zinc-900 p-3">
        <h2 class="font-medium">Command Candidates</h2>
        <div class="mt-2 grid grid-cols-2 gap-2 text-sm text-zinc-300">
          <span>Total: {snapshot.command_summary.total}</span>
          <span>Focused: {snapshot.command_summary.focused}</span>
          <span>Background: {snapshot.command_summary.background}</span>
          <span>Global: {snapshot.command_summary.global}</span>
          <span>Favorites: {snapshot.command_summary.favorites}</span>
          <span>High: {snapshot.command_summary.high_priority}</span>
          <span>Medium: {snapshot.command_summary.medium_priority}</span>
          <span>Low: {snapshot.command_summary.low_priority}</span>
          <span>Suppressed: {snapshot.command_summary.suppressed_priority}</span>
        </div>
      </section>

      <section class="rounded border border-zinc-800 bg-zinc-900 p-3">
        <h2 class="font-medium">Palette Filter</h2>
        <p class="mt-2 text-sm text-zinc-300">
          Query: {snapshot.palette_state.query || "(empty)"}
        </p>
        <p class="mt-1 text-sm text-zinc-400">
          Filtered rows: {snapshot.palette_state.filtered_count}
        </p>
        {#if snapshot.palette_state.top_rows.length === 0}
          <p class="mt-3 text-sm text-zinc-500">No palette rows recorded yet.</p>
        {:else}
          <div class="mt-3 grid gap-2">
            {#each snapshot.palette_state.top_rows as row}
              <article class="rounded border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm">
                <div class="font-medium">{row.label}</div>
                <div class="mt-1 text-xs text-zinc-400">
                  {row.focus_state} - {row.priority} - score {row.score}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </section>

      <section class="rounded border border-zinc-800 bg-zinc-900 p-3">
        <h2 class="font-medium">Background Windows</h2>
        <p class="mt-2 text-sm text-zinc-400">
          Showing {snapshot.background_windows.length} of {snapshot.background_total}
        </p>
        {#if snapshot.background_windows.length === 0}
          <p class="mt-3 text-sm text-zinc-500">No background windows found.</p>
        {:else}
          <div class="mt-3 grid gap-2">
            {#each snapshot.background_windows as window}
              <div class="rounded border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm">
                {windowLabel(window)}
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {:else}
    <p class="mt-4 text-sm text-zinc-400">Loading debug snapshot...</p>
  {/if}
</main>
