import { invoke } from "@tauri-apps/api/core";

export type CommandFocusState = "focused" | "background" | "global";
export type CommandPriority = "suppressed" | "low" | "medium" | "high";
export type CommandBehavior = "execute" | "guide";
export const HOTKEY_EVENT_NAME = "omni://palette-activation-requested";

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
  };
}

export const paletteApi = createPaletteApi();

export function nextSelectedCommandId(currentId: string, commands: CommandRow[]): string {
  if (commands.some((command) => command.id === currentId)) {
    return currentId;
  }

  return commands[0]?.id ?? "";
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
