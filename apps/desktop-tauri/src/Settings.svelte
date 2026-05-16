<script lang="ts">
  import { onMount } from "svelte";

  import type { ActivationShortcut, RuntimeSettings } from "./commands";
  import {
    activationShortcutFromKeyboardEvent,
    applyRuntimeSettingsSaveResult,
    discardRuntimeSettingsDraft,
    formatActivationShortcut,
    paletteApi,
    runtimeSettingsAreDirty,
    runtimeSettingsSaveRequestFromDraft,
  } from "./commands";

  let settingsSaved: RuntimeSettings | null = null;
  let settingsDraft: RuntimeSettings | null = null;
  let defaultActivationShortcut: ActivationShortcut | null = null;
  let settingsConfigPath: string | null = null;
  let settingsConfigError: string | null = null;
  let settingsLoading = true;
  let settingsSaving = false;
  let settingsReloading = false;
  let recordingActivationShortcut = false;
  let settingsMessage: string | null = null;
  let settingsFailed = false;

  $: settingsDirty = runtimeSettingsAreDirty(settingsSaved, settingsDraft);

  onMount(() => {
    loadSettingsBootstrap();
  });

  function loadSettingsBootstrap() {
    settingsLoading = true;
    paletteApi
      .getSettingsBootstrap()
      .then((bootstrap) => {
        settingsSaved = discardRuntimeSettingsDraft(bootstrap.config);
        settingsDraft = discardRuntimeSettingsDraft(bootstrap.config);
        defaultActivationShortcut = { ...bootstrap.default_activation_shortcut };
        settingsConfigPath = bootstrap.config_path;
        settingsConfigError = bootstrap.config_error;
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

  function recordActivationShortcut() {
    recordingActivationShortcut = true;
    settingsMessage = "Press the new activation shortcut";
    settingsFailed = false;
  }

  function resetActivationShortcut() {
    if (!defaultActivationShortcut) {
      return;
    }

    updateActivationShortcut(defaultActivationShortcut);
    recordingActivationShortcut = false;
    settingsMessage = `Reset to ${formatActivationShortcut(defaultActivationShortcut)}`;
    settingsFailed = false;
  }

  function updateActivationShortcut(shortcut: ActivationShortcut) {
    const nextShortcut = {
      ...shortcut,
      display_text: formatActivationShortcut(shortcut),
    };
    updateSettingsDraft((draft) => {
      draft.activation_shortcut = nextShortcut;
      draft.activation_hint = nextShortcut.display_text;
    });
  }

  function handleActivationShortcutKeydown(event: KeyboardEvent) {
    if (!recordingActivationShortcut) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const shortcut = activationShortcutFromKeyboardEvent(event);
    if (!shortcut) {
      settingsMessage = "Press a supported non-modifier shortcut key";
      settingsFailed = false;
      return;
    }

    updateActivationShortcut(shortcut);
    recordingActivationShortcut = false;
    settingsMessage = `Recorded ${formatActivationShortcut(shortcut)}`;
    settingsFailed = false;
  }

  function saveRuntimeSettings() {
    if (!settingsSaved || !settingsDraft || settingsSaving || !settingsDirty) {
      return;
    }

    settingsSaving = true;
    paletteApi
      .saveRuntimeSettings(runtimeSettingsSaveRequestFromDraft(settingsDraft))
      .then(async (result) => {
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
</script>

<svelte:window onkeydown={handleActivationShortcutKeydown} />

<main class="min-h-screen bg-zinc-950 p-6 text-zinc-100">
  <section class="mx-auto max-w-4xl rounded-lg border border-zinc-700 bg-zinc-900">
    <div class="border-b border-zinc-700 p-4">
      <div class="flex items-start justify-between gap-4">
        <div>
          <h1 class="text-xl font-semibold">Settings</h1>
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
          <div class="flex flex-wrap items-center gap-2">
            <span class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-300">
              {formatActivationShortcut(settingsDraft.activation_shortcut)}
            </span>
            <button
              class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100 disabled:text-zinc-600"
              disabled={recordingActivationShortcut}
              onclick={recordActivationShortcut}
              type="button"
            >
              {recordingActivationShortcut ? "Recording..." : "Record"}
            </button>
            <button
              class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100 disabled:text-zinc-600"
              disabled={!defaultActivationShortcut}
              onclick={resetActivationShortcut}
              type="button"
            >
              Reset
            </button>
          </div>
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
                  onchange={() => updateAppearanceTheme(theme as RuntimeSettings["appearance_theme"])}
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
</main>
