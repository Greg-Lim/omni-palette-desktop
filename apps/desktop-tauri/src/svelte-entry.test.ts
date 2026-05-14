import { describe, expect, it } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const srcDir = import.meta.dir;
const appRoot = join(srcDir, "..");

describe("Svelte frontend entrypoint", () => {
  it("loads the Svelte TypeScript entrypoint from index.html", () => {
    const indexHtml = readFileSync(join(appRoot, "index.html"), "utf8");

    expect(indexHtml).toContain('src="/src/main.ts"');
    expect(indexHtml).not.toContain("/src/main.tsx");
    expect(existsSync(join(srcDir, "main.ts"))).toBe(true);
    expect(existsSync(join(srcDir, "main.tsx"))).toBe(false);
  });

  it("mounts the palette or guide Svelte app from the entrypoint", () => {
    const mainPath = join(srcDir, "main.ts");

    expect(existsSync(mainPath)).toBe(true);

    if (!existsSync(mainPath)) {
      return;
    }

    const mainSource = readFileSync(mainPath, "utf8");
    expect(mainSource).toContain('from "svelte"');
    expect(mainSource).toContain('from "./App.svelte"');
    expect(mainSource).toContain('from "./Guide.svelte"');
    expect(mainSource).toContain("getCurrentWindow().label");
    expect(mainSource).toContain("mount(Component,");
  });
});
