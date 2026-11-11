/** IPC payload shapes — snake_case mirrors of the Rust structs. */

export interface FilePayload {
  path: string;
  title: string;
  content: string;
  encoding: string;
  eol: string;
  language: string;
  size: number;
  mtime_ms: number;
}

export interface SavePayload {
  path: string;
  size: number;
  mtime_ms: number;
}

export interface SavedTab {
  path: string | null;
  content: string | null;
}

export interface PersistedState {
  theme: string;
  font_size: number;
  show_line_numbers: boolean;
  word_wrap: boolean;
  show_whitespace: boolean;
  show_minimap: boolean;
  tab_size: number;
  auto_indent: boolean;
  auto_reload: boolean;
  open_tabs: SavedTab[];
  active_tab: number;
  recent_files: string[];
}

export interface ProjectHit {
  file: string;
  line: number;
  text: string;
}

export interface SearchOptions {
  include_extensions: string[];
  exclude_dirs: string[];
  max_results: number;
}

export interface PluginInfo {
  name: string;
  version: string;
  description: string;
  capabilities: string[];
  dir: string;
  running: boolean;
}

export interface PluginStatus {
  plugins: PluginInfo[];
  warnings: string[];
}

export interface FileChangedPayload {
  path: string;
  mtime_ms: number;
}

export function defaultSettings(): PersistedState {
  return {
    theme: "dark",
    font_size: 14,
    show_line_numbers: true,
    word_wrap: false,
    show_whitespace: false,
    show_minimap: true,
    tab_size: 4,
    auto_indent: true,
    auto_reload: true,
    open_tabs: [],
    active_tab: 0,
    recent_files: [],
  };
}
