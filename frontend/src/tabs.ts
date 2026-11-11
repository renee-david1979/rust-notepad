import { monaco } from "./monaco-setup";
import type { FilePayload, SavePayload } from "./types";

export type PaneId = 1 | 2;

export interface Tab {
  id: number;
  title: string;
  path: string | null;
  pane: PaneId;
  model: monaco.editor.ITextModel;
  savedContent: string;
  encoding: string;
  eol: string;
  language: string;
  size: number;
  mtimeMs: number;
  viewState: monaco.editor.ICodeEditorViewState | null;
  cursorLine: number;
  cursorColumn: number;
}

/**
 * Owns one Monaco model per tab. With split view open, each tab belongs to
 * a pane (Notepad++-style: two views, each with its own tab strip) and each
 * pane's editor swaps models on switch, keeping scroll/undo per-document.
 */
export class TabManager {
  tabs: Tab[] = [];
  activeByPane: Record<PaneId, number | null> = { 1: null, 2: null };
  activePane: PaneId = 1;
  /** Called whenever tabs open/close/switch/move panes (not on edits). */
  onChange: () => void = () => {};
  private nextId = 1;

  active(): Tab | null {
    return this.activeIn(this.activePane);
  }

  activeIn(pane: PaneId): Tab | null {
    const id = this.activeByPane[pane];
    return id != null ? this.byId(id) : null;
  }

  byId(id: number): Tab | null {
    return this.tabs.find((t) => t.id === id) ?? null;
  }

  byPath(path: string): Tab | null {
    return this.tabs.find((t) => t.path === path) ?? null;
  }

  tabsIn(pane: PaneId): Tab[] {
    return this.tabs.filter((t) => t.pane === pane);
  }

  isDirty(tab: Tab): boolean {
    return tab.model.getValue() !== tab.savedContent;
  }

  createUntitled(initialContent = "", pane: PaneId = this.activePane): Tab {
    const count = this.tabs.filter((t) => t.path === null).length + 1;
    const model = monaco.editor.createModel(
      initialContent,
      "plaintext",
      monaco.Uri.parse(`inmemory://untitled/${this.nextId}`),
    );
    model.setEOL(monaco.editor.EndOfLineSequence.LF);
    const tab: Tab = {
      id: this.nextId++,
      title: `Untitled ${count}`,
      path: null,
      pane,
      model,
      savedContent: initialContent,
      encoding: "UTF-8",
      eol: "LF",
      language: "plaintext",
      size: initialContent.length,
      mtimeMs: 0,
      viewState: null,
      cursorLine: 1,
      cursorColumn: 1,
    };
    this.tabs.push(tab);
    this.activate(tab.id);
    return tab;
  }

  openFile(payload: FilePayload, pane: PaneId = this.activePane): Tab {
    const existing = this.byPath(payload.path);
    if (existing) {
      this.activate(existing.id);
      return existing;
    }
    const model = monaco.editor.createModel(
      payload.content,
      payload.language,
      monaco.Uri.file(payload.path),
    );
    model.setEOL(monaco.editor.EndOfLineSequence.LF);
    const tab: Tab = {
      id: this.nextId++,
      title: payload.title,
      path: payload.path,
      pane,
      model,
      savedContent: payload.content,
      encoding: payload.encoding,
      eol: payload.eol,
      language: payload.language,
      size: payload.size,
      mtimeMs: payload.mtime_ms,
      viewState: null,
      cursorLine: 1,
      cursorColumn: 1,
    };
    this.tabs.push(tab);
    this.activate(tab.id);
    return tab;
  }

  /** Activate a tab and focus its pane. No-op for unknown ids. */
  activate(id: number): void {
    const tab = this.byId(id);
    if (!tab || (this.activeByPane[tab.pane] === id && this.activePane === tab.pane)) {
      return;
    }
    this.activePane = tab.pane;
    this.activeByPane[tab.pane] = id;
    this.onChange();
  }

  focusPane(pane: PaneId): void {
    if (this.activePane === pane) {
      return;
    }
    this.activePane = pane;
    this.onChange();
  }

  /** Move a tab into another pane (used by split view), focusing it there. */
  moveToPane(id: number, pane: PaneId): void {
    const tab = this.byId(id);
    if (!tab || tab.pane === pane) {
      return;
    }
    const from = tab.pane;
    tab.pane = pane;
    this.activeByPane[pane] = id;
    this.activePane = pane;
    if (this.activeByPane[from] === id) {
      const next = this.tabsIn(from)[0];
      this.activeByPane[from] = next ? next.id : null;
    }
    this.onChange();
  }

  close(id: number): void {
    const idx = this.tabs.findIndex((t) => t.id === id);
    if (idx < 0) {
      return;
    }
    const [removed] = this.tabs.splice(idx, 1);
    removed.model.dispose();
    if (this.activeByPane[removed.pane] === id) {
      const next = this.tabsIn(removed.pane)[0];
      this.activeByPane[removed.pane] = next ? next.id : null;
    }
    this.onChange();
  }

  closeOthers(id: number): void {
    for (const tab of [...this.tabs]) {
      if (tab.id !== id) {
        this.close(tab.id);
      }
    }
  }

  closeSaved(): void {
    for (const tab of [...this.tabs]) {
      if (!this.isDirty(tab)) {
        this.close(tab.id);
      }
    }
  }

  closeAll(): void {
    for (const tab of [...this.tabs]) {
      this.close(tab.id);
    }
  }

  markSaved(tab: Tab, payload: SavePayload): void {
    tab.savedContent = tab.model.getValue();
    tab.size = payload.size;
    tab.mtimeMs = payload.mtime_ms;
    this.onChange();
  }

  /** Apply a reload from disk to an existing tab. */
  applyReload(tab: Tab, payload: FilePayload): void {
    if (tab.model.getValue() !== payload.content) {
      tab.model.setValue(payload.content);
    }
    tab.savedContent = payload.content;
    tab.encoding = payload.encoding;
    tab.eol = payload.eol;
    tab.size = payload.size;
    tab.mtimeMs = payload.mtime_ms;
  }

  setLanguage(tab: Tab, language: string): void {
    tab.language = language;
    monaco.editor.setModelLanguage(tab.model, language);
    this.onChange();
  }
}
