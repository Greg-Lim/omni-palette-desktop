export type CommandRow = {
  id: string;
  label: string;
  shortcut: string;
  scope: "focused" | "background" | "global";
  tags: string[];
};

export const sampleCommands: CommandRow[] = [
  {
    id: "reload-extensions",
    label: "Omni Palette: Reload extensions",
    shortcut: "",
    scope: "global",
    tags: ["extensions", "reload"],
  },
  {
    id: "chrome-new-tab",
    label: "Chrome: New tab",
    shortcut: "Ctrl+T",
    scope: "focused",
    tags: ["browser", "tabs"],
  },
  {
    id: "windows-explorer",
    label: "Windows: Open File Explorer",
    shortcut: "Win+E",
    scope: "global",
    tags: ["windows", "files"],
  },
  {
    id: "ahk-date",
    label: "AHK: Insert current date",
    shortcut: "",
    scope: "global",
    tags: ["plugin", "typing"],
  },
];

export function filterCommands(commands: CommandRow[], query: string): CommandRow[] {
  const normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery.length === 0) {
    return commands;
  }

  return commands.filter((command) => {
    const searchableText = [command.label, command.shortcut, command.scope, ...command.tags]
      .join(" ")
      .toLowerCase();
    return searchableText.includes(normalizedQuery);
  });
}
