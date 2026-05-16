<script lang="ts">
  import { onMount } from "svelte";

  import type {
    ActivationShortcut,
    ExtensionRow,
    ExtensionsBootstrap,
    RuntimeSettings,
  } from "./commands";
  import {
    activationShortcutFromKeyboardEvent,
    applyExtensionMutationResult,
    applyRuntimeSettingsSaveResult,
    discardRuntimeSettingsDraft,
    formatActivationShortcut,
    paletteApi,
    runtimeSettingsAreDirty,
    runtimeSettingsSaveRequestFromDraft,
  } from "./commands";

  type SettingsPage = "general" | "extensions" | "marketplace";

  let activeSettingsPage: SettingsPage = "general";
  let settingsSaved: RuntimeSettings | null = null;
  let settingsDraft: RuntimeSettings | null = null;
  let defaultActivationShortcut: ActivationShortcut | null = null;
  let extensionsBootstrap: ExtensionsBootstrap | null = null;
  let settingsConfigPath: string | null = null;
  let settingsConfigError: string | null = null;
  let settingsLoading = true;
  let extensionsLoading = true;
  let settingsSaving = false;
  let settingsReloading = false;
  let recordingActivationShortcut = false;
  let extensionMutationKey: string | null = null;
  let settingsMessage: string | null = null;
  let settingsFailed = false;

  $: settingsDirty = runtimeSettingsAreDirty(settingsSaved, settingsDraft);

  onMount(() => {
    loadSettingsBootstrap();
    loadExtensionsBootstrap();
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

  function loadExtensionsBootstrap() {
    extensionsLoading = true;
    paletteApi
      .getExtensionsBootstrap()
      .then((bootstrap) => {
        extensionsBootstrap = bootstrap;
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        extensionsLoading = false;
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
      await loadExtensionsBootstrap();
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
      .then(async (result) => {
        settingsMessage = result.message;
        settingsFailed = result.status === "failed";
        await loadExtensionsBootstrap();
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        settingsReloading = false;
      });
  }

  function setExtensionEnabled(extension: ExtensionRow, enabled: boolean) {
    if (!extensionsBootstrap || extensionMutationKey) {
      return;
    }

    const mutationKey = extensionKey(extension);
    extensionMutationKey = mutationKey;
    paletteApi
      .setExtensionEnabled({
        extension_id: extension.id,
        source_id: extension.source_id,
        enabled,
      })
      .then((result) => {
        if (!extensionsBootstrap) {
          return;
        }

        const applied = applyExtensionMutationResult(extensionsBootstrap, result);
        extensionsBootstrap = applied.extensions;
        settingsMessage = applied.message;
        settingsFailed = applied.failed;
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        extensionMutationKey = null;
      });
  }

  function uninstallExtension(extension: ExtensionRow) {
    if (!extensionsBootstrap || extensionMutationKey || !extension.can_uninstall) {
      return;
    }

    const mutationKey = extensionKey(extension);
    extensionMutationKey = mutationKey;
    paletteApi
      .uninstallExtension({
        extension_id: extension.id,
        source_id: extension.source_id,
      })
      .then((result) => {
        if (!extensionsBootstrap) {
          return;
        }

        const applied = applyExtensionMutationResult(extensionsBootstrap, result);
        extensionsBootstrap = applied.extensions;
        settingsMessage = applied.message;
        settingsFailed = applied.failed;
      })
      .catch((error: unknown) => {
        settingsMessage = errorMessage(error);
        settingsFailed = true;
      })
      .finally(() => {
        extensionMutationKey = null;
      });
  }

  function extensionKey(extension: ExtensionRow): string {
    return `${extension.source_id}/${extension.id}`;
  }

  function extensionKindLabel(extension: ExtensionRow): string {
    return extension.kind === "wasm_plugin" ? "Plugin" : "Static";
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

<main class="min-h-screen bg-zinc-950 text-zinc-100">
  <section class="flex min-h-screen">
    <aside class="w-56 border-r border-zinc-800 bg-zinc-900 p-4">
      <h1 class="text-lg font-semibold">Omni Palette</h1>
      <p class="text-sm text-zinc-400">Preferences</p>
      <nav class="mt-6 grid gap-2">
        <button
          class={[
            "rounded border px-3 py-2 text-left text-sm",
            activeSettingsPage === "general"
              ? "border-amber-500 bg-zinc-800 text-zinc-100"
              : "border-transparent text-zinc-300",
          ].join(" ")}
          onclick={() => (activeSettingsPage = "general")}
          type="button"
        >
          <span class="block font-medium">General</span>
          <span class="block text-xs text-zinc-500">Shortcut and config</span>
        </button>
        <button
          class={[
            "rounded border px-3 py-2 text-left text-sm",
            activeSettingsPage === "extensions"
              ? "border-amber-500 bg-zinc-800 text-zinc-100"
              : "border-transparent text-zinc-300",
          ].join(" ")}
          onclick={() => (activeSettingsPage = "extensions")}
          type="button"
        >
          <span class="block font-medium">Manage Extensions</span>
          <span class="block text-xs text-zinc-500">Enable and remove</span>
        </button>
        <button
          class={[
            "rounded border px-3 py-2 text-left text-sm",
            activeSettingsPage === "marketplace"
              ? "border-amber-500 bg-zinc-800 text-zinc-100"
              : "border-transparent text-zinc-300",
          ].join(" ")}
          onclick={() => (activeSettingsPage = "marketplace")}
          type="button"
        >
          <span class="block font-medium">Marketplace</span>
          <span class="block text-xs text-zinc-500">Browse and install</span>
        </button>
      </nav>
    </aside>

    <div class="flex-1 p-6">
      <header class="mb-6 flex items-start justify-between gap-4">
        <div>
          {#if activeSettingsPage === "general"}
            <h2 class="text-2xl font-semibold">General</h2>
            <p class="mt-1 text-sm text-zinc-400">
              Control how Omni Palette opens and where preferences are stored.
            </p>
          {:else if activeSettingsPage === "extensions"}
            <h2 class="text-2xl font-semibold">Installed Extensions</h2>
            <p class="mt-1 text-sm text-zinc-400">
              Manage extensions that are available on this device.
            </p>
          {:else}
            <h2 class="text-2xl font-semibold">Extension Marketplace</h2>
            <p class="mt-1 text-sm text-zinc-400">
              Configure the catalog source for future extension installs.
            </p>
          {/if}
        </div>
        <button
          class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-100 disabled:text-zinc-600"
          disabled={settingsReloading}
          onclick={reloadRuntimeState}
          type="button"
        >
          {settingsReloading ? "Reloading..." : "Reload extensions"}
        </button>
      </header>

      {#if settingsConfigError}
        <p class="mb-4 rounded border border-red-800 bg-red-950 px-3 py-2 text-sm text-red-200">
          {settingsConfigError}
        </p>
      {/if}
      {#if extensionsBootstrap?.install_root_error}
        <p class="mb-4 rounded border border-red-800 bg-red-950 px-3 py-2 text-sm text-red-200">
          {extensionsBootstrap.install_root_error}
        </p>
      {/if}
      {#if settingsMessage}
        <p
          class={[
            "mb-4 rounded border px-3 py-2 text-sm",
            settingsFailed
              ? "border-red-800 bg-red-950 text-red-200"
              : "border-emerald-800 bg-emerald-950 text-emerald-200",
          ].join(" ")}
        >
          {settingsMessage}
        </p>
      {/if}

      {#if settingsLoading || !settingsDraft}
        <div class="rounded border border-zinc-800 bg-zinc-900 p-6 text-sm text-zinc-400">
          Loading settings...
        </div>
      {:else}
        {#if activeSettingsPage === "general"}
          <div class="space-y-6">
            <fieldset class="rounded border border-zinc-800 bg-zinc-900 p-4">
              <legend class="px-1 text-sm font-medium text-zinc-200">Appearance</legend>
              <div class="mt-3 flex flex-wrap gap-2">
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

            <fieldset class="rounded border border-zinc-800 bg-zinc-900 p-4">
              <legend class="px-1 text-sm font-medium text-zinc-200">
                Activation shortcut
              </legend>
              <div class="mt-3 flex flex-wrap items-center gap-2">
                <span
                  class="rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-300"
                >
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
            </fieldset>

            <fieldset class="rounded border border-zinc-800 bg-zinc-900 p-4">
              <legend class="px-1 text-sm font-medium text-zinc-200">Command behavior</legend>
              <div class="mt-3 flex flex-wrap gap-2">
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

            <section class="rounded border border-zinc-800 bg-zinc-900 p-4">
              <h3 class="text-sm font-medium text-zinc-200">Storage</h3>
              <p class="mt-3 rounded border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-300">
                {settingsConfigPath ?? "Config path unavailable"}
              </p>
            </section>
          </div>
        {:else if activeSettingsPage === "extensions"}
          {#if extensionsLoading || !extensionsBootstrap}
            <div class="rounded border border-zinc-800 bg-zinc-900 p-6 text-sm text-zinc-400">
              Loading extensions...
            </div>
          {:else}
            <div class="space-y-6">
              <section class="rounded border border-zinc-800 bg-zinc-900 p-4">
                <h3 class="text-lg font-medium">Bundled Defaults</h3>
                <p class="text-sm text-zinc-400">
                  Built into Omni Palette. They can be disabled, but not uninstalled.
                </p>
                <div class="mt-4 grid gap-3">
                  {#each extensionsBootstrap.bundled_extensions as extension}
                    <article class="rounded border border-zinc-800 bg-zinc-950 p-4">
                      <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                          <h4 class="font-medium">
                            {extension.name}
                            <span class="text-xs text-zinc-500">{extension.version}</span>
                          </h4>
                          <div class="mt-1 flex flex-wrap gap-2 text-xs text-zinc-400">
                            <span>Bundled</span>
                            <span>{extensionKindLabel(extension)}</span>
                            <span>{extension.enabled ? "Enabled" : "Disabled"}</span>
                          </div>
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                          <label class="flex items-center gap-2 text-sm text-zinc-300">
                            <input
                              checked={extension.enabled}
                              disabled={extensionMutationKey === extensionKey(extension)}
                              onchange={(event) =>
                                setExtensionEnabled(extension, checkedValue(event))}
                              type="checkbox"
                            />
                            {extension.enabled ? "Enabled" : "Disabled"}
                          </label>
                          {#if extension.has_settings}
                            <button
                              class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-500"
                              disabled
                              title="Extension settings panels arrive in Phase 6C.3."
                              type="button"
                            >
                              Settings
                            </button>
                          {/if}
                        </div>
                      </div>
                    </article>
                  {/each}
                </div>
              </section>

              <section class="rounded border border-zinc-800 bg-zinc-900 p-4">
                <h3 class="text-lg font-medium">Downloaded Extensions</h3>
                <p class="text-sm text-zinc-400">Installed from your configured catalog.</p>
                {#if extensionsBootstrap.downloaded_extensions.length === 0}
                  <p class="mt-4 rounded border border-zinc-800 bg-zinc-950 px-3 py-4 text-sm text-zinc-400">
                    No downloaded extensions installed yet.
                  </p>
                {:else}
                  <div class="mt-4 grid gap-3">
                    {#each extensionsBootstrap.downloaded_extensions as extension}
                      <article class="rounded border border-zinc-800 bg-zinc-950 p-4">
                        <div class="flex flex-wrap items-center justify-between gap-3">
                          <div>
                            <h4 class="font-medium">
                              {extension.name}
                              <span class="text-xs text-zinc-500">{extension.version}</span>
                            </h4>
                            <div class="mt-1 flex flex-wrap gap-2 text-xs text-zinc-400">
                              <span>Downloaded</span>
                              <span>{extensionKindLabel(extension)}</span>
                              <span>{extension.enabled ? "Enabled" : "Disabled"}</span>
                            </div>
                          </div>
                          <div class="flex flex-wrap items-center gap-2">
                            <label class="flex items-center gap-2 text-sm text-zinc-300">
                              <input
                                checked={extension.enabled}
                                disabled={extensionMutationKey === extensionKey(extension)}
                                onchange={(event) =>
                                  setExtensionEnabled(extension, checkedValue(event))}
                                type="checkbox"
                              />
                              {extension.enabled ? "Enabled" : "Disabled"}
                            </label>
                            {#if extension.has_settings}
                              <button
                                class="rounded border border-zinc-700 px-3 py-2 text-sm text-zinc-500"
                                disabled
                                title="Extension settings panels arrive in Phase 6C.3."
                                type="button"
                              >
                                Settings
                              </button>
                            {/if}
                            <button
                              class="rounded border border-red-800 px-3 py-2 text-sm text-red-200 disabled:text-zinc-600"
                              disabled={extensionMutationKey === extensionKey(extension)}
                              onclick={() => uninstallExtension(extension)}
                              type="button"
                            >
                              Uninstall
                            </button>
                          </div>
                        </div>
                      </article>
                    {/each}
                  </div>
                {/if}
              </section>
            </div>
          {/if}
        {:else}
          <div class="space-y-6">
            <fieldset class="rounded border border-zinc-800 bg-zinc-900 p-4">
              <legend class="px-1 text-sm font-medium text-zinc-200">Catalog source</legend>
              <p class="mb-4 text-sm text-zinc-400">
                Catalog refresh and install arrive in Phase 6C.2.
              </p>
              <label class="flex items-center gap-3 text-sm text-zinc-300">
                <input
                  checked={settingsDraft.github.enabled}
                  class="h-4 w-4 rounded border-zinc-700 bg-zinc-950"
                  onchange={(event) => updateCatalogEnabled(checkedValue(event))}
                  type="checkbox"
                />
                Enable GitHub catalog source
              </label>
              <div class="mt-4 grid gap-3 sm:grid-cols-2">
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
        {/if}

        {#if activeSettingsPage !== "extensions"}
          <div class="mt-6 flex items-center justify-between gap-3 border-t border-zinc-800 pt-4">
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
      {/if}
    </div>
  </section>
</main>
