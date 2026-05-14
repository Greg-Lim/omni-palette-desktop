import { describe, expect, it } from "bun:test";

import {
  CommandRow,
  CommandExecutionResult,
  HotkeyStatus,
  RuntimeStatus,
  WindowLifecycleStatus,
  createPaletteApi,
  commandExecutionShouldHidePalette,
  formatHotkeyStatus,
  formatGuideStatus,
  formatRuntimeStatus,
  formatWindowLifecycleStatus,
  guideShortcutParts,
  highlightedLabelSegments,
  nextKeyboardSelectedCommandId,
  nextGuideStatus,
  nextWindowLifecycleStatus,
  paletteKeyAction,
  nextSelectedCommandId,
  shouldStartGuideForCommand,
  shouldHidePaletteForWindowBlur,
  shouldRefreshCommandsForWindowLifecycleEvent,
} from "./commands";

const rows: CommandRow[] = [
  {
    id: "reload-extensions",
    label: "Omni Palette: Reload extensions",
    shortcut_text: "",
    focus_state: "global",
    priority: "medium",
    favorite: false,
    tags: ["extensions", "reload"],
    original_order: 0,
    score: 0,
    label_matches: [],
    guide_hint: null,
  },
  {
    id: "chrome-new-tab",
    label: "Chrome: New tab",
    shortcut_text: "Ctrl+T",
    focus_state: "focused",
    priority: "high",
    favorite: false,
    tags: ["browser", "tabs"],
    original_order: 1,
    score: 12,
    label_matches: [{ start: 8, end: 11 }],
    guide_hint: {
      shortcut_text: "Ctrl+T",
      captures_shortcut: true,
    },
  },
];

describe("palette api", () => {
  it("calls the backend bootstrap command and preserves runtime status", async () => {
    const runtimeStatus: RuntimeStatus = {
      config_path: "C:/Users/example/AppData/Roaming/OmniPalette/config.toml",
      config_error: null,
      activation_hint: "Ctrl+Space",
      command_behavior: "execute",
      application_count: 4,
      ignored_process_count: 2,
      plugin_count: 1,
      plugin_application_count: 1,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return {
        session_id: "session-1",
        backend_status: "ok",
        runtime_status: runtimeStatus,
        commands: rows,
      } as T;
    });

    const bootstrap = await api.getPaletteBootstrap();

    expect(calls).toEqual([{ command: "get_palette_bootstrap", args: undefined }]);
    expect(bootstrap.runtime_status).toEqual(runtimeStatus);
    expect(bootstrap.commands).toEqual(rows);
  });

  it("calls the backend search command with the current query", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return { session_id: "session-1", query: "new", commands: rows } as T;
    });

    const snapshot = await api.searchCommands("new");

    expect(calls).toEqual([{ command: "search_commands", args: { query: "new" } }]);
    expect(snapshot.commands).toEqual(rows);
  });

  it("calls the backend execution command with the selected id", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return { status: "deferred", message: "Deferred" } as T;
    });

    const result = await api.executeCommand("chrome-new-tab");

    expect(calls).toEqual([
      { command: "execute_command", args: { commandId: "chrome-new-tab" } },
    ]);
    expect(result.status).toBe("deferred");
  });

  it("preserves a successful backend execution result", async () => {
    const api = createPaletteApi(async <T>() => {
      return { status: "succeeded", message: "Executed Chrome: New tab" } as T;
    });

    const result = await api.executeCommand("chrome-new-tab");

    expect(result).toEqual({
      status: "succeeded",
      message: "Executed Chrome: New tab",
    });
  });

  it("calls the backend hotkey status command and preserves payload", async () => {
    const hotkeyStatus: HotkeyStatus = {
      running: true,
      activation_hint: "Ctrl+Shift+P",
      activation_count: 2,
      ignored_passthrough_count: 1,
      last_event: {
        kind: "activation_requested",
        shortcut: "Ctrl+Shift+P",
        process_name: "notepad.exe",
        activation_count: 2,
        ignored_passthrough_count: 1,
        message: null,
      },
      last_error: null,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return hotkeyStatus as T;
    });

    const status = await api.getHotkeyStatus();

    expect(calls).toEqual([{ command: "get_hotkey_status", args: undefined }]);
    expect(status).toEqual(hotkeyStatus);
  });

  it("calls the backend window lifecycle status command and preserves payload", async () => {
    const windowStatus: WindowLifecycleStatus = {
      visible: true,
      show_count: 2,
      hide_count: 1,
      focus_count: 2,
      position_count: 2,
      last_action: "shown",
      last_error: null,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return windowStatus as T;
    });

    const status = await api.getWindowLifecycleStatus();

    expect(calls).toEqual([{ command: "get_window_lifecycle_status", args: undefined }]);
    expect(status).toEqual(windowStatus);
  });

  it("calls the backend hide palette command and preserves payload", async () => {
    const windowStatus: WindowLifecycleStatus = {
      visible: false,
      show_count: 1,
      hide_count: 1,
      focus_count: 1,
      position_count: 1,
      last_action: "hidden",
      last_error: null,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return windowStatus as T;
    });

    const status = await api.hidePaletteWindow();

    expect(calls).toEqual([{ command: "hide_palette_window", args: undefined }]);
    expect(status).toEqual(windowStatus);
  });

  it("calls the backend guide commands and preserves payloads", async () => {
    const guideStatus = {
      active: true,
      command_label: "Chrome: New tab",
      shortcut_text: "Ctrl+T",
      activation_hint: "Ctrl+Shift+P",
      start_count: 1,
      complete_count: 0,
      cancel_count: 0,
      expire_count: 0,
      last_action: "started" as const,
      last_error: null,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return guideStatus as T;
    });

    expect(await api.startGuide("chrome-new-tab")).toEqual(guideStatus);
    expect(await api.cancelGuide()).toEqual(guideStatus);
    expect(await api.getGuideStatus()).toEqual(guideStatus);
    expect(calls).toEqual([
      { command: "start_guide", args: { commandId: "chrome-new-tab" } },
      { command: "cancel_guide", args: undefined },
      { command: "get_guide_status", args: undefined },
    ]);
  });
});

describe("formatRuntimeStatus", () => {
  it("summarizes runtime metadata for the status strip", () => {
    expect(
      formatRuntimeStatus({
        config_path: "C:/Users/example/AppData/Roaming/OmniPalette/config.toml",
        config_error: null,
        activation_hint: "Ctrl+Space",
        command_behavior: "execute",
        application_count: 4,
        ignored_process_count: 2,
        plugin_count: 1,
        plugin_application_count: 1,
      }),
    ).toBe("Ctrl+Space - execute - 4 apps - 2 ignored - 1 plugins");
  });
});

describe("formatHotkeyStatus", () => {
  it("summarizes running activation and passthrough state", () => {
    expect(
      formatHotkeyStatus({
        running: true,
        activation_hint: "Ctrl+Shift+P",
        activation_count: 2,
        ignored_passthrough_count: 1,
        last_event: {
          kind: "ignored_passthrough",
          shortcut: "Ctrl+Shift+P",
          process_name: "Code.exe",
          activation_count: 2,
          ignored_passthrough_count: 1,
          message: null,
        },
        last_error: null,
      }),
    ).toBe("hotkey on - Ctrl+Shift+P - 2 activations - 1 passthrough");
  });

  it("shows listener errors before event counts", () => {
    expect(
      formatHotkeyStatus({
        running: false,
        activation_hint: "Ctrl+Shift+P",
        activation_count: 0,
        ignored_passthrough_count: 0,
        last_event: {
          kind: "listener_error",
          shortcut: "Ctrl+Shift+P",
          process_name: null,
          activation_count: 0,
          ignored_passthrough_count: 0,
          message: "failed to register hotkey",
        },
        last_error: "failed to register hotkey",
      }),
    ).toBe("hotkey error - failed to register hotkey");
  });
});

describe("window lifecycle status", () => {
  it("summarizes visible window lifecycle state", () => {
    expect(
      formatWindowLifecycleStatus({
        visible: true,
        show_count: 2,
        hide_count: 1,
        focus_count: 2,
        position_count: 2,
        last_action: "shown",
        last_error: null,
      }),
    ).toBe("window visible - shown - 2 shown - 1 hidden");
  });

  it("shows lifecycle errors before counters", () => {
    expect(
      formatWindowLifecycleStatus({
        visible: false,
        show_count: 0,
        hide_count: 0,
        focus_count: 0,
        position_count: 0,
        last_action: "error",
        last_error: "Failed to show palette window: boom",
      }),
    ).toBe("window error - Failed to show palette window: boom");
  });

  it("applies lifecycle events to the latest status", () => {
    const status = nextWindowLifecycleStatus(
      {
        visible: false,
        show_count: 0,
        hide_count: 0,
        focus_count: 0,
        position_count: 0,
        last_action: null,
        last_error: null,
      },
      {
        action: "shown",
        visible: true,
        show_count: 1,
        hide_count: 0,
        focus_count: 1,
        position_count: 1,
        message: null,
      },
    );

    expect(status).toEqual({
      visible: true,
      show_count: 1,
      hide_count: 0,
      focus_count: 1,
      position_count: 1,
      last_action: "shown",
      last_error: null,
    });
  });

  it("refreshes commands only when the window is shown", () => {
    expect(
      shouldRefreshCommandsForWindowLifecycleEvent({
        action: "shown",
        visible: true,
        show_count: 1,
        hide_count: 0,
        focus_count: 1,
        position_count: 1,
        message: null,
      }),
    ).toBe(true);
    expect(
      shouldRefreshCommandsForWindowLifecycleEvent({
        action: "hidden",
        visible: false,
        show_count: 1,
        hide_count: 1,
        focus_count: 1,
        position_count: 1,
        message: null,
      }),
    ).toBe(false);
    expect(
      shouldRefreshCommandsForWindowLifecycleEvent({
        action: "error",
        visible: false,
        show_count: 0,
        hide_count: 0,
        focus_count: 0,
        position_count: 0,
        message: "Failed to show palette window: boom",
      }),
    ).toBe(false);
  });
});

describe("guide status", () => {
  it("summarizes active guide state", () => {
    expect(
      formatGuideStatus({
        active: true,
        command_label: "Chrome: New tab",
        shortcut_text: "Ctrl+T",
        activation_hint: "Ctrl+Shift+P",
        start_count: 1,
        complete_count: 0,
        cancel_count: 0,
        expire_count: 0,
        last_action: "started",
        last_error: null,
      }),
    ).toBe("guide active - Chrome: New tab - Ctrl+T");
  });

  it("applies guide events without requesting palette command refresh", () => {
    const status = nextGuideStatus(null, {
      action: "cancelled",
      active: false,
      command_label: null,
      shortcut_text: null,
      activation_hint: "Ctrl+Shift+P",
      start_count: 1,
      complete_count: 0,
      cancel_count: 1,
      expire_count: 0,
      message: null,
    });

    expect(status).toEqual({
      active: false,
      command_label: null,
      shortcut_text: null,
      activation_hint: "Ctrl+Shift+P",
      start_count: 1,
      complete_count: 0,
      cancel_count: 1,
      expire_count: 0,
      last_action: "cancelled",
      last_error: null,
    });
  });
});

describe("nextSelectedCommandId", () => {
  it("keeps the current selection when it remains visible", () => {
    expect(nextSelectedCommandId("chrome-new-tab", rows)).toBe("chrome-new-tab");
  });

  it("selects the first row when the current selection disappears", () => {
    expect(nextSelectedCommandId("missing", rows)).toBe("reload-extensions");
  });

  it("clears the selection when there are no rows", () => {
    expect(nextSelectedCommandId("missing", [])).toBe("");
  });
});

describe("nextKeyboardSelectedCommandId", () => {
  it("wraps down from the last visible row to the first", () => {
    expect(nextKeyboardSelectedCommandId("chrome-new-tab", rows, 1)).toBe("reload-extensions");
  });

  it("wraps up from the first visible row to the last", () => {
    expect(nextKeyboardSelectedCommandId("reload-extensions", rows, -1)).toBe("chrome-new-tab");
  });

  it("selects the first row when there is no current selection", () => {
    expect(nextKeyboardSelectedCommandId("", rows, 1)).toBe("reload-extensions");
  });

  it("clears selection when there are no rows", () => {
    expect(nextKeyboardSelectedCommandId("chrome-new-tab", [], 1)).toBe("");
  });
});

describe("commandExecutionShouldHidePalette", () => {
  it("hides only after successful command execution", () => {
    const succeeded: CommandExecutionResult = {
      status: "succeeded",
      message: "Executed Chrome: New tab",
    };
    const failed: CommandExecutionResult = {
      status: "failed",
      message: "Failed to execute Chrome: New tab",
    };
    const deferred: CommandExecutionResult = {
      status: "deferred",
      message: "Deferred",
    };

    expect(commandExecutionShouldHidePalette(succeeded)).toBe(true);
    expect(commandExecutionShouldHidePalette(failed)).toBe(false);
    expect(commandExecutionShouldHidePalette(deferred)).toBe(false);
  });
});

describe("guide command activation", () => {
  it("starts guide only when runtime behavior is guide and the command is guideable", () => {
    const runtimeStatus: RuntimeStatus = {
      config_path: null,
      config_error: null,
      activation_hint: "Ctrl+Shift+P",
      command_behavior: "guide",
      application_count: 1,
      ignored_process_count: 0,
      plugin_count: 0,
      plugin_application_count: 0,
    };

    expect(shouldStartGuideForCommand(runtimeStatus, rows[1])).toBe(true);
    expect(shouldStartGuideForCommand(runtimeStatus, rows[0])).toBe(false);
    expect(
      shouldStartGuideForCommand({ ...runtimeStatus, command_behavior: "execute" }, rows[1]),
    ).toBe(false);
  });
});

describe("paletteKeyAction", () => {
  it("maps core palette keys to UX actions", () => {
    expect(paletteKeyAction("ArrowDown")).toBe("select_next");
    expect(paletteKeyAction("ArrowUp")).toBe("select_previous");
    expect(paletteKeyAction("Enter")).toBe("execute");
    expect(paletteKeyAction("Escape")).toBe("hide");
    expect(paletteKeyAction("Tab")).toBeNull();
  });
});

describe("shouldHidePaletteForWindowBlur", () => {
  it("hides only when the lifecycle status says the palette is visible", () => {
    expect(
      shouldHidePaletteForWindowBlur({
        visible: true,
        show_count: 1,
        hide_count: 0,
        focus_count: 1,
        position_count: 1,
        last_action: "shown",
        last_error: null,
      }),
    ).toBe(true);
    expect(
      shouldHidePaletteForWindowBlur({
        visible: false,
        show_count: 1,
        hide_count: 1,
        focus_count: 1,
        position_count: 1,
        last_action: "hidden",
        last_error: null,
      }),
    ).toBe(false);
    expect(shouldHidePaletteForWindowBlur(null)).toBe(false);
  });
});

describe("highlightedLabelSegments", () => {
  it("splits label text into plain and highlighted segments", () => {
    expect(
      highlightedLabelSegments("Chrome: New tab", [
        { start: 0, end: 3 },
        { start: 8, end: 11 },
      ]),
    ).toEqual([
      { text: "Chr", highlighted: true },
      { text: "ome: ", highlighted: false },
      { text: "New", highlighted: true },
      { text: " tab", highlighted: false },
    ]);
  });

  it("ignores invalid ranges safely", () => {
    expect(
      highlightedLabelSegments("Chrome", [
        { start: 3, end: 3 },
        { start: 7, end: 9 },
        { start: 1, end: 4 },
      ]),
    ).toEqual([
      { text: "C", highlighted: false },
      { text: "hro", highlighted: true },
      { text: "me", highlighted: false },
    ]);
  });
});

describe("guideShortcutParts", () => {
  it("parses single chord and sequence shortcuts into keycaps", () => {
    expect(guideShortcutParts("Ctrl+T")).toEqual([["Ctrl", "T"]]);
    expect(guideShortcutParts("Alt+J, I")).toEqual([["Alt", "J"], ["I"]]);
  });
});
