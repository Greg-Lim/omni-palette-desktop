import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  CommandExecutionResult,
  CommandRow,
  nextSelectedCommandId,
  paletteApi,
} from "./commands";

type HealthPayload = {
  app_name: string;
  phase: string;
  status: string;
};

export function App() {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [rows, setRows] = useState<CommandRow[]>([]);
  const [activeView, setActiveView] = useState<"palette" | "settings">("palette");
  const [health, setHealth] = useState<HealthPayload | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [loadingCommands, setLoadingCommands] = useState(true);
  const [executionResult, setExecutionResult] = useState<CommandExecutionResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoadingCommands(true);

    paletteApi
      .searchCommands(query)
      .then((snapshot) => {
        if (cancelled) {
          return;
        }
        setRows(snapshot.commands);
        setCommandError(null);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }
        setRows([]);
        setCommandError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingCommands(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [query]);

  useEffect(() => {
    setSelectedId((currentId) => nextSelectedCommandId(currentId, rows));
  }, [rows]);

  useEffect(() => {
    invoke<HealthPayload>("health_check")
      .then((payload) => {
        setHealth(payload);
        setHealthError(null);
      })
      .catch((error: unknown) => {
        setHealth(null);
        setHealthError(error instanceof Error ? error.message : String(error));
      });
  }, []);

  const runSelectedCommand = () => {
    if (!selectedId) {
      return;
    }

    paletteApi
      .executeCommand(selectedId)
      .then(setExecutionResult)
      .catch((error: unknown) => {
        setExecutionResult({
          status: "failed",
          message: error instanceof Error ? error.message : String(error),
        });
      });
  };

  return (
    <main className="min-h-screen bg-zinc-950 p-6 text-zinc-100">
      <section className="mx-auto max-w-4xl">
        <header className="mb-4 flex items-center justify-between gap-4">
          <div>
            <h1 className="text-xl font-semibold">Omni Palette</h1>
            <p className="text-sm text-zinc-400">Phase 3 backend command bridge</p>
          </div>
          <div className="flex rounded-md border border-zinc-700 p-1 text-sm">
            <button
              className={viewButtonClass(activeView === "palette")}
              onClick={() => setActiveView("palette")}
              type="button"
            >
              Palette
            </button>
            <button
              className={viewButtonClass(activeView === "settings")}
              onClick={() => setActiveView("settings")}
              type="button"
            >
              Settings
            </button>
          </div>
        </header>

        <StatusStrip health={health} error={healthError} />

        {activeView === "palette" ? (
          <PaletteWireframe
            commandError={commandError}
            executionResult={executionResult}
            loading={loadingCommands}
            onQueryChange={setQuery}
            onRunSelected={runSelectedCommand}
            onSelect={setSelectedId}
            query={query}
            rows={rows}
            selectedId={selectedId}
          />
        ) : (
          <SettingsPlaceholder />
        )}
      </section>
    </main>
  );
}

function PaletteWireframe({
  commandError,
  executionResult,
  loading,
  onQueryChange,
  onRunSelected,
  onSelect,
  query,
  rows,
  selectedId,
}: {
  commandError: string | null;
  executionResult: CommandExecutionResult | null;
  loading: boolean;
  onQueryChange: (value: string) => void;
  onRunSelected: () => void;
  onSelect: (value: string) => void;
  query: string;
  rows: CommandRow[];
  selectedId: string;
}) {
  return (
    <section className="rounded-lg border border-zinc-700 bg-zinc-900">
      <div className="border-b border-zinc-700 p-4">
        <label className="sr-only" htmlFor="command-search">
          Search commands
        </label>
        <input
          autoFocus
          className="w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 py-2 text-base text-zinc-100 outline-none focus:border-amber-500"
          id="command-search"
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Type a command"
          value={query}
        />
      </div>

      <div className="flex items-center justify-between border-b border-zinc-700 px-4 py-2 text-xs text-zinc-400">
        <span>{loading ? "Loading commands..." : `${rows.length} commands`}</span>
        <button
          className="rounded border border-zinc-700 px-3 py-1 text-zinc-100 disabled:text-zinc-600"
          disabled={!selectedId}
          onClick={onRunSelected}
          type="button"
        >
          Run selected
        </button>
      </div>

      {commandError ? (
        <div className="border-b border-zinc-700 px-4 py-2 text-sm text-red-300">
          {commandError}
        </div>
      ) : null}

      {executionResult ? (
        <div className="border-b border-zinc-700 px-4 py-2 text-sm text-zinc-300">
          {executionResult.status}: {executionResult.message}
        </div>
      ) : null}

      <div className="max-h-[420px] overflow-y-auto p-2">
        {rows.length === 0 ? (
          <div className="rounded-md border border-dashed border-zinc-700 p-8 text-center text-sm text-zinc-400">
            {loading ? "Loading commands..." : "No matching commands"}
          </div>
        ) : (
          rows.map((command) => {
            const selected = command.id === selectedId;
            return (
              <button
                className={[
                  "flex w-full items-center justify-between rounded-md px-3 py-3 text-left",
                  selected ? "border border-amber-500 bg-zinc-800" : "border border-transparent",
                ].join(" ")}
                key={command.id}
                onClick={() => onSelect(command.id)}
                type="button"
              >
                <span>
                  <span className="block text-sm font-medium">{command.label}</span>
                  <span className="block text-xs text-zinc-400">
                    {command.focus_state} - {command.priority}
                  </span>
                </span>
                <span className="text-xs text-zinc-400">
                  {command.shortcut_text || "backend"}
                </span>
              </button>
            );
          })
        )}
      </div>
    </section>
  );
}

function SettingsPlaceholder() {
  return (
    <section className="rounded-lg border border-zinc-700 bg-zinc-900 p-6">
      <h2 className="text-lg font-semibold">Settings</h2>
      <p className="mt-2 text-sm text-zinc-400">
        Placeholder view. Runtime settings and extension management stay in the egui app until a
        later migration phase.
      </p>
    </section>
  );
}

function StatusStrip({ health, error }: { health: HealthPayload | null; error: string | null }) {
  return (
    <div className="mb-4 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-300">
      {health ? (
        <span>
          Backend: {health.status} - {health.app_name} - {health.phase}
        </span>
      ) : (
        <span>Backend: {error ? `unavailable (${error})` : "checking..."}</span>
      )}
    </div>
  );
}

function viewButtonClass(active: boolean) {
  return [
    "rounded px-3 py-1",
    active ? "bg-amber-600 text-white" : "text-zinc-300 hover:bg-zinc-800",
  ].join(" ");
}
