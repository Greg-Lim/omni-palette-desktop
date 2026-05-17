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
    expect(mainSource).toContain('from "./Settings.svelte"');
    expect(mainSource).toContain('from "./DebugOverlay.svelte"');
    expect(mainSource).toContain("getCurrentWindow().label");
    expect(mainSource).toContain('label === "settings"');
    expect(mainSource).toContain('label === "debug"');
    expect(mainSource).toContain("mount(Component,");
  });

  it("keeps settings controls out of the hotkey palette surface", () => {
    const appSource = readFileSync(join(srcDir, "App.svelte"), "utf8");

    expect(appSource).toContain("openSettingsFromPalette");
    expect(appSource).toContain("refreshExtensionsFromPalette");
    expect(appSource).not.toContain("Backend:");
    expect(appSource).not.toContain("activeView");
    expect(appSource).not.toContain("Activation shortcut");
    expect(appSource).not.toContain("Save settings");
  });

  it("renders Phase 6C settings navigation and surfaces in the settings window", () => {
    const settingsPath = join(srcDir, "Settings.svelte");

    expect(existsSync(settingsPath)).toBe(true);

    if (!existsSync(settingsPath)) {
      return;
    }

    const settingsSource = readFileSync(settingsPath, "utf8");
    expect(settingsSource).toContain("General");
    expect(settingsSource).toContain("Manage Extensions");
    expect(settingsSource).toContain("Marketplace");
    expect(settingsSource).toContain("Activation shortcut");
    expect(settingsSource).toContain("Record");
    expect(settingsSource).toContain("Reset");
    expect(settingsSource).toContain("Command behavior");
    expect(settingsSource).toContain("Pop up debugger");
    expect(settingsSource).toContain("showDebugOverlay");
    expect(settingsSource).toContain("Bundled Defaults");
    expect(settingsSource).toContain("Downloaded Extensions");
    expect(settingsSource).toContain("No downloaded extensions installed yet.");
    expect(settingsSource).toContain("Catalog source");
    expect(settingsSource).toContain("Save Source");
    expect(settingsSource).toContain("Refresh Catalog");
    expect(settingsSource).toContain("Available Extensions");
    expect(settingsSource).toContain("Search catalog");
    expect(settingsSource).toContain("Install");
    expect(settingsSource).toContain("Save settings");
    expect(settingsSource).toContain("getExtensionSettings");
    expect(settingsSource).toContain("saveExtensionSettings");
    expect(settingsSource).toContain("extensionSettingsPanel");
    expect(settingsSource).toContain("Reset Defaults");
    expect(settingsSource).toContain("Add Entry");
    expect(settingsSource).not.toContain("Extension settings panels arrive in Phase 6C.3.");
  });

  it("renders a separate debug overlay surface", () => {
    const debugPath = join(srcDir, "DebugOverlay.svelte");

    expect(existsSync(debugPath)).toBe(true);

    if (!existsSync(debugPath)) {
      return;
    }

    const debugSource = readFileSync(debugPath, "utf8");
    expect(debugSource).toContain("getDebugSnapshot");
    expect(debugSource).toContain("closeDebugOverlay");
    expect(debugSource).toContain("Foreground");
    expect(debugSource).toContain("Interaction");
    expect(debugSource).toContain("Command Candidates");
    expect(debugSource).toContain("Palette Filter");
    expect(debugSource).toContain("Background Windows");
  });

  it("declares separate palette, settings, and guide Tauri windows", () => {
    const config = JSON.parse(
      readFileSync(join(appRoot, "src-tauri", "tauri.conf.json"), "utf8"),
    );
    const windows = config.app.windows as Array<Record<string, unknown>>;
    const mainWindow = windows.find((window) => window.label === "main");
    const settingsWindow = windows.find((window) => window.label === "settings");
    const guideWindow = windows.find((window) => window.label === "guide");
    const debugWindow = windows.find((window) => window.label === "debug");

    expect(mainWindow).toMatchObject({
      label: "main",
      width: 780,
      decorations: false,
      visible: false,
    });
    expect(settingsWindow).toMatchObject({
      label: "settings",
      title: "Omni Palette Settings",
      decorations: true,
      resizable: true,
      visible: false,
    });
    expect(guideWindow).toMatchObject({
      label: "guide",
      visible: false,
    });
    expect(debugWindow).toMatchObject({
      label: "debug",
      title: "Omni Palette Debug",
      decorations: true,
      resizable: true,
      visible: false,
    });
  });
});
