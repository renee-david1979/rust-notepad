// The .ts extension is deliberate: Node's test runner (frontend/tests/)
// resolves only explicit extensions; Vite accepts either form.
import { monaco } from "./monaco-setup.ts";
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

  createUntitled(initialContent = "", pane: PaneId = this.activePane, title?: string): Tab {
    const model = monaco.editor.createModel(
      initialContent,
      "plaintext",
      monaco.Uri.parse(`inmemory://untitled/${this.nextId}`),
    );
    model.setEOL(monaco.editor.EndOfLineSequence.LF);
    const tab: Tab = {
      id: this.nextId++,
      title: title ?? this.nextUntitledTitle(),
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

  /**
   * Next free "Untitled N": continues past the highest existing untitled
   * number so titles never collide (the old count+1 scheme duplicated
   * names after a tab was closed).
   */
  private nextUntitledTitle(): string {
    let max = 0;
    for (const tab of this.tabs) {
      if (tab.path !== null) {
        continue;
      }
      const match = /^Untitled (\d+)$/.exec(tab.title);
      if (match) {
        const value = Number(match[1]);
        // Skip absurd hand-edited numbers (non-finite or > 2^53): they can
        // never equal a generated name, so ignoring them cannot collide.
        if (Number.isSafeInteger(value)) {
          max = Math.max(max, value);
        }
      }
    }
    return `Untitled ${max + 1}`;
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
    if (this.placeInPane(id, pane)) {
      this.onChange();
    }
  }

  /** Pane move without firing onChange. Returns false when nothing moved. */
  private placeInPane(id: number, pane: PaneId): boolean {
    const tab = this.byId(id);
    if (!tab || tab.pane === pane) {
      return false;
    }
    const from = tab.pane;
    tab.pane = pane;
    this.activeByPane[pane] = id;
    this.activePane = pane;
    if (this.activeByPane[from] === id) {
      const next = this.tabsIn(from)[0];
      this.activeByPane[from] = next ? next.id : null;
    }
    return true;
  }

  /**
   * Reorder a tab to sit before `beforeId` within `pane` (drag & drop).
   * `beforeId = null` places it after the pane's last tab. A tab dropped
   * on the other pane's tab strip is moved there first.
   */
  reorder(id: number, beforeId: number | null, pane: PaneId): void {
    const tab = this.byId(id);
    if (!tab) {
      return;
    }
    this.placeInPane(id, pane);
    const from = this.tabs.indexOf(tab);
    this.tabs.splice(from, 1);
    let idx =
      beforeId != null ? this.tabs.findIndex((t) => t.id === beforeId) : -1;
    if (idx < 0) {
      // Append after the pane's last tab.
      idx = this.tabs.length;
      for (let i = this.tabs.length - 1; i >= 0; i--) {
        if (this.tabs[i].pane === pane) {
          idx = i + 1;
          break;
        }
      }
    }
    this.tabs.splice(idx, 0, tab);
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
