import { DiffView, type DiffSpec } from "./diff";
import { monaco } from "./monaco-setup";
import type { PaneId, Tab, TabManager } from "./tabs";

export interface PaneCallbacks {
  onTabActivate(id: number): void;
  onTabClose(id: number): void;
  onPaneFocus(pane: PaneId): void;
  onCursorChange(pane: PaneId): void;
  onContentChange(): void;
  onDiffClosed(): void;
}

class Pane {
  root: HTMLElement;
  tabbar: HTMLElement;
  editorHost: HTMLElement;
  diffHost: HTMLElement;
  editor: monaco.editor.IStandaloneCodeEditor;
  diff: DiffView | null = null;
  diffTitle = "";
  lastTabId: number | null = null;

  constructor(id: PaneId, callbacks: PaneCallbacks) {
    this.root = document.createElement("div");
    this.root.className = "pane";
    this.root.dataset.pane = String(id);

    this.tabbar = document.createElement("div");
    this.tabbar.className = "pane-tabbar";
    this.tabbar.addEventListener("mousedown", () => callbacks.onPaneFocus(id));

    const body = document.createElement("div");
    body.className = "pane-body";
    this.editorHost = document.createElement("div");
    this.editorHost.className = "pane-editor-host";
    this.diffHost = document.createElement("div");
    this.diffHost.className = "pane-diff-host";
    body.append(this.editorHost, this.diffHost);

    this.root.append(this.tabbar, body);

    this.editor = monaco.editor.create(this.editorHost, {
      model: null,
      theme: "rn-dark",
      fontSize: 14,
      automaticLayout: true,
      smoothScrolling: true,
      scrollBeyondLastLine: false,
      renderLineHighlight: "all",
    });
    this.editor.onDidChangeCursorSelection(() => callbacks.onCursorChange(id));
    this.editor.onDidChangeModelContent(() => callbacks.onContentChange());
  }

  showDiff(spec: DiffSpec): void {
    if (!this.diff) {
      this.diff = new DiffView(this.diffHost);
    }
    this.diff.setModel(spec);
    this.diffTitle = `${spec.originalTitle} ↔ ${spec.modifiedTitle}`;
    this.editorHost.style.display = "none";
    this.diffHost.style.display = "block";
  }

  closeDiff(): void {
    if (this.diff) {
      this.diff.dispose();
      this.diff = null;
    }
    this.diffTitle = "";
    this.diffHost.style.display = "none";
    this.editorHost.style.display = "block";
  }
}

/**
 * Owns the split container: up to two panes side by side, each with its
 * own editor, tab strip and optional compare (diff) view.
 */
export class PaneManager {
  panes: Record<PaneId, Pane>;
  private separator: HTMLElement;
  private callbacks: PaneCallbacks;
  split = false;

  constructor(container: HTMLElement, callbacks: PaneCallbacks) {
    this.callbacks = callbacks;
    container.innerHTML = "";
    this.panes = {
      1: new Pane(1, callbacks),
      2: new Pane(2, callbacks),
    };
    this.separator = document.createElement("div");
    this.separator.className = "pane-separator hidden";
    container.append(this.panes[1].root, this.separator, this.panes[2].root);
    this.panes[2].root.classList.add("hidden");
  }

  get(id: PaneId): Pane {
    return this.panes[id];
  }

  setFocus(pane: PaneId): void {
    for (const id of [1, 2] as PaneId[]) {
      this.panes[id].root.classList.toggle(
        "pane-inactive",
        this.split && id !== pane,
      );
    }
  }

  setSplit(on: boolean): void {
    this.split = on;
    this.panes[2].root.classList.toggle("hidden", !on);
    this.separator.classList.toggle("hidden", !on);
  }

  /** Swap each pane's editor onto its pane's active tab model. */
  syncEditors(tabs: TabManager): void {
    for (const id of [1, 2] as PaneId[]) {
      const pane = this.panes[id];
      if (pane.diff) {
        // Any tab change supersedes the compare view in that pane.
        pane.closeDiff();
      }
      const tab = tabs.activeIn(id);
      const model = tab?.model ?? null;
      if (pane.editor.getModel() !== model) {
        if (pane.lastTabId != null) {
          const prev = tabs.byId(pane.lastTabId);
          if (prev) {
            prev.viewState = pane.editor.saveViewState();
          }
        }
        pane.editor.setModel(model);
        if (tab?.viewState) {
          pane.editor.restoreViewState(tab.viewState);
        }
      }
      pane.lastTabId = tab?.id ?? null;
    }
    this.setFocus(tabs.activePane);
  }

  renderTabBars(tabs: TabManager): void {
    for (const id of [1, 2] as PaneId[]) {
      const pane = this.panes[id];
      pane.tabbar.innerHTML = "";
      if (pane.diff) {
        pane.tabbar.appendChild(this.diffTab(pane));
        continue;
      }
      for (const tab of tabs.tabsIn(id)) {
        pane.tabbar.appendChild(this.tabEl(tab, tabs));
      }
    }
  }

  private tabEl(tab: Tab, tabs: TabManager): HTMLElement {
    const el = document.createElement("div");
    el.className = "tab" + (tabs.activeByPane[tab.pane] === tab.id ? " active" : "");
    const dot = document.createElement("span");
    dot.className = "dirty-dot";
    dot.style.visibility = tabs.isDirty(tab) ? "visible" : "hidden";
    const title = document.createElement("span");
    title.textContent = tab.title;
    const closeBtn = document.createElement("button");
    closeBtn.className = "close-btn";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.callbacks.onTabClose(tab.id);
    });
    el.append(dot, title, closeBtn);
    el.addEventListener("click", () => this.callbacks.onTabActivate(tab.id));
    el.addEventListener("auxclick", (e) => {
      if ((e as MouseEvent).button === 1) {
        this.callbacks.onTabClose(tab.id);
      }
    });
    return el;
  }

  private diffTab(pane: Pane): HTMLElement {
    const el = document.createElement("div");
    el.className = "tab diff-tab active";
    const title = document.createElement("span");
    title.textContent = `⟺ ${pane.diffTitle}`;
    const closeBtn = document.createElement("button");
    closeBtn.className = "close-btn";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", () => {
      pane.closeDiff();
      this.callbacks.onDiffClosed();
    });
    el.append(title, closeBtn);
    return el;
  }

  applyOptions(options: monaco.editor.IEditorOptions): void {
    for (const id of [1, 2] as PaneId[]) {
      this.panes[id].editor.updateOptions(options);
    }
    for (const id of [1, 2] as PaneId[]) {
      this.panes[id].diff?.updateOptions(options);
    }
  }
}
