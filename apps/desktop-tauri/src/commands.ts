import { invoke } from "@tauri-apps/api/core";

export type CommandFocusState = "focused" | "background" | "global";
export type CommandPriority = "suppressed" | "low" | "medium" | "high";
export type CommandBehavior = "execute" | "guide";
export const HOTKEY_EVENT_NAME = "omni://palette-activation-requested";
export const WINDOW_LIFECYCLE_EVENT_NAME = "omni://palette-window-lifecycle";

export type MatchRange = {
  start: number;
  end: number;
};

export type CommandRow = {
  id: string;
  label: string;
  shortcut_text: string;
  focus_state: CommandFocusState;
  priority: CommandPriority;
  favorite: boolean;
  tags: string[];
  original_order: number;
  score: number;
  label_matches: MatchRange[];
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
  };
}

export const paletteApi = createPaletteApi();

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

export function shouldRefreshCommandsForWindowLifecycleEvent(
  event: WindowLifecycleEventPayload,
): boolean {
  return event.action === "shown";
}
