import { describe, expect, it } from "bun:test";

import { filterCommands, sampleCommands } from "./commands";

describe("filterCommands", () => {
  it("keeps commands whose label or tags match the query", () => {
    const rows = filterCommands(sampleCommands, "reload");

    expect(rows.map((row) => row.label)).toEqual(["Omni Palette: Reload extensions"]);
  });

  it("returns all sample commands for an empty query", () => {
    const rows = filterCommands(sampleCommands, "");

    expect(rows).toHaveLength(sampleCommands.length);
  });

  it("returns no rows when there is no match", () => {
    const rows = filterCommands(sampleCommands, "definitely missing");

    expect(rows).toEqual([]);
  });
});
