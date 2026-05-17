import { describe, expect, it } from "bun:test";

import {
  ActivationShortcut,
  CatalogEntry,
  CatalogRefreshResult,
  CommandRow,
  CommandExecutionResult,
  DebugOverlayStatus,
  DebugSnapshot,
  ExtensionSettingItem,
  ExtensionSettingsBootstrap,
  ExtensionSettingsSaveResult,
  ExtensionMutationResult,
  ExtensionsBootstrap,
  HotkeyStatus,
  RuntimeStatus,
  RuntimeSettings,
  RuntimeSettingsSaveResult,
  RuntimeSettingsSaveRequest,
  SettingsWindowStatus,
  WindowLifecycleStatus,
  OPEN_SETTINGS_COMMAND_ID,
  REFRESH_EXTENSIONS_COMMAND_ID,
  activationShortcutFromKeyboardEvent,
  applyExtensionMutationResult,
  applyCatalogRefreshResult,
  addExtensionSettingListEntry,
  createPaletteApi,
  commandExecutionShouldHidePalette,
  defaultExtensionSettingsValues,
  extensionSettingsAreDirty,
  extensionSettingsSaveRequestFromDraft,
  extensionSettingsSections,
  formatHotkeyStatus,
  formatActivationShortcut,
  formatGuideStatus,
  formatRuntimeStatus,
  formatWindowLifecycleStatus,
  formatDebugOverlayStatus,
  guideShortcutParts,
  highlightedLabelSegments,
  filterCatalogEntries,
  nextKeyboardSelectedCommandId,
  selectedRowScrollTop,
  nextGuideStatus,
  nextWindowLifecycleStatus,
  paletteKeyAction,
  runtimeSettingsAreDirty,
  runtimeSettingsSaveRequestFromDraft,
  applyRuntimeSettingsSaveResult,
  discardRuntimeSettingsDraft,
  isOpenSettingsCommand,
  isRefreshExtensionsCommand,
  nextSelectedCommandId,
  openSettingsCommandRow,
  openSettingsFromPalette,
  paletteRowsWithFixedActions,
  refreshExtensionsCommandRow,
  refreshExtensionsFromPalette,
  removeExtensionSettingListEntry,
  applyExtensionSettingsSaveResult,
  shouldStartGuideForCommand,
  shouldHidePaletteForWindowBlur,
  shouldRefreshCommandsForWindowLifecycleEvent,
  updateExtensionSettingListEntry,
  updateExtensionSettingToggle,
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

const defaultActivationShortcut: ActivationShortcut = {
  control: true,
  shift: true,
  alt: false,
  win: false,
  key: "KeyP",
  display_text: "Ctrl+Shift+P",
};

const ctrlAltSpaceShortcut: ActivationShortcut = {
  control: true,
  shift: false,
  alt: true,
  win: false,
  key: "Space",
  display_text: "Ctrl+Alt+Space",
};

const runtimeSettings: RuntimeSettings = {
  activation_hint: "Ctrl+Shift+P",
  activation_shortcut: defaultActivationShortcut,
  command_behavior: "execute",
  appearance_theme: "system",
  github: {
    owner: "Greg-Lim",
    repo: "omni-palette-desktop",
    branch: "main",
    catalog_path: "extensions/registry/catalog.v1.json",
    enabled: false,
  },
};

const runtimeStatus: RuntimeStatus = {
  config_path: "C:/Users/example/AppData/Roaming/OmniPalette/config.toml",
  config_error: null,
  activation_hint: "Ctrl+Shift+P",
  command_behavior: "execute",
  application_count: 4,
  ignored_process_count: 0,
  plugin_count: 3,
  plugin_application_count: 3,
};

const debugOverlayStatus: DebugOverlayStatus = {
  status: "succeeded",
  message: "Debug window shown",
  visible: true,
  show_count: 1,
  hide_count: 0,
  focus_count: 1,
  last_error: null,
};

const debugSnapshot: DebugSnapshot = {
  foreground_window: {
    process_name: "notepad.exe",
    hwnd: 42,
  },
  background_windows: [
    {
      process_name: "explorer.exe",
      hwnd: 100,
    },
  ],
  background_total: 1,
  active_tags: ["ui.text_input"],
  text_input_active: true,
  ignored_process_name: null,
  command_summary: {
    total: 2,
    focused: 1,
    background: 0,
    global: 1,
    favorites: 1,
    suppressed_priority: 0,
    low_priority: 0,
    medium_priority: 1,
    high_priority: 1,
  },
  palette_state: {
    query: "date",
    filtered_count: 2,
    top_rows: [
      {
        label: "DateTime Typer: Print date short",
        focus_state: "global",
        priority: "medium",
        favorite: false,
        score: 14,
        tags: ["datetime"],
      },
    ],
  },
};

const extensionsBootstrap: ExtensionsBootstrap = {
  bundled_extensions: [
    {
      id: "auto_typer",
      source_id: "bundled",
      name: "Auto Typer",
      version: "0.1.0",
      kind: "static",
      enabled: true,
      can_uninstall: false,
      has_settings: false,
    },
    {
      id: "ahk_agent",
      source_id: "bundled",
      name: "AHK",
      version: "0.1.0",
      kind: "wasm_plugin",
      enabled: false,
      can_uninstall: false,
      has_settings: true,
    },
  ],
  downloaded_extensions: [],
  install_root: "C:/Users/example/AppData/Roaming/OmniPalette/extensions",
  install_root_error: null,
  runtime_status: runtimeStatus,
};

const catalogEntries: CatalogEntry[] = [
  {
    id: "chrome",
    name: "Chrome",
    version: "0.1.0",
    platform: "windows",
    kind: "static",
    description: "Chrome keyboard shortcut command pack.",
    keywords: ["browser", "tabs"],
  },
  {
    id: "file_explorer",
    name: "File Explorer",
    version: "0.1.0",
    platform: "windows",
    kind: "static",
    description: "File Explorer keyboard shortcut command pack.",
    keywords: ["files"],
  },
];

const extensionSettingsBootstrap: ExtensionSettingsBootstrap = {
  status: "succeeded",
  message: "Loaded settings for AHK",
  target: {
    extension_id: "ahk_agent",
    source_id: "bundled",
    display_name: "AHK",
    kind: "wasm_plugin",
  },
  schema: {
    categories: [
      {
        key: "scripts",
        label: "Scripts",
        description: "Discovered AutoHotkey scripts",
        toggle_key: "scripts.enabled",
        default_collapsed: false,
      },
    ],
    items: [
      {
        key: "scripts.enabled",
        label: "Enable scripts",
        description: "Expose commands from scripts",
        category: "scripts",
        kind: "toggle",
        default: true,
        default_entries: [],
        entry_list_format_hint: null,
        entry_list_default_format: null,
      },
      {
        key: "scripts.entries",
        label: "Scripts",
        description: null,
        category: "scripts",
        kind: "entry_list",
        default: false,
        default_entries: [
          {
            id: "script_1",
            name: "Main",
            format: "main.ahk",
            enabled: true,
          },
        ],
        entry_list_format_hint: "Path",
        entry_list_default_format: "script.ahk",
      },
    ],
  },
  values: {
    toggles: {
      "scripts.enabled": true,
    },
    lists: {
      "scripts.entries": [
        {
          id: "script_1",
          name: "Main",
          format: "main.ahk",
          enabled: true,
        },
      ],
    },
  },
  runtime_status: runtimeStatus,
};

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

  it("calls the backend settings commands and preserves payloads", async () => {
    const runtimeStatus: RuntimeStatus = {
      config_path: "C:/Users/example/AppData/Roaming/OmniPalette/config.toml",
      config_error: null,
      activation_hint: "Ctrl+Shift+P",
      command_behavior: "execute",
      application_count: 4,
      ignored_process_count: 1,
      plugin_count: 1,
      plugin_application_count: 1,
    };
    const saveRequest: RuntimeSettingsSaveRequest = runtimeSettingsSaveRequestFromDraft({
      ...runtimeSettings,
      command_behavior: "guide",
      appearance_theme: "dark",
    });
    const saveResult: RuntimeSettingsSaveResult = {
      status: "succeeded",
      message: "Settings saved",
      config: { ...runtimeSettings, command_behavior: "guide", appearance_theme: "dark" },
      runtime_status: { ...runtimeStatus, command_behavior: "guide" },
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      if (command === "get_settings_bootstrap") {
        return {
          config: runtimeSettings,
          default_activation_shortcut: defaultActivationShortcut,
          config_path: runtimeStatus.config_path,
          config_error: null,
          runtime_status: runtimeStatus,
        } as T;
      }
      if (command === "save_runtime_settings") {
        return saveResult as T;
      }
      return {
        status: "succeeded",
        message: "Reloaded extensions: 4 applications, 1 ignored processes",
        runtime_status: runtimeStatus,
      } as T;
    });

    expect(await api.getSettingsBootstrap()).toEqual({
      config: runtimeSettings,
      default_activation_shortcut: defaultActivationShortcut,
      config_path: runtimeStatus.config_path,
      config_error: null,
      runtime_status: runtimeStatus,
    });
    expect(await api.saveRuntimeSettings(saveRequest)).toEqual(saveResult);
    expect(await api.reloadRuntimeState()).toEqual({
      status: "succeeded",
      message: "Reloaded extensions: 4 applications, 1 ignored processes",
      runtime_status: runtimeStatus,
    });
    expect(calls).toEqual([
      { command: "get_settings_bootstrap", args: undefined },
      { command: "save_runtime_settings", args: { request: saveRequest } },
      { command: "reload_runtime_state", args: undefined },
    ]);
  });

  it("calls the backend settings window command and preserves payload", async () => {
    const settingsWindowStatus: SettingsWindowStatus = {
      status: "succeeded",
      message: "Settings window shown",
      visible: true,
      show_count: 1,
      focus_count: 1,
      last_error: null,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return settingsWindowStatus as T;
    });

    const status = await api.showSettingsWindow();

    expect(calls).toEqual([{ command: "show_settings_window", args: undefined }]);
    expect(status).toEqual(settingsWindowStatus);
  });

  it("calls the backend debug overlay commands and preserves payloads", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      if (command === "get_debug_snapshot") {
        return debugSnapshot as T;
      }
      return debugOverlayStatus as T;
    });

    expect(await api.showDebugOverlay()).toEqual(debugOverlayStatus);
    expect(await api.closeDebugOverlay()).toEqual(debugOverlayStatus);
    expect(await api.getDebugOverlayStatus()).toEqual(debugOverlayStatus);
    expect(await api.getDebugSnapshot()).toEqual(debugSnapshot);
    expect(calls).toEqual([
      { command: "show_debug_overlay", args: undefined },
      { command: "close_debug_overlay", args: undefined },
      { command: "get_debug_overlay_status", args: undefined },
      { command: "get_debug_snapshot", args: undefined },
    ]);
  });

  it("calls the backend extension management commands and preserves payloads", async () => {
    const enabledResult: ExtensionMutationResult = {
      status: "succeeded",
      message: "Disabled Auto Typer",
      extensions: {
        ...extensionsBootstrap,
        bundled_extensions: [
          { ...extensionsBootstrap.bundled_extensions[0], enabled: false },
          extensionsBootstrap.bundled_extensions[1],
        ],
      },
      runtime_status: runtimeStatus,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      if (command === "get_extensions_bootstrap") {
        return extensionsBootstrap as T;
      }
      return enabledResult as T;
    });
    const enableRequest = {
      extension_id: "auto_typer",
      source_id: "bundled",
      enabled: false,
    };
    const uninstallRequest = {
      extension_id: "chrome",
      source_id: "github",
    };

    expect(await api.getExtensionsBootstrap()).toEqual(extensionsBootstrap);
    expect(await api.setExtensionEnabled(enableRequest)).toEqual(enabledResult);
    expect(await api.uninstallExtension(uninstallRequest)).toEqual(enabledResult);
    expect(calls).toEqual([
      { command: "get_extensions_bootstrap", args: undefined },
      { command: "set_extension_enabled", args: { request: enableRequest } },
      { command: "uninstall_extension", args: { request: uninstallRequest } },
    ]);
  });

  it("calls the backend marketplace commands and preserves payloads", async () => {
    const refreshResult: CatalogRefreshResult = {
      status: "succeeded",
      message: "Catalog refreshed: 2 extensions available",
      entries: catalogEntries,
      runtime_status: runtimeStatus,
    };
    const installResult: ExtensionMutationResult = {
      status: "succeeded",
      message: "Installed Chrome v0.1.0",
      extensions: {
        ...extensionsBootstrap,
        downloaded_extensions: [
          {
            id: "chrome",
            source_id: "github",
            name: "chrome",
            version: "0.1.0",
            kind: "static",
            enabled: true,
            can_uninstall: true,
            has_settings: false,
          },
        ],
      },
      runtime_status: runtimeStatus,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return (command === "refresh_extension_catalog" ? refreshResult : installResult) as T;
    });

    expect(await api.refreshExtensionCatalog(runtimeSettings.github)).toEqual(refreshResult);
    expect(await api.installCatalogExtension("chrome")).toEqual(installResult);
    expect(calls).toEqual([
      {
        command: "refresh_extension_catalog",
        args: { source: runtimeSettings.github },
      },
      { command: "install_catalog_extension", args: { extensionId: "chrome" } },
    ]);
  });

  it("calls the backend extension settings commands and preserves payloads", async () => {
    const saveResult: ExtensionSettingsSaveResult = {
      status: "succeeded",
      message: "Saved settings for AHK",
      target: extensionSettingsBootstrap.target,
      values: extensionSettingsBootstrap.values,
      runtime_status: runtimeStatus,
    };
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const api = createPaletteApi(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      return (command === "get_extension_settings"
        ? extensionSettingsBootstrap
        : saveResult) as T;
    });
    const target = {
      extension_id: "ahk_agent",
      source_id: "bundled",
    };
    const saveRequest = {
      target,
      values: extensionSettingsBootstrap.values,
    };

    expect(await api.getExtensionSettings(target)).toEqual(extensionSettingsBootstrap);
    expect(await api.saveExtensionSettings(saveRequest)).toEqual(saveResult);
    expect(calls).toEqual([
      { command: "get_extension_settings", args: { request: target } },
      { command: "save_extension_settings", args: { request: saveRequest } },
    ]);
  });
});

describe("extension settings helpers", () => {
  it("builds default values from toggle and entry-list schema items", () => {
    const defaults = defaultExtensionSettingsValues(extensionSettingsBootstrap.schema!);

    expect(defaults).toEqual(extensionSettingsBootstrap.values);
  });

  it("detects dirty extension settings by comparing resolved values", () => {
    const draft = updateExtensionSettingToggle(
      extensionSettingsBootstrap.values,
      "scripts.enabled",
      false,
    );

    expect(extensionSettingsAreDirty(extensionSettingsBootstrap.values, extensionSettingsBootstrap.values)).toBe(false);
    expect(extensionSettingsAreDirty(extensionSettingsBootstrap.values, draft)).toBe(true);
  });

  it("groups extension setting items by general and declared categories", () => {
    const schema = {
      ...extensionSettingsBootstrap.schema!,
      items: [
        {
          ...(extensionSettingsBootstrap.schema!.items[0] as ExtensionSettingItem),
          key: "loose.enabled",
          category: null,
        },
        ...extensionSettingsBootstrap.schema!.items,
      ],
    };

    const sections = extensionSettingsSections(schema);

    expect(sections.map((section) => section.category.label)).toEqual(["General", "Scripts"]);
    expect(sections[0].items[0].key).toBe("loose.enabled");
    expect(sections[1].items.map((item) => item.key)).toEqual([
      "scripts.enabled",
      "scripts.entries",
    ]);
  });

  it("updates entry-list settings without mutating the prior draft", () => {
    const item = extensionSettingsBootstrap.schema!.items.find(
      (candidate) => candidate.key === "scripts.entries",
    )!;

    const withAdded = addExtensionSettingListEntry(extensionSettingsBootstrap.values, item);
    const withUpdated = updateExtensionSettingListEntry(withAdded, item.key, 1, {
      name: "Second",
      format: "second.ahk",
      enabled: false,
    });
    const withRemoved = removeExtensionSettingListEntry(withUpdated, item.key, 0);

    expect(extensionSettingsBootstrap.values.lists[item.key]).toHaveLength(1);
    expect(withAdded.lists[item.key][1]).toMatchObject({
      id: "custom_2",
      name: "Entry 2",
      format: "script.ahk",
      enabled: true,
    });
    expect(withUpdated.lists[item.key][1]).toMatchObject({
      name: "Second",
      format: "second.ahk",
      enabled: false,
    });
    expect(withRemoved.lists[item.key]).toHaveLength(1);
    expect(withRemoved.lists[item.key][0].id).toBe("custom_2");
  });

  it("creates save requests and applies save success or failure", () => {
    const target = extensionSettingsBootstrap.target!;
    const draft = updateExtensionSettingToggle(
      extensionSettingsBootstrap.values,
      "scripts.enabled",
      false,
    );
    const saveRequest = extensionSettingsSaveRequestFromDraft(target, draft);
    const success = applyExtensionSettingsSaveResult(extensionSettingsBootstrap.values, draft, {
      status: "succeeded",
      message: "Saved settings for AHK",
      target,
      values: draft,
      runtime_status: runtimeStatus,
    });
    const failure = applyExtensionSettingsSaveResult(extensionSettingsBootstrap.values, draft, {
      status: "failed",
      message: "Could not save",
      target,
      values: extensionSettingsBootstrap.values,
      runtime_status: runtimeStatus,
    });

    expect(saveRequest).toEqual({
      target: {
        extension_id: "ahk_agent",
        source_id: "bundled",
      },
      values: draft,
    });
    expect(success).toEqual({
      saved: draft,
      draft,
      message: "Saved settings for AHK",
      failed: false,
    });
    expect(failure).toEqual({
      saved: extensionSettingsBootstrap.values,
      draft,
      message: "Could not save",
      failed: true,
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
  it("selects the first row after search results refresh", () => {
    expect(nextSelectedCommandId("chrome-new-tab", rows)).toBe("reload-extensions");
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

describe("selectedRowScrollTop", () => {
  it("places the selected row about one third from the top", () => {
    expect(
      selectedRowScrollTop({
        currentScrollTop: 0,
        containerHeight: 300,
        scrollHeight: 1000,
        rowTop: 360,
        rowHeight: 48,
      }),
    ).toBe(284);
  });

  it("clamps the scroll target at the top and bottom", () => {
    expect(
      selectedRowScrollTop({
        currentScrollTop: 120,
        containerHeight: 300,
        scrollHeight: 1000,
        rowTop: 20,
        rowHeight: 48,
      }),
    ).toBe(0);
    expect(
      selectedRowScrollTop({
        currentScrollTop: 120,
        containerHeight: 300,
        scrollHeight: 1000,
        rowTop: 960,
        rowHeight: 48,
      }),
    ).toBe(700);
  });

  it("preserves scroll when there is no scrollable overflow", () => {
    expect(
      selectedRowScrollTop({
        currentScrollTop: 24,
        containerHeight: 300,
        scrollHeight: 260,
        rowTop: 120,
        rowHeight: 48,
      }),
    ).toBe(24);
  });
});

describe("palette fixed actions", () => {
  it("adds stable refresh and open-settings rows after backend commands", () => {
    const withFixedActions = paletteRowsWithFixedActions(rows);

    expect(withFixedActions.map((row) => row.id)).toEqual([
      "reload-extensions",
      "chrome-new-tab",
      REFRESH_EXTENSIONS_COMMAND_ID,
      OPEN_SETTINGS_COMMAND_ID,
    ]);
    expect(refreshExtensionsCommandRow()).toMatchObject({
      id: REFRESH_EXTENSIONS_COMMAND_ID,
      label: "Refresh extensions",
      guide_hint: null,
      tags: ["extensions", "reload"],
    });
    expect(openSettingsCommandRow()).toMatchObject({
      id: OPEN_SETTINGS_COMMAND_ID,
      label: "Open settings for Omni Palette",
      guide_hint: null,
    });
    expect(isRefreshExtensionsCommand(REFRESH_EXTENSIONS_COMMAND_ID)).toBe(true);
    expect(isRefreshExtensionsCommand("reload-extensions")).toBe(false);
    expect(isOpenSettingsCommand(OPEN_SETTINGS_COMMAND_ID)).toBe(true);
    expect(isOpenSettingsCommand("chrome-new-tab")).toBe(false);
  });

  it("refreshes extensions through the runtime reload invoke and preserves the payload", async () => {
    const reloadResult = {
      status: "succeeded" as const,
      message: "Reloaded extensions: 4 applications, 0 ignored processes, 3 plugins",
      runtime_status: runtimeStatus,
    };
    const calls: string[] = [];
    const api = {
      reloadRuntimeState: async () => {
        calls.push("reload");
        return reloadResult;
      },
    };

    const result = await refreshExtensionsFromPalette(api);

    expect(calls).toEqual(["reload"]);
    expect(result).toEqual(reloadResult);
  });

  it("opens settings from the palette only after hiding the palette", async () => {
    const windowStatus: WindowLifecycleStatus = {
      visible: false,
      show_count: 1,
      hide_count: 1,
      focus_count: 1,
      position_count: 1,
      last_action: "hidden",
      last_error: null,
    };
    const settingsStatus: SettingsWindowStatus = {
      status: "succeeded",
      message: "Settings window shown",
      visible: true,
      show_count: 1,
      focus_count: 1,
      last_error: null,
    };
    const calls: string[] = [];
    const api = {
      hidePaletteWindow: async () => {
        calls.push("hide");
        return windowStatus;
      },
      showSettingsWindow: async () => {
        calls.push("settings");
        return settingsStatus;
      },
    };

    const result = await openSettingsFromPalette(api);

    expect(calls).toEqual(["hide", "settings"]);
    expect(result).toEqual({
      window_status: windowStatus,
      settings_status: settingsStatus,
    });
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

describe("debug overlay helpers", () => {
  it("formats debug overlay status with counts or the last error", () => {
    expect(formatDebugOverlayStatus(debugOverlayStatus)).toBe(
      "debug visible - 1 shown - 0 hidden",
    );
    expect(
      formatDebugOverlayStatus({
        ...debugOverlayStatus,
        status: "failed",
        last_error: "show failed",
      }),
    ).toBe("debug error - show failed");
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

describe("runtime settings helpers", () => {
  it("detects dirty editable settings including activation shortcut changes", () => {
    expect(runtimeSettingsAreDirty(runtimeSettings, runtimeSettings)).toBe(false);
    expect(
      runtimeSettingsAreDirty(runtimeSettings, {
        ...runtimeSettings,
        activation_hint: "Ctrl+Space",
        activation_shortcut: ctrlAltSpaceShortcut,
      }),
    ).toBe(true);
    expect(
      runtimeSettingsAreDirty(runtimeSettings, {
        ...runtimeSettings,
        command_behavior: "guide",
      }),
    ).toBe(true);
  });

  it("builds save requests with activation shortcut fields", () => {
    expect(runtimeSettingsSaveRequestFromDraft(runtimeSettings)).toEqual({
      activation_shortcut: defaultActivationShortcut,
      command_behavior: "execute",
      appearance_theme: "system",
      github: runtimeSettings.github,
    });
    expect(runtimeSettingsSaveRequestFromDraft(runtimeSettings)).not.toHaveProperty(
      "activation_hint",
    );
  });

  it("formats activation shortcuts from structured DTOs", () => {
    expect(formatActivationShortcut(defaultActivationShortcut)).toBe("Ctrl+Shift+P");
    expect(formatActivationShortcut(ctrlAltSpaceShortcut)).toBe("Ctrl+Alt+Space");
  });

  it("captures supported browser keyboard events as activation shortcuts", () => {
    expect(
      activationShortcutFromKeyboardEvent({
        code: "KeyP",
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        metaKey: false,
      }),
    ).toEqual(defaultActivationShortcut);
    expect(
      activationShortcutFromKeyboardEvent({
        code: "Digit1",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: true,
      }),
    ).toEqual({
      control: true,
      shift: false,
      alt: false,
      win: true,
      key: "Key1",
      display_text: "Ctrl+Win+1",
    });
    expect(
      activationShortcutFromKeyboardEvent({
        code: "Space",
        ctrlKey: true,
        shiftKey: false,
        altKey: true,
        metaKey: false,
      }),
    ).toEqual(ctrlAltSpaceShortcut);
    expect(
      activationShortcutFromKeyboardEvent({
        code: "Escape",
        ctrlKey: false,
        shiftKey: false,
        altKey: true,
        metaKey: false,
      }),
    ).toEqual({
      control: false,
      shift: false,
      alt: true,
      win: false,
      key: "Escape",
      display_text: "Alt+Esc",
    });
    expect(
      activationShortcutFromKeyboardEvent({
        code: "ArrowLeft",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
      }),
    ).toEqual({
      control: true,
      shift: false,
      alt: false,
      win: false,
      key: "LeftArrow",
      display_text: "Ctrl+Left",
    });
  });

  it("ignores modifier-only and unsupported keys while recording activation shortcuts", () => {
    expect(
      activationShortcutFromKeyboardEvent({
        code: "ControlLeft",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
      }),
    ).toBeNull();
    expect(
      activationShortcutFromKeyboardEvent({
        code: "AudioVolumeUp",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
      }),
    ).toBeNull();
  });

  it("updates saved and draft settings only after successful save", () => {
    const success: RuntimeSettingsSaveResult = {
      status: "succeeded",
      message: "Settings saved",
      config: { ...runtimeSettings, command_behavior: "guide" },
      runtime_status: {
        config_path: null,
        config_error: null,
        activation_hint: "Ctrl+Shift+P",
        command_behavior: "guide",
        application_count: 0,
        ignored_process_count: 0,
        plugin_count: 0,
        plugin_application_count: 0,
      },
    };
    const failure: RuntimeSettingsSaveResult = {
      ...success,
      status: "failed",
      message: "APPDATA is not set",
      config: runtimeSettings,
    };
    const dirtyDraft = { ...runtimeSettings, command_behavior: "guide" as const };

    expect(applyRuntimeSettingsSaveResult(runtimeSettings, dirtyDraft, success)).toEqual({
      saved: success.config,
      draft: success.config,
      message: "Settings saved",
      failed: false,
    });
    expect(applyRuntimeSettingsSaveResult(runtimeSettings, dirtyDraft, failure)).toEqual({
      saved: runtimeSettings,
      draft: dirtyDraft,
      message: "APPDATA is not set",
      failed: true,
    });
  });

  it("discards runtime settings draft back to saved values", () => {
    expect(discardRuntimeSettingsDraft(runtimeSettings)).toEqual(runtimeSettings);
  });
});

describe("extension management helpers", () => {
  it("applies successful extension mutations and preserves rows after failures", () => {
    const success: ExtensionMutationResult = {
      status: "succeeded",
      message: "Disabled Auto Typer",
      extensions: {
        ...extensionsBootstrap,
        bundled_extensions: [
          { ...extensionsBootstrap.bundled_extensions[0], enabled: false },
          extensionsBootstrap.bundled_extensions[1],
        ],
      },
      runtime_status: runtimeStatus,
    };
    const failure: ExtensionMutationResult = {
      ...success,
      status: "failed",
      message: "Bundled extensions can be disabled, but not uninstalled.",
    };

    expect(applyExtensionMutationResult(extensionsBootstrap, success)).toEqual({
      extensions: success.extensions,
      message: "Disabled Auto Typer",
      failed: false,
    });
    expect(applyExtensionMutationResult(extensionsBootstrap, failure)).toEqual({
      extensions: extensionsBootstrap,
      message: "Bundled extensions can be disabled, but not uninstalled.",
      failed: true,
    });
  });
});

describe("marketplace helpers", () => {
  it("filters catalog entries by name, id, description, and keywords", () => {
    expect(filterCatalogEntries(catalogEntries, "")).toEqual(catalogEntries);
    expect(filterCatalogEntries(catalogEntries, "chrome")).toEqual([catalogEntries[0]]);
    expect(filterCatalogEntries(catalogEntries, "file_explorer")).toEqual([
      catalogEntries[1],
    ]);
    expect(filterCatalogEntries(catalogEntries, "keyboard shortcut")).toEqual(
      catalogEntries,
    );
    expect(filterCatalogEntries(catalogEntries, "tabs")).toEqual([catalogEntries[0]]);
    expect(filterCatalogEntries(catalogEntries, "missing")).toEqual([]);
  });

  it("applies catalog refresh success and preserves entries after failures", () => {
    const success: CatalogRefreshResult = {
      status: "succeeded",
      message: "Catalog refreshed: 2 extensions available",
      entries: catalogEntries,
      runtime_status: runtimeStatus,
    };
    const failure: CatalogRefreshResult = {
      status: "failed",
      message: "network down",
      entries: [],
      runtime_status: runtimeStatus,
    };

    expect(applyCatalogRefreshResult([], success)).toEqual({
      entries: catalogEntries,
      message: "Catalog refreshed: 2 extensions available",
      failed: false,
    });
    expect(applyCatalogRefreshResult(catalogEntries, failure)).toEqual({
      entries: catalogEntries,
      message: "network down",
      failed: true,
    });
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
