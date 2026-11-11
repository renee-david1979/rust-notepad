import { monaco } from "./monaco-setup";
import type { FilePayload, SavePayload } from "./types";

export interface Tab {
  id: number;
  title: string;
  path: string | null;
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
 * Owns one Monaco model per tab; a single editor swaps models on switch
 * (the VS Code pattern) so scroll/undo state stays per-document.
 */
export class TabManager {
  tabs: Tab[] = [];
  activeId: number | null = null;
  /** Called whenever tabs open/close/switch (not on content edits). */
  onChange: () => void = () => {};
  private nextId = 1;

  active(): Tab | null {
    return this.tabs.find((t) => t.id === this.activeId) ?? null;
  }

  byId(id: number): Tab | null {
    return this.tabs.find((t) => t.id === id) ?? null;
  }

  byPath(path: string): Tab | null {
    return this.tabs.find((t) => t.path === path) ?? null;
  }

  isDirty(tab: Tab): boolean {
    return tab.model.getValue() !== tab.savedContent;
  }

  createUntitled(initialContent = ""): Tab {
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

  openFile(payload: FilePayload): Tab {
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

  activate(id: number): void {
    if (this.activeId === id || !this.byId(id)) {
      return;
    }
    this.activeId = id;
    this.onChange();
  }

  close(id: number): void {
    const idx = this.tabs.findIndex((t) => t.id === id);
    if (idx < 0) {
      return;
    }
    const [removed] = this.tabs.splice(idx, 1);
    removed.model.dispose();
    if (this.activeId === id) {
      const next = this.tabs[Math.min(idx, this.tabs.length - 1)];
      this.activeId = next ? next.id : null;
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
