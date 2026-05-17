import { invoke } from "@tauri-apps/api/core";

export type CommandFocusState = "focused" | "background" | "global";
export type CommandPriority = "suppressed" | "low" | "medium" | "high";
export type CommandBehavior = "execute" | "guide";
export const HOTKEY_EVENT_NAME = "omni://palette-activation-requested";
export const WINDOW_LIFECYCLE_EVENT_NAME = "omni://palette-window-lifecycle";
export const GUIDE_EVENT_NAME = "omni://palette-guide";

export type MatchRange = {
  start: number;
  end: number;
};

export type CommandRow = {
  id: string;
  label: string;
  shortcut_text: string;
  guide_hint: GuideHint | null;
  focus_state: CommandFocusState;
  priority: CommandPriority;
  favorite: boolean;
  tags: string[];
  original_order: number;
  score: number;
  label_matches: MatchRange[];
};

export type GuideHint = {
  shortcut_text: string;
  captures_shortcut: boolean;
};

export type HighlightedLabelSegment = {
  text: string;
  highlighted: boolean;
};

export type PaletteSnapshot = {
  session_id: string;
  query: string;
  commands: CommandRow[];
};

export type RuntimeStatus = {
  config_path: string | null;
  config_error: string | null;
  activation_hint: string;
  command_behavior: CommandBehavior;
  application_count: number;
  ignored_process_count: number;
  plugin_count: number;
  plugin_application_count: number;
};

export type AppearanceTheme = "system" | "light" | "dark";

export type GitHubCatalogSource = {
  owner: string;
  repo: string;
  branch: string;
  catalog_path: string;
  enabled: boolean;
};

export type ActivationShortcut = {
  control: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
  key: string;
  display_text: string;
};

export type RuntimeSettings = {
  activation_hint: string;
  activation_shortcut: ActivationShortcut;
  command_behavior: CommandBehavior;
  appearance_theme: AppearanceTheme;
  github: GitHubCatalogSource;
};

export type SettingsBootstrap = {
  config: RuntimeSettings;
  default_activation_shortcut: ActivationShortcut;
  config_path: string | null;
  config_error: string | null;
  runtime_status: RuntimeStatus;
};

export type RuntimeSettingsSaveRequest = {
  activation_shortcut: ActivationShortcut;
  command_behavior: CommandBehavior;
  appearance_theme: AppearanceTheme;
  github: GitHubCatalogSource;
};

export type RuntimeSettingsResultStatus = "succeeded" | "failed";

export type RuntimeSettingsSaveResult = {
  status: RuntimeSettingsResultStatus;
  message: string;
  config: RuntimeSettings;
  runtime_status: RuntimeStatus;
};

export type RuntimeReloadResult = {
  status: RuntimeSettingsResultStatus;
  message: string;
  runtime_status: RuntimeStatus;
};

export type PlatformOs = "windows" | "macos" | "linux";
export type ExtensionKind = "static" | "wasm_plugin";

export type CatalogEntry = {
  id: string;
  name: string;
  version: string;
  platform: PlatformOs;
  kind: ExtensionKind;
  description: string | null;
  keywords: string[];
};

export type CatalogRefreshResult = {
  status: RuntimeSettingsResultStatus;
  message: string;
  entries: CatalogEntry[];
  runtime_status: RuntimeStatus;
};

export type CatalogRefreshApplyResult = {
  entries: CatalogEntry[];
  message: string;
  failed: boolean;
};

export type ExtensionRow = {
  id: string;
  source_id: string;
  name: string;
  version: string;
  kind: ExtensionKind;
  enabled: boolean;
  can_uninstall: boolean;
  has_settings: boolean;
};

export type ExtensionsBootstrap = {
  bundled_extensions: ExtensionRow[];
  downloaded_extensions: ExtensionRow[];
  install_root: string | null;
  install_root_error: string | null;
  runtime_status: RuntimeStatus;
};

export type ExtensionEnabledRequest = {
  extension_id: string;
  source_id: string;
  enabled: boolean;
};

export type ExtensionTargetRequest = {
  extension_id: string;
  source_id: string;
};

export type ExtensionMutationResult = {
  status: RuntimeSettingsResultStatus;
  message: string;
  extensions: ExtensionsBootstrap;
  runtime_status: RuntimeStatus;
};

export type ExtensionMutationApplyResult = {
  extensions: ExtensionsBootstrap;
  message: string;
  failed: boolean;
};

export type ExtensionSettingsTarget = {
  extension_id: string;
  source_id: string;
  display_name: string;
  kind: ExtensionKind;
};

export type ExtensionSettingsCategory = {
  key: string;
  label: string;
  description: string | null;
  toggle_key: string | null;
  default_collapsed: boolean;
};

export type ExtensionSettingKind = "toggle" | "entry_list";

export type ExtensionSettingListEntry = {
  id: string;
  name: string;
  format: string;
  enabled: boolean;
};

export type ExtensionSettingItem = {
  key: string;
  label: string;
  description: string | null;
  category: string | null;
  kind: ExtensionSettingKind;
  default: boolean;
  default_entries: ExtensionSettingListEntry[];
  entry_list_format_hint: string | null;
  entry_list_default_format: string | null;
};

export type ExtensionSettingsSchema = {
  categories: ExtensionSettingsCategory[];
  items: ExtensionSettingItem[];
};

export type ExtensionSettingsValues = {
  toggles: Record<string, boolean>;
  lists: Record<string, ExtensionSettingListEntry[]>;
};

export type ExtensionSettingsBootstrap = {
  status: RuntimeSettingsResultStatus;
  message: string;
  target: ExtensionSettingsTarget | null;
  schema: ExtensionSettingsSchema | null;
  values: ExtensionSettingsValues;
  runtime_status: RuntimeStatus;
};

export type ExtensionSettingsSaveRequest = {
  target: ExtensionTargetRequest;
  values: ExtensionSettingsValues;
};

export type ExtensionSettingsSaveResult = {
  status: RuntimeSettingsResultStatus;
  message: string;
  target: ExtensionSettingsTarget | null;
  values: ExtensionSettingsValues;
  runtime_status: RuntimeStatus;
};

export type ExtensionSettingsApplyResult = {
  saved: ExtensionSettingsValues;
  draft: ExtensionSettingsValues;
  message: string;
  failed: boolean;
};

export type ExtensionSettingsSection = {
  category: ExtensionSettingsCategory;
  items: ExtensionSettingItem[];
};

export type SettingsWindowStatus = {
  status: RuntimeSettingsResultStatus;
  message: string;
  visible: boolean;
  show_count: number;
  focus_count: number;
  last_error: string | null;
};

export type DebugOverlayStatus = {
  status: RuntimeSettingsResultStatus;
  message: string;
  visible: boolean;
  show_count: number;
  hide_count: number;
  focus_count: number;
  last_error: string | null;
};

export type DebugWindowSummary = {
  process_name: string | null;
  hwnd: number | null;
};

export type DebugCommandSummary = {
  total: number;
  focused: number;
  background: number;
  global: number;
  favorites: number;
  suppressed_priority: number;
  low_priority: number;
  medium_priority: number;
  high_priority: number;
};

export type DebugCommandRow = {
  label: string;
  focus_state: CommandFocusState;
  priority: CommandPriority;
  favorite: boolean;
  score: number;
  tags: string[];
};

export type DebugPaletteState = {
  query: string;
  filtered_count: number;
  top_rows: DebugCommandRow[];
};

export type DebugSnapshot = {
  foreground_window: DebugWindowSummary | null;
  background_windows: DebugWindowSummary[];
  background_total: number;
  active_tags: string[];
  text_input_active: boolean;
  ignored_process_name: string | null;
  command_summary: DebugCommandSummary;
  palette_state: DebugPaletteState;
};

export type OpenSettingsFromPaletteResult = {
  window_status: WindowLifecycleStatus;
  settings_status: SettingsWindowStatus;
};

export type RuntimeSettingsApplyResult = {
  saved: RuntimeSettings;
  draft: RuntimeSettings;
  message: string;
  failed: boolean;
};

export type PaletteBootstrap = {
  session_id: string;
  backend_status: string;
  runtime_status: RuntimeStatus;
  commands: CommandRow[];
};

export type CommandExecutionStatus = "succeeded" | "failed" | "deferred";

export type CommandExecutionResult = {
  status: CommandExecutionStatus;
  message: string;
};

export type HotkeyEventKind =
  | "activation_requested"
  | "ignored_passthrough"
  | "listener_error";

export type HotkeyEventPayload = {
  kind: HotkeyEventKind;
  shortcut: string;
  process_name: string | null;
  activation_count: number;
  ignored_passthrough_count: number;
  message: string | null;
};

export type HotkeyStatus = {
  running: boolean;
  activation_hint: string;
  activation_count: number;
  ignored_passthrough_count: number;
  last_event: HotkeyEventPayload | null;
  last_error: string | null;
};

export type WindowLifecycleAction = "shown" | "hidden" | "error";

export type WindowLifecycleEventPayload = {
  action: WindowLifecycleAction;
  visible: boolean;
  show_count: number;
  hide_count: number;
  focus_count: number;
  position_count: number;
  message: string | null;
};

export type WindowLifecycleStatus = {
  visible: boolean;
  show_count: number;
  hide_count: number;
  focus_count: number;
  position_count: number;
  last_action: WindowLifecycleAction | null;
  last_error: string | null;
};

export type GuideAction = "started" | "completed" | "cancelled" | "expired" | "error";

export type GuideStatus = {
  active: boolean;
  command_label: string | null;
  shortcut_text: string | null;
  activation_hint: string;
  start_count: number;
  complete_count: number;
  cancel_count: number;
  expire_count: number;
  last_action: GuideAction | null;
  last_error: string | null;
};

export type GuideEventPayload = {
  action: GuideAction;
  active: boolean;
  command_label: string | null;
  shortcut_text: string | null;
  activation_hint: string;
  start_count: number;
  complete_count: number;
  cancel_count: number;
  expire_count: number;
  message: string | null;
};

export type PaletteKeyAction = "select_next" | "select_previous" | "execute" | "hide";

export type PaletteInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createPaletteApi(invokeCommand: PaletteInvoke = invoke) {
  return {
    getPaletteBootstrap: () => invokeCommand<PaletteBootstrap>("get_palette_bootstrap"),
    searchCommands: (query: string) =>
      invokeCommand<PaletteSnapshot>("search_commands", { query }),
    executeCommand: (commandId: string) =>
      invokeCommand<CommandExecutionResult>("execute_command", { commandId }),
    getHotkeyStatus: () => invokeCommand<HotkeyStatus>("get_hotkey_status"),
    getWindowLifecycleStatus: () =>
      invokeCommand<WindowLifecycleStatus>("get_window_lifecycle_status"),
    hidePaletteWindow: () => invokeCommand<WindowLifecycleStatus>("hide_palette_window"),
    startGuide: (commandId: string) =>
      invokeCommand<GuideStatus>("start_guide", { commandId }),
    cancelGuide: () => invokeCommand<GuideStatus>("cancel_guide"),
    getGuideStatus: () => invokeCommand<GuideStatus>("get_guide_status"),
    getSettingsBootstrap: () => invokeCommand<SettingsBootstrap>("get_settings_bootstrap"),
    saveRuntimeSettings: (request: RuntimeSettingsSaveRequest) =>
      invokeCommand<RuntimeSettingsSaveResult>("save_runtime_settings", { request }),
    reloadRuntimeState: () => invokeCommand<RuntimeReloadResult>("reload_runtime_state"),
    getExtensionsBootstrap: () =>
      invokeCommand<ExtensionsBootstrap>("get_extensions_bootstrap"),
    setExtensionEnabled: (request: ExtensionEnabledRequest) =>
      invokeCommand<ExtensionMutationResult>("set_extension_enabled", { request }),
    uninstallExtension: (request: ExtensionTargetRequest) =>
      invokeCommand<ExtensionMutationResult>("uninstall_extension", { request }),
    refreshExtensionCatalog: (source: GitHubCatalogSource) =>
      invokeCommand<CatalogRefreshResult>("refresh_extension_catalog", { source }),
    installCatalogExtension: (extensionId: string) =>
      invokeCommand<ExtensionMutationResult>("install_catalog_extension", { extensionId }),
    getExtensionSettings: (request: ExtensionTargetRequest) =>
      invokeCommand<ExtensionSettingsBootstrap>("get_extension_settings", { request }),
    saveExtensionSettings: (request: ExtensionSettingsSaveRequest) =>
      invokeCommand<ExtensionSettingsSaveResult>("save_extension_settings", { request }),
    showSettingsWindow: () => invokeCommand<SettingsWindowStatus>("show_settings_window"),
    showDebugOverlay: () => invokeCommand<DebugOverlayStatus>("show_debug_overlay"),
    closeDebugOverlay: () => invokeCommand<DebugOverlayStatus>("close_debug_overlay"),
    getDebugOverlayStatus: () =>
      invokeCommand<DebugOverlayStatus>("get_debug_overlay_status"),
    getDebugSnapshot: () => invokeCommand<DebugSnapshot>("get_debug_snapshot"),
  };
}

export const paletteApi = createPaletteApi();
export const REFRESH_EXTENSIONS_COMMAND_ID = "fixed-refresh-extensions";
export const OPEN_SETTINGS_COMMAND_ID = "open-settings";

export function refreshExtensionsCommandRow(): CommandRow {
  return {
    id: REFRESH_EXTENSIONS_COMMAND_ID,
    label: "Refresh extensions",
    shortcut_text: "",
    guide_hint: null,
    focus_state: "global",
    priority: "medium",
    favorite: false,
    tags: ["extensions", "reload"],
    original_order: Number.MAX_SAFE_INTEGER - 1,
    score: 0,
    label_matches: [],
  };
}

export function openSettingsCommandRow(): CommandRow {
  return {
    id: OPEN_SETTINGS_COMMAND_ID,
    label: "Open settings for Omni Palette",
    shortcut_text: "Settings",
    guide_hint: null,
    focus_state: "global",
    priority: "medium",
    favorite: false,
    tags: ["settings"],
    original_order: Number.MAX_SAFE_INTEGER,
    score: 0,
    label_matches: [],
  };
}

export function paletteRowsWithFixedActions(commands: CommandRow[]): CommandRow[] {
  return [...commands, refreshExtensionsCommandRow(), openSettingsCommandRow()];
}

export function isRefreshExtensionsCommand(commandId: string): boolean {
  return commandId === REFRESH_EXTENSIONS_COMMAND_ID;
}

export function isOpenSettingsCommand(commandId: string): boolean {
  return commandId === OPEN_SETTINGS_COMMAND_ID;
}

export async function refreshExtensionsFromPalette(
  api: Pick<ReturnType<typeof createPaletteApi>, "reloadRuntimeState"> = paletteApi,
): Promise<RuntimeReloadResult> {
  return api.reloadRuntimeState();
}

export async function openSettingsFromPalette(
  api: Pick<ReturnType<typeof createPaletteApi>, "hidePaletteWindow" | "showSettingsWindow"> =
    paletteApi,
): Promise<OpenSettingsFromPaletteResult> {
  const windowStatus = await api.hidePaletteWindow();
  const settingsStatus = await api.showSettingsWindow();
  return {
    window_status: windowStatus,
    settings_status: settingsStatus,
  };
}

export function nextSelectedCommandId(currentId: string, commands: CommandRow[]): string {
  if (commands.some((command) => command.id === currentId)) {
    return currentId;
  }

  return commands[0]?.id ?? "";
}

export function nextKeyboardSelectedCommandId(
  currentId: string,
  commands: CommandRow[],
  delta: number,
): string {
  if (commands.length === 0) {
    return "";
  }

  const currentIndex = commands.findIndex((command) => command.id === currentId);
  if (currentIndex < 0) {
    return commands[0].id;
  }

  const nextIndex = (currentIndex + delta + commands.length) % commands.length;
  return commands[nextIndex].id;
}

export function commandExecutionShouldHidePalette(result: CommandExecutionResult): boolean {
  return result.status === "succeeded";
}

export function shouldStartGuideForCommand(
  runtimeStatus: RuntimeStatus | null,
  command: CommandRow | undefined,
): boolean {
  return runtimeStatus?.command_behavior === "guide" && command?.guide_hint != null;
}

export function paletteKeyAction(key: string): PaletteKeyAction | null {
  switch (key) {
    case "ArrowDown":
      return "select_next";
    case "ArrowUp":
      return "select_previous";
    case "Enter":
      return "execute";
    case "Escape":
      return "hide";
    default:
      return null;
  }
}

export function shouldHidePaletteForWindowBlur(
  status: WindowLifecycleStatus | null,
): boolean {
  return status?.visible === true;
}

export function runtimeSettingsSaveRequestFromDraft(
  draft: RuntimeSettings,
): RuntimeSettingsSaveRequest {
  return {
    activation_shortcut: { ...draft.activation_shortcut },
    command_behavior: draft.command_behavior,
    appearance_theme: draft.appearance_theme,
    github: { ...draft.github },
  };
}

export function runtimeSettingsAreDirty(
  saved: RuntimeSettings | null,
  draft: RuntimeSettings | null,
): boolean {
  if (!saved || !draft) {
    return false;
  }

  return (
    JSON.stringify(runtimeSettingsSaveRequestFromDraft(saved)) !==
    JSON.stringify(runtimeSettingsSaveRequestFromDraft(draft))
  );
}

export function applyRuntimeSettingsSaveResult(
  saved: RuntimeSettings,
  draft: RuntimeSettings,
  result: RuntimeSettingsSaveResult,
): RuntimeSettingsApplyResult {
  if (result.status === "succeeded") {
    const next = discardRuntimeSettingsDraft(result.config);
    return {
      saved: next,
      draft: discardRuntimeSettingsDraft(next),
      message: result.message,
      failed: false,
    };
  }

  return {
    saved,
    draft,
    message: result.message,
    failed: true,
  };
}

export function discardRuntimeSettingsDraft(saved: RuntimeSettings): RuntimeSettings {
  return {
    ...saved,
    activation_shortcut: { ...saved.activation_shortcut },
    github: { ...saved.github },
  };
}

export function applyExtensionMutationResult(
  current: ExtensionsBootstrap,
  result: ExtensionMutationResult,
): ExtensionMutationApplyResult {
  if (result.status === "succeeded") {
    return {
      extensions: result.extensions,
      message: result.message,
      failed: false,
    };
  }

  return {
    extensions: current,
    message: result.message,
    failed: true,
  };
}

export function applyCatalogRefreshResult(
  currentEntries: CatalogEntry[],
  result: CatalogRefreshResult,
): CatalogRefreshApplyResult {
  if (result.status === "succeeded") {
    return {
      entries: result.entries,
      message: result.message,
      failed: false,
    };
  }

  return {
    entries: currentEntries,
    message: result.message,
    failed: true,
  };
}

export function defaultExtensionSettingsValues(
  schema: ExtensionSettingsSchema,
): ExtensionSettingsValues {
  const values: ExtensionSettingsValues = {
    toggles: {},
    lists: {},
  };

  for (const item of schema.items) {
    if (item.kind === "toggle") {
      values.toggles[item.key] = item.default;
    } else {
      values.lists[item.key] = item.default_entries.map(copyExtensionSettingListEntry);
    }
  }

  return values;
}

export function copyExtensionSettingsValues(
  values: ExtensionSettingsValues,
): ExtensionSettingsValues {
  return {
    toggles: { ...values.toggles },
    lists: Object.fromEntries(
      Object.entries(values.lists).map(([key, entries]) => [
        key,
        entries.map(copyExtensionSettingListEntry),
      ]),
    ),
  };
}

export function extensionSettingsAreDirty(
  saved: ExtensionSettingsValues | null,
  draft: ExtensionSettingsValues | null,
): boolean {
  if (!saved || !draft) {
    return false;
  }

  return JSON.stringify(saved) !== JSON.stringify(draft);
}

export function extensionSettingsSaveRequestFromDraft(
  target: ExtensionSettingsTarget,
  draft: ExtensionSettingsValues,
): ExtensionSettingsSaveRequest {
  return {
    target: {
      extension_id: target.extension_id,
      source_id: target.source_id,
    },
    values: copyExtensionSettingsValues(draft),
  };
}

export function applyExtensionSettingsSaveResult(
  saved: ExtensionSettingsValues,
  draft: ExtensionSettingsValues,
  result: ExtensionSettingsSaveResult,
): ExtensionSettingsApplyResult {
  if (result.status === "succeeded") {
    const next = copyExtensionSettingsValues(result.values);
    return {
      saved: next,
      draft: copyExtensionSettingsValues(next),
      message: result.message,
      failed: false,
    };
  }

  return {
    saved,
    draft,
    message: result.message,
    failed: true,
  };
}

export function extensionSettingsSections(
  schema: ExtensionSettingsSchema,
): ExtensionSettingsSection[] {
  const itemsByCategory = new Map<string, ExtensionSettingItem[]>();
  const generalItems: ExtensionSettingItem[] = [];

  for (const item of schema.items) {
    if (item.category) {
      const items = itemsByCategory.get(item.category) ?? [];
      items.push(item);
      itemsByCategory.set(item.category, items);
    } else {
      generalItems.push(item);
    }
  }

  const sections: ExtensionSettingsSection[] = [];
  if (generalItems.length > 0) {
    sections.push({
      category: {
        key: "__general__",
        label: "General",
        description: null,
        toggle_key: null,
        default_collapsed: false,
      },
      items: generalItems,
    });
  }

  for (const category of schema.categories) {
    sections.push({
      category,
      items: itemsByCategory.get(category.key) ?? [],
    });
  }

  return sections;
}

export function updateExtensionSettingToggle(
  values: ExtensionSettingsValues,
  key: string,
  enabled: boolean,
): ExtensionSettingsValues {
  const next = copyExtensionSettingsValues(values);
  next.toggles[key] = enabled;
  return next;
}

export function addExtensionSettingListEntry(
  values: ExtensionSettingsValues,
  item: ExtensionSettingItem,
): ExtensionSettingsValues {
  const next = copyExtensionSettingsValues(values);
  const entries = next.lists[item.key] ?? [];
  const nextIndex = entries.length + 1;
  entries.push({
    id: `custom_${nextIndex}`,
    name: `Entry ${nextIndex}`,
    format: item.entry_list_default_format ?? "",
    enabled: true,
  });
  next.lists[item.key] = entries;
  return next;
}

export function updateExtensionSettingListEntry(
  values: ExtensionSettingsValues,
  key: string,
  index: number,
  patch: Partial<ExtensionSettingListEntry>,
): ExtensionSettingsValues {
  const next = copyExtensionSettingsValues(values);
  const entries = next.lists[key] ?? [];
  if (entries[index]) {
    entries[index] = {
      ...entries[index],
      ...patch,
    };
  }
  next.lists[key] = entries;
  return next;
}

export function removeExtensionSettingListEntry(
  values: ExtensionSettingsValues,
  key: string,
  index: number,
): ExtensionSettingsValues {
  const next = copyExtensionSettingsValues(values);
  const entries = next.lists[key] ?? [];
  next.lists[key] = entries.filter((_, entryIndex) => entryIndex !== index);
  return next;
}

function copyExtensionSettingListEntry(
  entry: ExtensionSettingListEntry,
): ExtensionSettingListEntry {
  return { ...entry };
}

export function filterCatalogEntries(
  entries: CatalogEntry[],
  query: string,
): CatalogEntry[] {
  const terms = query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  if (terms.length === 0) {
    return entries;
  }

  return entries.filter((entry) => {
    const fields = [
      entry.id,
      entry.name,
      entry.description ?? "",
      ...entry.keywords,
    ].map((field) => field.toLowerCase());
    return terms.every((term) => fields.some((field) => field.includes(term)));
  });
}

type KeyboardEventLike = Pick<
  KeyboardEvent,
  "code" | "ctrlKey" | "shiftKey" | "altKey" | "metaKey"
>;

const KEY_DISPLAY_NAMES: Record<string, string> = {
  Key0: "0",
  Key1: "1",
  Key2: "2",
  Key3: "3",
  Key4: "4",
  Key5: "5",
  Key6: "6",
  Key7: "7",
  Key8: "8",
  Key9: "9",
  Semicolon: ";",
  Equal: "=",
  Comma: ",",
  Minus: "-",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  BracketLeft: "[",
  Backslash: "\\",
  BracketRight: "]",
  Quote: "'",
  Enter: "Enter",
  Space: "Space",
  Tab: "Tab",
  Escape: "Esc",
  Delete: "Del",
  Backspace: "Backspace",
  Home: "Home",
  End: "End",
  PageUp: "PgUp",
  PageDown: "PgDn",
  Insert: "Ins",
  PrintScreen: "PrtSc",
  ScrollLock: "ScrLk",
  Pause: "Pause",
  LeftArrow: "Left",
  RightArrow: "Right",
  UpArrow: "Up",
  DownArrow: "Down",
};

const SPECIAL_KEY_BY_BROWSER_CODE: Record<string, string> = {
  Semicolon: "Semicolon",
  Equal: "Equal",
  Comma: "Comma",
  Minus: "Minus",
  Period: "Period",
  Slash: "Slash",
  Backquote: "Grave",
  BracketLeft: "LeftBracket",
  Backslash: "Backslash",
  BracketRight: "RightBracket",
  Quote: "Apostrophe",
  Enter: "Enter",
  Space: "Space",
  Tab: "Tab",
  Escape: "Escape",
  Delete: "Delete",
  Backspace: "BackSpace",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
  PrintScreen: "PrintScreen",
  ScrollLock: "ScrollLock",
  Pause: "Pause",
  ArrowLeft: "LeftArrow",
  ArrowRight: "RightArrow",
  ArrowUp: "UpArrow",
  ArrowDown: "DownArrow",
};

const KEY_DISPLAY_BY_RUNTIME_KEY: Record<string, string> = {
  ...KEY_DISPLAY_NAMES,
  Grave: "`",
  LeftBracket: "[",
  RightBracket: "]",
  Apostrophe: "'",
  BackSpace: "Backspace",
};

export function activationShortcutFromKeyboardEvent(
  event: KeyboardEventLike,
): ActivationShortcut | null {
  const key = runtimeShortcutKeyFromBrowserCode(event.code);
  if (!key) {
    return null;
  }

  const shortcut: ActivationShortcut = {
    control: event.ctrlKey,
    shift: event.shiftKey,
    alt: event.altKey,
    win: event.metaKey,
    key,
    display_text: "",
  };
  shortcut.display_text = formatActivationShortcut(shortcut);
  return shortcut;
}

export function formatActivationShortcut(shortcut: ActivationShortcut): string {
  const parts: string[] = [];
  if (shortcut.control) {
    parts.push("Ctrl");
  }
  if (shortcut.shift) {
    parts.push("Shift");
  }
  if (shortcut.alt) {
    parts.push("Alt");
  }
  if (shortcut.win) {
    parts.push("Win");
  }

  const keyDisplay = keyDisplayName(shortcut.key);
  if (!keyDisplay) {
    return shortcut.display_text || shortcut.key;
  }

  parts.push(keyDisplay);
  return parts.join("+");
}

function runtimeShortcutKeyFromBrowserCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) {
    return code;
  }

  const digitMatch = /^Digit([0-9])$/.exec(code);
  if (digitMatch) {
    return `Key${digitMatch[1]}`;
  }

  const functionKeyMatch = /^F([1-9]|1[0-2])$/.exec(code);
  if (functionKeyMatch) {
    return code;
  }

  return SPECIAL_KEY_BY_BROWSER_CODE[code] ?? null;
}

function keyDisplayName(key: string): string | null {
  if (/^Key[A-Z]$/.test(key)) {
    return key.slice(3);
  }

  return KEY_DISPLAY_BY_RUNTIME_KEY[key] ?? null;
}

export function highlightedLabelSegments(
  label: string,
  ranges: MatchRange[],
): HighlightedLabelSegment[] {
  const validRanges = ranges
    .map((range) => ({
      start: byteOffsetToStringIndex(label, range.start),
      end: byteOffsetToStringIndex(label, range.end),
    }))
    .filter(
      (range): range is { start: number; end: number } =>
        range.start !== null && range.end !== null && range.start < range.end,
    )
    .sort((left, right) => left.start - right.start || left.end - right.end);
  const segments: HighlightedLabelSegment[] = [];
  let cursor = 0;

  for (const range of validRanges) {
    if (range.start < cursor) {
      continue;
    }

    if (cursor < range.start) {
      segments.push({ text: label.slice(cursor, range.start), highlighted: false });
    }

    segments.push({ text: label.slice(range.start, range.end), highlighted: true });
    cursor = range.end;
  }

  if (cursor < label.length) {
    segments.push({ text: label.slice(cursor), highlighted: false });
  }

  return segments.length > 0 ? segments : [{ text: label, highlighted: false }];
}

function byteOffsetToStringIndex(label: string, byteOffset: number): number | null {
  if (!Number.isInteger(byteOffset) || byteOffset < 0) {
    return null;
  }

  const encoder = new TextEncoder();
  let bytes = 0;
  for (let index = 0; index < label.length; ) {
    if (bytes === byteOffset) {
      return index;
    }

    const codePoint = label.codePointAt(index);
    if (codePoint === undefined) {
      return null;
    }

    const char = String.fromCodePoint(codePoint);
    bytes += encoder.encode(char).length;
    index += char.length;

    if (bytes > byteOffset) {
      return null;
    }
  }

  return bytes === byteOffset ? label.length : null;
}

export function formatRuntimeStatus(status: RuntimeStatus): string {
  return [
    status.activation_hint,
    status.command_behavior,
    `${status.application_count} apps`,
    `${status.ignored_process_count} ignored`,
    `${status.plugin_count} plugins`,
  ].join(" - ");
}

export function formatHotkeyStatus(status: HotkeyStatus): string {
  if (status.last_error) {
    return `hotkey error - ${status.last_error}`;
  }

  return [
    status.running ? "hotkey on" : "hotkey off",
    status.activation_hint,
    `${status.activation_count} activations`,
    `${status.ignored_passthrough_count} passthrough`,
  ].join(" - ");
}

export function formatWindowLifecycleStatus(status: WindowLifecycleStatus): string {
  if (status.last_error) {
    return `window error - ${status.last_error}`;
  }

  return [
    status.visible ? "window visible" : "window hidden",
    status.last_action ?? "idle",
    `${status.show_count} shown`,
    `${status.hide_count} hidden`,
  ].join(" - ");
}

export function formatGuideStatus(status: GuideStatus): string {
  if (status.last_error) {
    return `guide error - ${status.last_error}`;
  }

  if (status.active) {
    return [
      "guide active",
      status.command_label ?? "command",
      status.shortcut_text ?? status.activation_hint,
    ].join(" - ");
  }

  return [
    "guide idle",
    status.last_action ?? "idle",
    `${status.start_count} started`,
    `${status.complete_count} completed`,
  ].join(" - ");
}

export function formatDebugOverlayStatus(status: DebugOverlayStatus): string {
  if (status.last_error) {
    return `debug error - ${status.last_error}`;
  }

  return [
    status.visible ? "debug visible" : "debug hidden",
    `${status.show_count} shown`,
    `${status.hide_count} hidden`,
  ].join(" - ");
}

export function nextWindowLifecycleStatus(
  _current: WindowLifecycleStatus | null,
  event: WindowLifecycleEventPayload,
): WindowLifecycleStatus {
  return {
    visible: event.visible,
    show_count: event.show_count,
    hide_count: event.hide_count,
    focus_count: event.focus_count,
    position_count: event.position_count,
    last_action: event.action,
    last_error: event.action === "error" ? event.message : null,
  };
}

export function nextGuideStatus(
  _current: GuideStatus | null,
  event: GuideEventPayload,
): GuideStatus {
  return {
    active: event.active,
    command_label: event.command_label,
    shortcut_text: event.shortcut_text,
    activation_hint: event.activation_hint,
    start_count: event.start_count,
    complete_count: event.complete_count,
    cancel_count: event.cancel_count,
    expire_count: event.expire_count,
    last_action: event.action,
    last_error: event.action === "error" ? event.message : null,
  };
}

export function shouldRefreshCommandsForWindowLifecycleEvent(
  event: WindowLifecycleEventPayload,
): boolean {
  return event.action === "shown";
}

export function guideShortcutParts(shortcutText: string): string[][] {
  return shortcutText
    .split(",")
    .map((chord) =>
      chord
        .split("+")
        .map((part) => part.trim())
        .filter(Boolean),
    )
    .filter((chord) => chord.length > 0);
}
