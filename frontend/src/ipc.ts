import { invoke } from "@tauri-apps/api/core";
import type {
  FilePayload,
  PersistedState,
  PluginStatus,
  ProjectHit,
  SavePayload,
  SearchOptions,
} from "./types";

export const ipc = {
  loadSession: () => invoke<PersistedState>("load_session"),

  saveSession: (session: PersistedState) => invoke<void>("save_session", { session }),

  exitApp: () => invoke<void>("exit_app"),

  readFile: (path: string) => invoke<FilePayload>("read_file", { path }),

  saveFile: (path: string, content: string, encoding: string, eol: string) =>
    invoke<SavePayload>("save_file", { path, content, encoding, eol }),

  watchFiles: (paths: string[]) => invoke<void>("watch_files", { paths }),

  findInFiles: (root: string, needle: string, options: SearchOptions) =>
    invoke<ProjectHit[]>("find_in_files", { root, needle, options }),

  listPlugins: () => invoke<PluginStatus>("list_plugins"),
  reloadPlugins: () => invoke<PluginStatus>("reload_plugins"),
  setPluginEnabled: (name: string, enabled: boolean) =>
    invoke<PluginStatus>("set_plugin_enabled", { name, enabled }),
  installPlugin: (source: string) => invoke<PluginStatus>("install_plugin", { source }),
  removePlugin: (name: string) => invoke<PluginStatus>("remove_plugin", { name }),

  invokePlugin: (name: string, method: string, params: unknown) =>
    invoke<unknown>("invoke_plugin", { name, method, params }),
};
