import { invoke } from "@tauri-apps/api/core";

export type CommandFocusState = "focused" | "background" | "global";
export type CommandPriority = "suppressed" | "low" | "medium" | "high";

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

export type PaletteBootstrap = {
  session_id: string;
  backend_status: string;
  commands: CommandRow[];
};

export type CommandExecutionStatus = "succeeded" | "failed" | "deferred";

export type CommandExecutionResult = {
  status: CommandExecutionStatus;
  message: string;
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
  };
}

export const paletteApi = createPaletteApi();

export function nextSelectedCommandId(currentId: string, commands: CommandRow[]): string {
  if (commands.some((command) => command.id === currentId)) {
    return currentId;
  }

  return commands[0]?.id ?? "";
}
