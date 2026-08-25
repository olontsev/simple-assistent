import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  phaseColor,
  phaseLabel,
  subscribeModelsEvents,
  subscribeStatusEvents,
  useAppStore,
  type Profile,
} from "./store";
import "./App.css";

function StatusBar() {
  const status = useAppStore((s) => s.status);
  const modelName = status.loadedModel
    ? status.loadedModel.split(/[/\\]/).pop()
    : null;

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-lg border border-zinc-200 bg-white px-4 py-3 shadow-sm dark:border-zinc-700 dark:bg-zinc-900">
      <span
        className={`inline-block h-3 w-3 rounded-full ${phaseColor(status.phase)}`}
        title={phaseLabel(status.phase)}
      />
      <div className="min-w-0 flex-1">
        <div className="font-medium text-zinc-900 dark:text-zinc-100">
          {phaseLabel(status.phase)}
          {status.pid != null ? (
            <span className="ml-2 text-sm font-normal text-zinc-500">
              PID {status.pid}
            </span>
          ) : null}
        </div>
        <div className="truncate text-sm text-zinc-500">
          {status.url ?? "—"}
          {modelName ? ` · ${modelName}` : ""}
        </div>
        {status.lastError ? (
          <div className="mt-1 text-sm text-red-600 dark:text-red-400">
            {status.lastError}
          </div>
        ) : null}
      </div>
      <ServerControls />
    </div>
  );
}

function ServerControls() {
  const status = useAppStore((s) => s.status);
  const startServer = useAppStore((s) => s.startServer);
  const stopServer = useAppStore((s) => s.stopServer);
  const loadModel = useAppStore((s) => s.loadModel);
  const unloadModel = useAppStore((s) => s.unloadModel);
  const [busy, setBusy] = useState(false);

  const canStart =
    status.phase === "stopped" || status.phase === "error";
  const canStop =
    status.phase === "starting" ||
    status.phase === "runningEmpty" ||
    status.phase === "runningLoaded";
  const canLoad =
    canStop && status.phase !== "starting";
  const canUnload = status.phase === "runningLoaded";

  async function run(fn: () => Promise<void>) {
    setBusy(true);
    try {
      await fn();
    } catch {
      /* error in store */
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap gap-2">
      <button
        type="button"
        disabled={!canStart || busy}
        onClick={() => run(startServer)}
        className="btn-primary"
      >
        Start
      </button>
      <button
        type="button"
        disabled={!canStop || busy}
        onClick={() => run(stopServer)}
        className="btn-secondary"
      >
        Stop
      </button>
      <button
        type="button"
        disabled={!canLoad || busy}
        onClick={() => run(loadModel)}
        className="btn-secondary"
      >
        Load model
      </button>
      <button
        type="button"
        disabled={!canUnload || busy}
        onClick={() => run(unloadModel)}
        className="btn-secondary"
      >
        Unload
      </button>
    </div>
  );
}

function PathsSection() {
  const config = useAppStore((s) => s.config);
  const saveSettings = useAppStore((s) => s.saveSettings);
  const [llamaPath, setLlamaPath] = useState("");
  const [modelsDir, setModelsDir] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!config) return;
    setLlamaPath(config.llamaCppPath);
    setModelsDir(config.modelsDir);
    setAutostart(config.autostart);
  }, [config]);

  async function pickLlama() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "llama-server", extensions: ["exe"] }],
    });
    if (typeof selected === "string") {
      setLlamaPath(selected);
    }
  }

  async function pickLlamaDir() {
    const selected = await open({ multiple: false, directory: true });
    if (typeof selected === "string") {
      setLlamaPath(selected);
    }
  }

  async function pickModels() {
    const selected = await open({ multiple: false, directory: true });
    if (typeof selected === "string") {
      setModelsDir(selected);
    }
  }

  async function onSave() {
    setSaving(true);
    setSaved(false);
    try {
      await saveSettings(llamaPath, modelsDir, autostart);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch {
      /* store */
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="card">
      <h2 className="card-title">Paths and autostart</h2>
      <label className="field">
        <span>llama.cpp (llama-server.exe or folder)</span>
        <div className="flex gap-2">
          <input
            className="input flex-1"
            value={llamaPath}
            onChange={(e) => setLlamaPath(e.target.value)}
            placeholder="C:/AI/llamacpp/llama-server.exe"
          />
          <button type="button" className="btn-secondary" onClick={pickLlama}>
            File…
          </button>
          <button type="button" className="btn-secondary" onClick={pickLlamaDir}>
            Folder…
          </button>
        </div>
      </label>
      <label className="field">
        <span>Models directory (.gguf)</span>
        <div className="flex gap-2">
          <input
            className="input flex-1"
            value={modelsDir}
            onChange={(e) => setModelsDir(e.target.value)}
            placeholder="C:/AI/LM Models"
          />
          <button type="button" className="btn-secondary" onClick={pickModels}>
            Browse…
          </button>
        </div>
      </label>
      <label className="flex items-center gap-2 text-sm text-zinc-800 dark:text-zinc-200">
        <input
          type="checkbox"
          checked={autostart}
          onChange={(e) => setAutostart(e.target.checked)}
          className="h-4 w-4 rounded border-zinc-300"
        />
        Launch app when Windows starts
      </label>
      <div className="mt-3 flex items-center gap-3">
        <button
          type="button"
          className="btn-primary"
          disabled={saving}
          onClick={onSave}
        >
          {saving ? "Saving…" : "Save"}
        </button>
        {saved ? (
          <span className="text-sm text-emerald-600">Saved</span>
        ) : null}
      </div>
    </section>
  );
}

function ProfilesSection() {
  const config = useAppStore((s) => s.config);
  const selectedProfileId = useAppStore((s) => s.selectedProfileId);
  const setSelectedProfileId = useAppStore((s) => s.setSelectedProfileId);
  const upsertProfile = useAppStore((s) => s.upsertProfile);
  const deleteProfile = useAppStore((s) => s.deleteProfile);
  const setActiveProfile = useAppStore((s) => s.setActiveProfile);

  const selected: Profile | null = useMemo(() => {
    if (!config || !selectedProfileId) return null;
    return config.profiles.find((p) => p.id === selectedProfileId) ?? null;
  }, [config, selectedProfileId]);

  const [name, setName] = useState("");
  const [args, setArgs] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (selected) {
      setName(selected.name);
      setArgs(selected.args);
    }
  }, [selected]);

  if (!config) return null;

  async function onSave() {
    if (!selected) return;
    setBusy(true);
    try {
      await upsertProfile({ id: selected.id, name, args });
    } catch {
      /* store */
    } finally {
      setBusy(false);
    }
  }

  async function onAdd() {
    setBusy(true);
    try {
      await upsertProfile({
        name: "New profile",
        args: "-ngl 99 --host 0.0.0.0 --port 8080",
      });
    } catch {
      /* store */
    } finally {
      setBusy(false);
    }
  }

  async function onDelete() {
    if (!selected) return;
    if (!confirm(`Delete profile “${selected.name}”?`)) return;
    setBusy(true);
    try {
      await deleteProfile(selected.id);
    } catch {
      /* store */
    } finally {
      setBusy(false);
    }
  }

  async function onActivate() {
    if (!selected) return;
    setBusy(true);
    try {
      await setActiveProfile(selected.id);
    } catch {
      /* store */
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card">
      <h2 className="card-title">Launch profiles</h2>
      <p className="mb-3 text-sm text-zinc-500">
        Server arguments only. Model path and{" "}
        <code className="text-xs">--alias</code> come from the tray Model
        menu — do not add <code className="text-xs">-m</code> /{" "}
        <code className="text-xs">--alias</code> here.
      </p>
      <div className="flex gap-4">
        <div className="w-48 shrink-0 space-y-1">
          {config.profiles.map((p) => (
            <button
              key={p.id}
              type="button"
              onClick={() => setSelectedProfileId(p.id)}
              className={`block w-full rounded-md px-3 py-2 text-left text-sm ${
                p.id === selectedProfileId
                  ? "bg-sky-100 text-sky-900 dark:bg-sky-900/40 dark:text-sky-100"
                  : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
              }`}
            >
              {p.name}
              {p.id === config.activeProfileId ? (
                <span className="ml-1 text-xs text-emerald-600">●</span>
              ) : null}
            </button>
          ))}
          <button
            type="button"
            className="btn-secondary mt-2 w-full"
            disabled={busy}
            onClick={onAdd}
          >
            + Add
          </button>
        </div>
        {selected ? (
          <div className="min-w-0 flex-1 space-y-3">
            <label className="field">
              <span>Name</span>
              <input
                className="input"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="field">
              <span>Arguments</span>
              <textarea
                className="input min-h-36 font-mono text-xs"
                value={args}
                onChange={(e) => setArgs(e.target.value)}
                spellCheck={false}
              />
            </label>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                className="btn-primary"
                disabled={busy}
                onClick={onSave}
              >
                Save profile
              </button>
              <button
                type="button"
                className="btn-secondary"
                disabled={busy || selected.id === config.activeProfileId}
                onClick={onActivate}
              >
                Set active
              </button>
              <button
                type="button"
                className="btn-danger"
                disabled={busy || config.profiles.length <= 1}
                onClick={onDelete}
              >
                Delete
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function ModelsHint() {
  const models = useAppStore((s) => s.models);
  const config = useAppStore((s) => s.config);
  const active = config?.activeModelPath;

  return (
    <section className="card">
      <h2 className="card-title">Models</h2>
      <p className="mb-2 text-sm text-zinc-500">
        List is built by recursively scanning the models folder. Selection is
        in the tray “Model” menu.
      </p>
      {active ? (
        <p className="mb-2 text-sm text-zinc-700 dark:text-zinc-300">
          Active:{" "}
          <span className="font-mono text-xs">{active}</span>
        </p>
      ) : (
        <p className="mb-2 text-sm text-zinc-500">No model selected</p>
      )}
      <div className="max-h-40 overflow-auto rounded border border-zinc-200 dark:border-zinc-700">
        {models.length === 0 ? (
          <div className="p-3 text-sm text-zinc-500">No .gguf files found</div>
        ) : (
          <ul className="divide-y divide-zinc-100 text-sm dark:divide-zinc-800">
            {models.map((m) => (
              <li
                key={m.path}
                className={`px-3 py-1.5 font-mono text-xs ${
                  m.path === active
                    ? "bg-emerald-50 dark:bg-emerald-950/40"
                    : ""
                }`}
              >
                {m.relativePath}
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function App() {
  const loadAll = useAppStore((s) => s.loadAll);
  const loading = useAppStore((s) => s.loading);
  const error = useAppStore((s) => s.error);

  useEffect(() => {
    loadAll();
    const unsubs: Array<() => void> = [];
    subscribeStatusEvents().then((fn) => unsubs.push(fn));
    subscribeModelsEvents().then((fn) => unsubs.push(fn));
    return () => {
      unsubs.forEach((fn) => fn());
    };
  }, [loadAll]);

  if (loading) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-zinc-50 text-zinc-600 dark:bg-zinc-950 dark:text-zinc-300">
        Loading…
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-zinc-50 p-5 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <header className="mb-4">
        <h1 className="text-xl font-semibold tracking-tight">
          Simple Assistant
        </h1>
        <p className="text-sm text-zinc-500">
          llama.cpp manager · settings and profiles
        </p>
      </header>

      {error ? (
        <div className="mb-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-300">
          {error}
        </div>
      ) : null}

      <div className="space-y-4">
        <StatusBar />
        <PathsSection />
        <ProfilesSection />
        <ModelsHint />
      </div>
    </main>
  );
}

export default App;
