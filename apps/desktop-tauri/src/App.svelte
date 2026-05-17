<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  import type {
    CommandExecutionResult,
    CommandRow,
    GuideEventPayload,
    GuideStatus,
    RuntimeStatus,
    WindowLifecycleEventPayload,
    WindowLifecycleStatus,
  } from "./commands";
  import {
    GUIDE_EVENT_NAME,
    WINDOW_LIFECYCLE_EVENT_NAME,
    commandExecutionShouldHidePalette,
    highlightedLabelSegments,
    isOpenSettingsCommand,
    isRefreshExtensionsCommand,
    nextKeyboardSelectedCommandId,
    nextGuideStatus,
    nextSelectedCommandId,
    nextWindowLifecycleStatus,
    openSettingsFromPalette,
    paletteKeyAction,
    paletteApi,
    paletteRowsWithFixedActions,
    refreshExtensionsFromPalette,
    selectedRowScrollTop,
    shouldStartGuideForCommand,
    shouldHidePaletteForWindowBlur,
    shouldRefreshCommandsForWindowLifecycleEvent,
  } from "./commands";

  let query = "";
  let selectedId = "";
  let rows: CommandRow[] = [];
  let runtimeStatus: RuntimeStatus | null = null;
  let windowLifecycleStatus: WindowLifecycleStatus | null = null;
  let guideStatus: GuideStatus | null = null;
  let commandError: string | null = null;
  let loadingCommands = true;
  let executionResult: CommandExecutionResult | null = null;
  let searchInput: HTMLInputElement | null = null;
  let resultsScroller: HTMLDivElement | null = null;
  let showTopFade = false;
  let showBottomFade = false;
  let searchRun = 0;
  let hidingPalette = false;
  const commandRowElements = new Map<string, HTMLButtonElement>();

  $: searchCommands(query);

  onMount(() => {
    searchInput?.focus();

    paletteApi
      .getPaletteBootstrap()
      .then((bootstrap) => {
        runtimeStatus = bootstrap.runtime_status;
      })
      .catch((error: unknown) => {
        commandError = errorMessage(error);
      });

    paletteApi
      .getWindowLifecycleStatus()
      .then((status) => {
        windowLifecycleStatus = status;
      })
      .catch((error: unknown) => {
        commandError = errorMessage(error);
      });

    paletteApi
      .getGuideStatus()
      .then((status) => {
        guideStatus = status;
      })
      .catch((error: unknown) => {
        commandError = errorMessage(error);
      });

    let unlistenWindowLifecycleEvents: (() => void) | null = null;
    listen<WindowLifecycleEventPayload>(WINDOW_LIFECYCLE_EVENT_NAME, (event) => {
      windowLifecycleStatus = nextWindowLifecycleStatus(windowLifecycleStatus, event.payload);
      if (shouldRefreshCommandsForWindowLifecycleEvent(event.payload)) {
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
        commandError = errorMessage(error);
      });

    let unlistenGuideEvents: (() => void) | null = null;
    listen<GuideEventPayload>(GUIDE_EVENT_NAME, (event) => {
      guideStatus = nextGuideStatus(guideStatus, event.payload);
    })
      .then((unlisten) => {
        unlistenGuideEvents = unlisten;
      })
      .catch((error: unknown) => {
        commandError = errorMessage(error);
      });

    window.addEventListener("blur", handleWindowBlur);

    return () => {
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

        rows = paletteRowsWithFixedActions(snapshot.commands);
        selectedId = nextSelectedCommandId(selectedId, rows);
        commandError = null;
        tick().then(resetResultsScrollToTop);
      })
      .catch((error: unknown) => {
        if (run !== searchRun) {
          return;
        }

        rows = paletteRowsWithFixedActions([]);
        selectedId = nextSelectedCommandId(selectedId, rows);
        commandError = errorMessage(error);
        tick().then(resetResultsScrollToTop);
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
    if (isRefreshExtensionsCommand(commandId)) {
      executionResult = null;
      refreshExtensionsFromPalette(paletteApi)
        .then((result) => {
          executionResult = {
            status: result.status === "succeeded" ? "succeeded" : "failed",
            message: result.message,
          };
          if (result.status === "succeeded") {
            hidePaletteWindow();
          }
        })
        .catch((error: unknown) => {
          executionResult = {
            status: "failed",
            message: errorMessage(error),
          };
        });
      return;
    }

    if (isOpenSettingsCommand(commandId)) {
      executionResult = null;
      openSettingsFromPalette(paletteApi)
        .then((result) => {
          windowLifecycleStatus = result.window_status;
          if (result.settings_status.status === "failed") {
            executionResult = {
              status: "failed",
              message: result.settings_status.message,
            };
          }
        })
        .catch((error: unknown) => {
          executionResult = {
            status: "failed",
            message: errorMessage(error),
          };
        });
      return;
    }

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
    if (!action) {
      return;
    }

    event.preventDefault();
    if (action === "select_next") {
      const nextId = nextKeyboardSelectedCommandId(selectedId, rows, 1);
      selectedId = nextId;
      scrollSelectedCommandIntoView(nextId);
    } else if (action === "select_previous") {
      const nextId = nextKeyboardSelectedCommandId(selectedId, rows, -1);
      selectedId = nextId;
      scrollSelectedCommandIntoView(nextId);
    } else if (action === "execute") {
      runCommand(selectedId);
    } else {
      hidePaletteWindow();
    }
  }

  function scrollSelectedCommandIntoView(commandId: string) {
    tick().then(() => {
      const scroller = resultsScroller;
      const row = commandRowElements.get(commandId);
      if (!scroller || !row) {
        return;
      }

      const scrollerRect = scroller.getBoundingClientRect();
      const rowRect = row.getBoundingClientRect();
      const nextScrollTop = selectedRowScrollTop({
        currentScrollTop: scroller.scrollTop,
        containerHeight: scroller.clientHeight,
        scrollHeight: scroller.scrollHeight,
        rowTop: rowRect.top - scrollerRect.top + scroller.scrollTop,
        rowHeight: rowRect.height,
      });
      scroller.scrollTo({ top: nextScrollTop, behavior: "smooth" });
    });
  }

  function updateResultsFade() {
    const scroller = resultsScroller;
    if (!scroller) {
      showTopFade = false;
      showBottomFade = false;
      return;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    showTopFade = scroller.scrollTop > 1;
    showBottomFade = scroller.scrollTop < maxScrollTop - 1;
  }

  function resetResultsScrollToTop() {
    const scroller = resultsScroller;
    if (scroller) {
      scroller.scrollTop = 0;
    }
    updateResultsFade();
  }

  function handleResultsScroll() {
    updateResultsFade();
  }

  function trackCommandRow(node: HTMLButtonElement, commandId: string) {
    commandRowElements.set(commandId, node);
    return {
      update(nextCommandId: string) {
        commandRowElements.delete(commandId);
        commandId = nextCommandId;
        commandRowElements.set(commandId, node);
      },
      destroy() {
        commandRowElements.delete(commandId);
      },
    };
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
        commandError = errorMessage(error);
      })
      .finally(() => {
        hidingPalette = false;
      });
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<svelte:window onkeydown={handlePaletteKeydown} />

<main class="h-screen overflow-hidden bg-zinc-950 p-3 text-zinc-100">
  <section class="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-zinc-700 bg-zinc-900">
    <div class="shrink-0 bg-zinc-900">
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
    </div>

    <div class="relative min-h-0 flex-1">
      <div
        bind:this={resultsScroller}
        class="palette-results-scroll h-full overflow-y-auto p-2 pb-14"
        onscroll={handleResultsScroll}
      >
        {#if rows.length === 0}
          <div class="rounded-md border border-dashed border-zinc-700 p-8 text-center text-sm text-zinc-400">
            {loadingCommands ? "Loading commands..." : "No matching commands"}
          </div>
        {:else}
          {#each rows as command (command.id)}
            {@const selected = command.id === selectedId}
            <button
              use:trackCommandRow={command.id}
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
      <div
        aria-hidden="true"
        class={[
          "pointer-events-none absolute inset-x-0 top-0 h-10 bg-gradient-to-b from-zinc-900 to-transparent transition-opacity",
          showTopFade ? "opacity-100" : "opacity-0",
        ].join(" ")}
      ></div>
      <div
        aria-hidden="true"
        class={[
          "pointer-events-none absolute inset-x-0 bottom-0 h-10 bg-gradient-to-t from-zinc-900 to-transparent transition-opacity",
          showBottomFade ? "opacity-100" : "opacity-0",
        ].join(" ")}
      ></div>
    </div>
  </section>
</main>

<style>
  .palette-results-scroll {
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  .palette-results-scroll::-webkit-scrollbar {
    display: none;
  }
</style>
