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
    RuntimeSettings,
    RuntimeStatus,
    WindowLifecycleEventPayload,
    WindowLifecycleStatus,
  } from "./commands";
  import {
    GUIDE_EVENT_NAME,
    HOTKEY_EVENT_NAME,
    WINDOW_LIFECYCLE_EVENT_NAME,
    applyRuntimeSettingsSaveResult,
    commandExecutionShouldHidePalette,
    discardRuntimeSettingsDraft,
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
    runtimeSettingsAreDirty,
    runtimeSettingsSaveRequestFromDraft,
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
  let settingsSaved: RuntimeSettings | null = null;
  let settingsDraft: RuntimeSettings | null = null;
  let settingsConfigPath: string | null = null;
  let settingsConfigError: string | null = null;
  let settingsLoading = true;
  let settingsSaving = false;
  let settingsReloading = false;
  let settingsMessage: string | null = null;
  let settingsFailed = false;

  $: searchCommands(query);
  $: settingsDirty = runtimeSettingsAreDirty(settingsSaved, settingsDraft);

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

    loadSettingsBootstrap();

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

  function loadSettingsBootstrap() {
    settingsLoading = true;
    paletteApi
      .getSettingsBootstrap()
      .then((bootstrap) => {
        settingsSaved = discardRuntimeSettingsDraft(bootstrap.config);
        settingsDraft = discardRuntimeSettingsDraft(bootstrap.config);
        settingsConfigPath = bootstrap.config_path;
        settingsConfigError = bootstrap.config_error;
        runtimeStatus = bootstrap.runtime_status;
        settingsMessage = null;
        settingsFailed = false;
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        settingsLoading = false;
      });
  }

  function updateSettingsDraft(update: (draft: RuntimeSettings) => void) {
    if (!settingsDraft) {
      return;
    }

    const next = discardRuntimeSettingsDraft(settingsDraft);
    update(next);
    settingsDraft = next;
  }

  function updateCommandBehavior(value: RuntimeSettings["command_behavior"]) {
    updateSettingsDraft((draft) => {
      draft.command_behavior = value;
    });
  }

  function updateAppearanceTheme(value: RuntimeSettings["appearance_theme"]) {
    updateSettingsDraft((draft) => {
      draft.appearance_theme = value;
    });
  }

  function updateCatalogEnabled(value: boolean) {
    updateSettingsDraft((draft) => {
      draft.github.enabled = value;
    });
  }

  function updateCatalogText(
    field: "owner" | "repo" | "branch" | "catalog_path",
    value: string,
  ) {
    updateSettingsDraft((draft) => {
      draft.github[field] = value;
    });
  }

  function saveRuntimeSettings() {
    if (!settingsSaved || !settingsDraft || settingsSaving || !settingsDirty) {
      return;
    }

    settingsSaving = true;
    paletteApi
      .saveRuntimeSettings(runtimeSettingsSaveRequestFromDraft(settingsDraft))
      .then(async (result) => {
        runtimeStatus = result.runtime_status;
        settingsConfigPath = result.runtime_status.config_path;
        settingsConfigError = result.runtime_status.config_error;

        if (!settingsSaved || !settingsDraft) {
          return;
        }

        const applied = applyRuntimeSettingsSaveResult(settingsSaved, settingsDraft, result);
        settingsSaved = applied.saved;
        settingsDraft = applied.draft;
        settingsMessage = applied.message;
        settingsFailed = applied.failed;

        if (!applied.failed) {
          await reloadRuntimeStateAfterSave();
        }
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        settingsSaving = false;
      });
  }

  async function reloadRuntimeStateAfterSave() {
    try {
      const result = await paletteApi.reloadRuntimeState();
      runtimeStatus = result.runtime_status;
      if (result.status === "failed") {
        settingsMessage = `Settings saved; reload failed: ${result.message}`;
        settingsFailed = true;
      }
    } catch (error: unknown) {
      settingsMessage = `Settings saved; reload failed: ${errorMessage(error)}`;
      settingsFailed = true;
    }
  }

  function discardSettingsChanges() {
    if (!settingsSaved) {
      return;
    }

    settingsDraft = discardRuntimeSettingsDraft(settingsSaved);
    settingsMessage = "Changes discarded";
    settingsFailed = false;
  }

  function reloadRuntimeState() {
    if (settingsReloading) {
      return;
    }

    settingsReloading = true;
    paletteApi
      .reloadRuntimeState()
      .then((result) => {
        runtimeStatus = result.runtime_status;
        settingsMessage = result.message;
        settingsFailed = result.status === "failed";
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        settingsReloading = false;
      });
  }

  function inputValue(event: Event): string {
    return (event.currentTarget as HTMLInputElement).value;
  }

  function checkedValue(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
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
        <p class="text-sm text-zinc-400">Phase 6A runtime settings foundation</p>
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
      <section class="rounded-lg border border-zinc-700 bg-zinc-900">
        <div class="border-b border-zinc-700 p-4">
          <div class="flex items-start justify-between gap-4">
            <div>
              <h2 class="text-lg font-semibold">Settings</h2>
              <p class="mt-1 text-sm text-zinc-400">
                {settingsConfigPath ?? "Config path unavailable"}
              </p>
            </div>
            <button
              class="rounded border border-zinc-700 px-3 py-1 text-sm text-zinc-100 disabled:text-zinc-600"
              disabled={settingsReloading}
              onclick={reloadRuntimeState}
              type="button"
            >
              {settingsReloading ? "Reloading..." : "Reload extensions"}
            </button>
          </div>
          {#if settingsConfigError}
            <p class="mt-3 rounded border border-red-800 bg-red-950 px-3 py-2 text-sm text-red-200">
              {settingsConfigError}
            </p>
          {/if}
          {#if settingsMessage}
            <p
              class={[
                "mt-3 rounded border px-3 py-2 text-sm",
                settingsFailed
                  ? "border-red-800 bg-red-950 text-red-200"
                  : "border-emerald-800 bg-emerald-950 text-emerald-200",
              ].join(" ")}
            >
              {settingsMessage}
            </p>
          {/if}
        </div>

        {#if settingsLoading || !settingsDraft}
          <div class="p-6 text-sm text-zinc-400">Loading settings...</div>
        {:else}
          <div class="space-y-6 p-4">
            <div class="grid gap-2">
              <span class="text-sm font-medium text-zinc-200">Activation shortcut</span>
              <span class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-300">
                {settingsDraft.activation_hint}
              </span>
            </div>

            <fieldset class="grid gap-3">
              <legend class="text-sm font-medium text-zinc-200">Command behavior</legend>
              <div class="flex flex-wrap gap-2">
                <label
                  class={[
                    "rounded border px-3 py-2 text-sm",
                    settingsDraft.command_behavior === "execute"
                      ? "border-amber-500 bg-zinc-800 text-zinc-100"
                      : "border-zinc-700 text-zinc-300",
                  ].join(" ")}
                >
                  <input
                    checked={settingsDraft.command_behavior === "execute"}
                    class="sr-only"
                    name="command-behavior"
                    onchange={() => updateCommandBehavior("execute")}
                    type="radio"
                  />
                  Execute
                </label>
                <label
                  class={[
                    "rounded border px-3 py-2 text-sm",
                    settingsDraft.command_behavior === "guide"
                      ? "border-amber-500 bg-zinc-800 text-zinc-100"
                      : "border-zinc-700 text-zinc-300",
                  ].join(" ")}
                >
                  <input
                    checked={settingsDraft.command_behavior === "guide"}
                    class="sr-only"
                    name="command-behavior"
                    onchange={() => updateCommandBehavior("guide")}
                    type="radio"
                  />
                  Guide
                </label>
              </div>
            </fieldset>

            <fieldset class="grid gap-3">
              <legend class="text-sm font-medium text-zinc-200">Theme</legend>
              <div class="flex flex-wrap gap-2">
                {#each ["system", "light", "dark"] as theme}
                  <label
                    class={[
                      "rounded border px-3 py-2 text-sm capitalize",
                      settingsDraft.appearance_theme === theme
                        ? "border-amber-500 bg-zinc-800 text-zinc-100"
                        : "border-zinc-700 text-zinc-300",
                    ].join(" ")}
                  >
                    <input
                      checked={settingsDraft.appearance_theme === theme}
                      class="sr-only"
                      name="appearance-theme"
                      onchange={() =>
                        updateAppearanceTheme(theme as RuntimeSettings["appearance_theme"])}
                      type="radio"
                    />
                    {theme}
                  </label>
                {/each}
              </div>
            </fieldset>

            <fieldset class="grid gap-3">
              <legend class="text-sm font-medium text-zinc-200">Catalog source</legend>
              <label class="flex items-center gap-3 text-sm text-zinc-300">
                <input
                  checked={settingsDraft.github.enabled}
                  class="h-4 w-4 rounded border-zinc-700 bg-zinc-950"
                  onchange={(event) => updateCatalogEnabled(checkedValue(event))}
                  type="checkbox"
                />
                Enable GitHub catalog source
              </label>
              <div class="grid gap-3 sm:grid-cols-2">
                <label class="grid gap-1 text-sm text-zinc-300">
                  Owner
                  <input
                    class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-amber-500"
                    value={settingsDraft.github.owner}
                    oninput={(event) => updateCatalogText("owner", inputValue(event))}
                  />
                </label>
                <label class="grid gap-1 text-sm text-zinc-300">
                  Repo
                  <input
                    class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-amber-500"
                    value={settingsDraft.github.repo}
                    oninput={(event) => updateCatalogText("repo", inputValue(event))}
                  />
                </label>
                <label class="grid gap-1 text-sm text-zinc-300">
                  Branch
                  <input
                    class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-amber-500"
                    value={settingsDraft.github.branch}
                    oninput={(event) => updateCatalogText("branch", inputValue(event))}
                  />
                </label>
                <label class="grid gap-1 text-sm text-zinc-300">
                  Catalog path
                  <input
                    class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100 outline-none focus:border-amber-500"
                    value={settingsDraft.github.catalog_path}
                    oninput={(event) => updateCatalogText("catalog_path", inputValue(event))}
                  />
                </label>
              </div>
            </fieldset>
          </div>

          <div class="flex items-center justify-between gap-3 border-t border-zinc-700 p-4">
            <span class="text-sm text-zinc-400">
              {settingsDirty ? "Unsaved changes" : "Settings are current"}
            </span>
            <div class="flex gap-2">
              <button
                class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100 disabled:text-zinc-600"
                disabled={!settingsDirty || settingsSaving}
                onclick={discardSettingsChanges}
                type="button"
              >
                Discard
              </button>
              <button
                class="rounded bg-amber-600 px-3 py-2 text-sm font-medium text-white disabled:bg-zinc-700 disabled:text-zinc-400"
                disabled={!settingsDirty || settingsSaving}
                onclick={saveRuntimeSettings}
                type="button"
              >
                {settingsSaving ? "Saving..." : "Save settings"}
              </button>
            </div>
          </div>
        {/if}
      </section>
    {/if}
  </section>
</main>
