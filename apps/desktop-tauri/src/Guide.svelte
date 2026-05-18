<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  import type { GuideEventPayload, GuideStatus } from "./commands";
  import {
    GUIDE_EVENT_NAME,
    guideShortcutParts,
    nextGuideStatus,
    paletteApi,
  } from "./commands";

  let guideStatus: GuideStatus | null = null;
  let error: string | null = null;

  $: shortcutParts = guideShortcutParts(guideStatus?.shortcut_text ?? "");
  $: fallbackText = `${guideStatus?.activation_hint ?? "Ctrl+Shift+P"} to run for me`;

  onMount(() => {
    paletteApi
      .getGuideStatus()
      .then((status) => {
        guideStatus = status;
      })
      .catch((caught: unknown) => {
        error = errorMessage(caught);
      });

    let unlistenGuideEvents: (() => void) | null = null;
    listen<GuideEventPayload>(GUIDE_EVENT_NAME, (event) => {
      guideStatus = nextGuideStatus(guideStatus, event.payload);
    })
      .then((unlisten) => {
        unlistenGuideEvents = unlisten;
      })
      .catch((caught: unknown) => {
        error = errorMessage(caught);
      });

    return () => {
      unlistenGuideEvents?.();
    };
  });

  function cancelGuide() {
    paletteApi
      .cancelGuide()
      .then((status) => {
        guideStatus = status;
      })
      .catch((caught: unknown) => {
        error = errorMessage(caught);
      });
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      cancelGuide();
    }
  }

  function errorMessage(caught: unknown): string {
    return caught instanceof Error ? caught.message : String(caught);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<main class="flex min-h-screen items-center justify-center bg-transparent p-4 text-zinc-100">
  <section class="w-full rounded-lg border border-amber-500/60 bg-zinc-950/[0.92] px-5 py-4">
    {#if error}
      <p class="text-sm text-red-300">{error}</p>
    {:else if guideStatus?.active}
      <p class="text-sm font-semibold">{guideStatus.command_label}</p>

      {#if shortcutParts.length > 0}
        <div class="mt-4 flex flex-wrap items-center gap-2">
          {#each shortcutParts as chord, chordIndex}
            {#if chordIndex > 0}
              <span class="text-xs text-zinc-500">then</span>
            {/if}
            <span class="flex items-center gap-1">
              {#each chord as key}
                <kbd class="min-h-12 min-w-16 rounded-md border border-zinc-600 bg-zinc-900 px-4 py-3 text-center text-base font-semibold text-amber-200">
                  {key}
                </kbd>
              {/each}
            </span>
          {/each}
        </div>
      {/if}

      <p class="mt-4 text-xs text-zinc-400">{fallbackText}</p>
    {:else}
      <p class="text-sm text-zinc-400">Guide idle</p>
    {/if}
  </section>
</main>
