<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  import type { CommandExecutionResult, CommandRow, RuntimeStatus } from "./commands";
  import { formatRuntimeStatus, nextSelectedCommandId, paletteApi } from "./commands";

  type HealthPayload = {
    app_name: string;
    phase: string;
    status: string;
  };

  let query = "";
  let selectedId = "";
  let rows: CommandRow[] = [];
  let activeView: "palette" | "settings" = "palette";
  let health: HealthPayload | null = null;
  let healthError: string | null = null;
  let runtimeStatus: RuntimeStatus | null = null;
  let commandError: string | null = null;
  let loadingCommands = true;
  let executionResult: CommandExecutionResult | null = null;
  let searchInput: HTMLInputElement | null = null;
  let searchRun = 0;

  $: searchCommands(query);

  onMount(() => {
    searchInput?.focus();

    invoke<HealthPayload>("health_check")
      .then((payload) => {
        health = payload;
        healthError = null;
      })
      .catch((error: unknown) => {
        health = null;
        healthError = errorMessage(error);
      });

    paletteApi
      .getPaletteBootstrap()
      .then((bootstrap) => {
        runtimeStatus = bootstrap.runtime_status;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });
  });

  function searchCommands(currentQuery: string) {
    const run = ++searchRun;
    loadingCommands = true;

    paletteApi
      .searchCommands(currentQuery)
      .then((snapshot) => {
        if (run !== searchRun) {
          return;
        }

        rows = snapshot.commands;
        selectedId = nextSelectedCommandId(selectedId, rows);
        commandError = null;
      })
      .catch((error: unknown) => {
        if (run !== searchRun) {
          return;
        }

        rows = [];
        selectedId = "";
        commandError = errorMessage(error);
      })
      .finally(() => {
        if (run === searchRun) {
          loadingCommands = false;
        }
      });
  }

  function runSelectedCommand() {
    if (!selectedId) {
      return;
    }

    paletteApi
      .executeCommand(selectedId)
      .then((result) => {
        executionResult = result;
      })
      .catch((error: unknown) => {
        executionResult = {
          status: "failed",
          message: errorMessage(error),
        };
      });
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function viewButtonClass(active: boolean): string {
    return [
      "rounded px-3 py-1",
      active ? "bg-amber-600 text-white" : "text-zinc-300 hover:bg-zinc-800",
    ].join(" ");
  }
</script>

<main class="min-h-screen bg-zinc-950 p-6 text-zinc-100">
  <section class="mx-auto max-w-4xl">
    <header class="mb-4 flex items-center justify-between gap-4">
      <div>
        <h1 class="text-xl font-semibold">Omni Palette</h1>
        <p class="text-sm text-zinc-400">Phase 3 backend command bridge</p>
      </div>
      <div class="flex rounded-md border border-zinc-700 p-1 text-sm">
        <button
          class={viewButtonClass(activeView === "palette")}
          onclick={() => (activeView = "palette")}
          type="button"
        >
          Palette
        </button>
        <button
          class={viewButtonClass(activeView === "settings")}
          onclick={() => (activeView = "settings")}
          type="button"
        >
          Settings
        </button>
      </div>
    </header>

    <div class="mb-4 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-300">
      {#if health}
        <span>Backend: {health.status} - {health.app_name} - {health.phase}</span>
        {#if runtimeStatus}
          <span class="ml-2 text-zinc-400">- {formatRuntimeStatus(runtimeStatus)}</span>
        {/if}
      {:else}
        <span>Backend: {healthError ? `unavailable (${healthError})` : "checking..."}</span>
      {/if}
    </div>

    {#if activeView === "palette"}
      <section class="rounded-lg border border-zinc-700 bg-zinc-900">
        <div class="border-b border-zinc-700 p-4">
          <label class="sr-only" for="command-search">Search commands</label>
          <input
            bind:this={searchInput}
            bind:value={query}
            class="w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 py-2 text-base text-zinc-100 outline-none focus:border-amber-500"
            id="command-search"
            placeholder="Type a command"
          />
        </div>

        <div
          class="flex items-center justify-between border-b border-zinc-700 px-4 py-2 text-xs text-zinc-400"
        >
          <span>{loadingCommands ? "Loading commands..." : `${rows.length} commands`}</span>
          <button
            class="rounded border border-zinc-700 px-3 py-1 text-zinc-100 disabled:text-zinc-600"
            disabled={!selectedId}
            onclick={runSelectedCommand}
            type="button"
          >
            Run selected
          </button>
        </div>

        {#if commandError}
          <div class="border-b border-zinc-700 px-4 py-2 text-sm text-red-300">
            {commandError}
          </div>
        {/if}

        {#if executionResult}
          <div class="border-b border-zinc-700 px-4 py-2 text-sm text-zinc-300">
            {executionResult.status}: {executionResult.message}
          </div>
        {/if}

        <div class="max-h-[420px] overflow-y-auto p-2">
          {#if rows.length === 0}
            <div class="rounded-md border border-dashed border-zinc-700 p-8 text-center text-sm text-zinc-400">
              {loadingCommands ? "Loading commands..." : "No matching commands"}
            </div>
          {:else}
            {#each rows as command (command.id)}
              {@const selected = command.id === selectedId}
              <button
                class={[
                  "flex w-full items-center justify-between rounded-md px-3 py-3 text-left",
                  selected ? "border border-amber-500 bg-zinc-800" : "border border-transparent",
                ].join(" ")}
                onclick={() => (selectedId = command.id)}
                type="button"
              >
                <span>
                  <span class="block text-sm font-medium">{command.label}</span>
                  <span class="block text-xs text-zinc-400">
                    {command.focus_state} - {command.priority}
                  </span>
                </span>
                <span class="text-xs text-zinc-400">
                  {command.shortcut_text || "backend"}
                </span>
              </button>
            {/each}
          {/if}
        </div>
      </section>
    {:else}
      <section class="rounded-lg border border-zinc-700 bg-zinc-900 p-6">
        <h2 class="text-lg font-semibold">Settings</h2>
        <p class="mt-2 text-sm text-zinc-400">
          Placeholder view. Runtime settings and extension management stay in the egui app until a
          later migration phase.
        </p>
      </section>
    {/if}
  </section>
</main>
