import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ServerPhase =
  | "stopped"
  | "starting"
  | "runningEmpty"
  | "runningLoaded"
  | "error";

export interface Profile {
  id: string;
  name: string;
  args: string;
}

export interface AppConfig {
  version: number;
  llamaCppPath: string;
  modelsDir: string;
  autostart: boolean;
  activeProfileId: string;
  activeModelPath: string | null;
  profiles: Profile[];
}

export interface ModelEntry {
  path: string;
  relativePath: string;
  name: string;
  group: string;
}

export interface ServerStatus {
  phase: ServerPhase;
  pid: number | null;
  url: string | null;
  loadedModel: string | null;
  lastError: string | null;
  wantsModel: boolean;
}

interface AppStore {
  config: AppConfig | null;
  models: ModelEntry[];
  status: ServerStatus;
  error: string | null;
  loading: boolean;
  selectedProfileId: string | null;
  loadAll: () => Promise<void>;
  setError: (msg: string | null) => void;
  setStatus: (status: ServerStatus) => void;
  setConfig: (config: AppConfig) => void;
  setSelectedProfileId: (id: string | null) => void;
  saveSettings: (
    llamaCppPath: string,
    modelsDir: string,
    autostart: boolean
  ) => Promise<void>;
  upsertProfile: (input: {
    id?: string;
    name: string;
    args: string;
  }) => Promise<void>;
  deleteProfile: (profileId: string) => Promise<void>;
  setActiveProfile: (profileId: string) => Promise<void>;
  startServer: () => Promise<void>;
  stopServer: () => Promise<void>;
  loadModel: () => Promise<void>;
  unloadModel: () => Promise<void>;
}

const defaultStatus: ServerStatus = {
  phase: "stopped",
  pid: null,
  url: null,
  loadedModel: null,
  lastError: null,
  wantsModel: false,
};

export const useAppStore = create<AppStore>((set, get) => ({
  config: null,
  models: [],
  status: defaultStatus,
  error: null,
  loading: true,
  selectedProfileId: null,

  setError: (msg) => set({ error: msg }),
  setStatus: (status) => set({ status }),
  setConfig: (config) =>
    set({
      config,
      selectedProfileId: get().selectedProfileId ?? config.activeProfileId,
    }),
  setSelectedProfileId: (id) => set({ selectedProfileId: id }),

  loadAll: async () => {
    set({ loading: true, error: null });
    try {
      const [config, models, status] = await Promise.all([
        invoke<AppConfig>("get_config"),
        invoke<ModelEntry[]>("list_models"),
        invoke<ServerStatus>("get_status"),
      ]);
      set({
        config,
        models,
        status,
        selectedProfileId: config.activeProfileId,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  saveSettings: async (llamaCppPath, modelsDir, autostart) => {
    try {
      const config = await invoke<AppConfig>("save_settings", {
        llamaCppPath,
        modelsDir,
        autostart,
      });
      const models = await invoke<ModelEntry[]>("list_models");
      set({ config, models, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  upsertProfile: async (input) => {
    try {
      const config = await invoke<AppConfig>("upsert_profile", { input });
      set({
        config,
        selectedProfileId: config.activeProfileId,
        error: null,
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  deleteProfile: async (profileId) => {
    try {
      const config = await invoke<AppConfig>("delete_profile", { profileId });
      set({
        config,
        selectedProfileId: config.activeProfileId,
        error: null,
      });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  setActiveProfile: async (profileId) => {
    try {
      const config = await invoke<AppConfig>("set_active_profile", {
        profileId,
      });
      set({ config, selectedProfileId: profileId, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  startServer: async () => {
    try {
      const status = await invoke<ServerStatus>("start_server");
      set({ status, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  stopServer: async () => {
    try {
      const status = await invoke<ServerStatus>("stop_server");
      set({ status, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  loadModel: async () => {
    try {
      const status = await invoke<ServerStatus>("load_model");
      set({ status, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  unloadModel: async () => {
    try {
      const status = await invoke<ServerStatus>("unload_model");
      set({ status, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));

export async function subscribeStatusEvents() {
  return listen<ServerStatus>("app://status", (event) => {
    useAppStore.getState().setStatus(event.payload);
  });
}

export async function subscribeModelsEvents() {
  return listen<ModelEntry[]>("app://models", (event) => {
    useAppStore.setState({ models: event.payload });
  });
}

export function phaseLabel(phase: ServerPhase): string {
  switch (phase) {
    case "stopped":
      return "Stopped";
    case "starting":
      return "Starting…";
    case "runningEmpty":
      return "Running (no model)";
    case "runningLoaded":
      return "Running";
    case "error":
      return "Error";
  }
}

export function phaseColor(phase: ServerPhase): string {
  switch (phase) {
    case "stopped":
      return "bg-zinc-400";
    case "starting":
      return "bg-amber-400";
    case "runningEmpty":
    case "runningLoaded":
      return "bg-emerald-500";
    case "error":
      return "bg-red-500";
  }
}
