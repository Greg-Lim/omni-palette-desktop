<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  import type {
    CommandExecutionResult,
    CommandRow,
    GuideEventPayload,
    GuideStatus,
    HotkeyEventPayload,
    HotkeyStatus,
    RuntimeStatus,
    WindowLifecycleEventPayload,
    WindowLifecycleStatus,
  } from "./commands";
  import {
    GUIDE_EVENT_NAME,
    HOTKEY_EVENT_NAME,
    WINDOW_LIFECYCLE_EVENT_NAME,
    commandExecutionShouldHidePalette,
    formatGuideStatus,
    formatHotkeyStatus,
    formatRuntimeStatus,
    formatWindowLifecycleStatus,
    highlightedLabelSegments,
    nextKeyboardSelectedCommandId,
    nextGuideStatus,
    nextSelectedCommandId,
    nextWindowLifecycleStatus,
    paletteKeyAction,
    paletteApi,
    shouldStartGuideForCommand,
    shouldHidePaletteForWindowBlur,
    shouldRefreshCommandsForWindowLifecycleEvent,
  } from "./commands";

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
  let hotkeyStatus: HotkeyStatus | null = null;
  let windowLifecycleStatus: WindowLifecycleStatus | null = null;
  let guideStatus: GuideStatus | null = null;
  let commandError: string | null = null;
  let loadingCommands = true;
  let executionResult: CommandExecutionResult | null = null;
  let searchInput: HTMLInputElement | null = null;
  let searchRun = 0;
  let hidingPalette = false;

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

    paletteApi
      .getHotkeyStatus()
      .then((status) => {
        hotkeyStatus = status;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    paletteApi
      .getWindowLifecycleStatus()
      .then((status) => {
        windowLifecycleStatus = status;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    paletteApi
      .getGuideStatus()
      .then((status) => {
        guideStatus = status;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    let unlistenHotkeyEvents: (() => void) | null = null;
    listen<HotkeyEventPayload>(HOTKEY_EVENT_NAME, (event) => {
      hotkeyStatus = nextHotkeyStatus(hotkeyStatus, event.payload);
    })
      .then((unlisten) => {
        unlistenHotkeyEvents = unlisten;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    let unlistenWindowLifecycleEvents: (() => void) | null = null;
    listen<WindowLifecycleEventPayload>(WINDOW_LIFECYCLE_EVENT_NAME, (event) => {
      windowLifecycleStatus = nextWindowLifecycleStatus(windowLifecycleStatus, event.payload);
      if (shouldRefreshCommandsForWindowLifecycleEvent(event.payload)) {
        activeView = "palette";
        query = "";
        selectedId = "";
        executionResult = null;
        searchCommands("");
        tick().then(() => searchInput?.focus());
      }
    })
      .then((unlisten) => {
        unlistenWindowLifecycleEvents = unlisten;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    let unlistenGuideEvents: (() => void) | null = null;
    listen<GuideEventPayload>(GUIDE_EVENT_NAME, (event) => {
      guideStatus = nextGuideStatus(guideStatus, event.payload);
    })
      .then((unlisten) => {
        unlistenGuideEvents = unlisten;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      });

    window.addEventListener("blur", handleWindowBlur);

    return () => {
      unlistenHotkeyEvents?.();
      unlistenWindowLifecycleEvents?.();
      unlistenGuideEvents?.();
      window.removeEventListener("blur", handleWindowBlur);
    };
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

  function runCommand(commandId: string) {
    if (!commandId) {
      return;
    }

    selectedId = commandId;
    const command = rows.find((row) => row.id === commandId);
    if (shouldStartGuideForCommand(runtimeStatus, command)) {
      executionResult = null;
      paletteApi
        .startGuide(commandId)
        .then((status) => {
          guideStatus = status;
        })
        .catch((error: unknown) => {
          executionResult = {
            status: "failed",
            message: errorMessage(error),
          };
        });
      return;
    }

    paletteApi
      .executeCommand(commandId)
      .then((result) => {
        executionResult = result;
        if (commandExecutionShouldHidePalette(result)) {
          hidePaletteWindow();
        }
      })
      .catch((error: unknown) => {
        executionResult = {
          status: "failed",
          message: errorMessage(error),
        };
      });
  }

  function handlePaletteKeydown(event: KeyboardEvent) {
    const action = paletteKeyAction(event.key);
    if (!action || activeView !== "palette") {
      return;
    }

    event.preventDefault();
    if (action === "select_next") {
      selectedId = nextKeyboardSelectedCommandId(selectedId, rows, 1);
    } else if (action === "select_previous") {
      selectedId = nextKeyboardSelectedCommandId(selectedId, rows, -1);
    } else if (action === "execute") {
      runCommand(selectedId);
    } else {
      hidePaletteWindow();
    }
  }

  function handleWindowBlur() {
    if (shouldHidePaletteForWindowBlur(windowLifecycleStatus)) {
      hidePaletteWindow();
    }
  }

  function hidePaletteWindow() {
    if (hidingPalette) {
      return;
    }

    hidingPalette = true;
    paletteApi
      .hidePaletteWindow()
      .then((status) => {
        windowLifecycleStatus = status;
      })
      .catch((error: unknown) => {
        healthError = errorMessage(error);
      })
      .finally(() => {
        hidingPalette = false;
      });
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function nextHotkeyStatus(
    current: HotkeyStatus | null,
    event: HotkeyEventPayload,
  ): HotkeyStatus {
    return {
      running: event.kind !== "listener_error" && (current?.running ?? true),
      activation_hint: current?.activation_hint ?? event.shortcut,
      activation_count: event.activation_count,
      ignored_passthrough_count: event.ignored_passthrough_count,
      last_event: event,
      last_error: event.kind === "listener_error" ? event.message : null,
    };
  }

  function viewButtonClass(active: boolean): string {
    return [
      "rounded px-3 py-1",
      active ? "bg-amber-600 text-white" : "text-zinc-300 hover:bg-zinc-800",
    ].join(" ");
  }
</script>

<svelte:window onkeydown={handlePaletteKeydown} />

<main class="min-h-screen bg-zinc-950 p-6 text-zinc-100">
  <section class="mx-auto max-w-4xl">
    <header class="mb-4 flex items-center justify-between gap-4">
      <div>
        <h1 class="text-xl font-semibold">Omni Palette</h1>
        <p class="text-sm text-zinc-400">Phase 5B guide mode and refined palette positioning</p>
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
        {#if hotkeyStatus}
          <span class="ml-2 text-zinc-400">- {formatHotkeyStatus(hotkeyStatus)}</span>
        {/if}
        {#if windowLifecycleStatus}
          <span class="ml-2 text-zinc-400">- {formatWindowLifecycleStatus(windowLifecycleStatus)}</span>
        {/if}
        {#if guideStatus}
          <span class="ml-2 text-zinc-400">- {formatGuideStatus(guideStatus)}</span>
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
            onclick={() => runCommand(selectedId)}
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
                onclick={() => runCommand(command.id)}
                type="button"
              >
                <span>
                  <span class="block text-sm font-medium">
                    {#each highlightedLabelSegments(command.label, command.label_matches) as segment}
                      <span class={segment.highlighted ? "text-amber-300" : ""}>{segment.text}</span>
                    {/each}
                  </span>
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
