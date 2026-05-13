import { describe, expect, it } from "bun:test";

import { CommandRow, createPaletteApi, nextSelectedCommandId } from "./commands";

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
