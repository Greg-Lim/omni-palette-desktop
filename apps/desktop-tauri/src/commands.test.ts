import { describe, expect, it } from "bun:test";

import {
  CommandRow,
  RuntimeStatus,
  createPaletteApi,
  formatRuntimeStatus,
  nextSelectedCommandId,
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
