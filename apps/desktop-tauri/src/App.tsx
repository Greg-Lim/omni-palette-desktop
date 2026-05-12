import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { filterCommands, sampleCommands } from "./commands";

type HealthPayload = {
  app_name: string;
  phase: string;
  status: string;
};

export function App() {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState(sampleCommands[0]?.id ?? "");
  const [activeView, setActiveView] = useState<"palette" | "settings">("palette");
  const [health, setHealth] = useState<HealthPayload | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const visibleCommands = useMemo(() => filterCommands(sampleCommands, query), [query]);

  useEffect(() => {
    if (!visibleCommands.some((command) => command.id === selectedId)) {
      setSelectedId(visibleCommands[0]?.id ?? "");
    }
  }, [selectedId, visibleCommands]);

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

  return (
    <main className="min-h-screen bg-zinc-950 p-6 text-zinc-100">
      <section className="mx-auto max-w-4xl">
        <header className="mb-4 flex items-center justify-between gap-4">
          <div>
            <h1 className="text-xl font-semibold">Omni Palette</h1>
            <p className="text-sm text-zinc-400">Phase 2 Tauri wireframe</p>
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
            query={query}
            onQueryChange={setQuery}
            selectedId={selectedId}
            onSelect={setSelectedId}
            rows={visibleCommands}
          />
        ) : (
          <SettingsPlaceholder />
        )}
      </section>
    </main>
  );
}

function PaletteWireframe({
  query,
  onQueryChange,
  selectedId,
  onSelect,
  rows,
}: {
  query: string;
  onQueryChange: (value: string) => void;
  selectedId: string;
  onSelect: (value: string) => void;
  rows: ReturnType<typeof filterCommands>;
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

      <div className="max-h-[420px] overflow-y-auto p-2">
        {rows.length === 0 ? (
          <div className="rounded-md border border-dashed border-zinc-700 p-8 text-center text-sm text-zinc-400">
            No matching commands
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
                  <span className="block text-xs text-zinc-400">{command.scope}</span>
                </span>
                <span className="text-xs text-zinc-400">{command.shortcut || "backend"}</span>
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
        Placeholder view for Phase 2. Runtime settings and extension management stay in the egui
        app until later migration phases.
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
