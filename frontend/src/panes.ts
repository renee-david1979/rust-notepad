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
  onSplitResize(ratio: number): void;
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
  /** Pane-1 share of the split width (clamped to 0.15–0.85). */
  splitRatio = 0.5;
  private tabDragJustEnded = false;

  constructor(container: HTMLElement, callbacks: PaneCallbacks) {
    this.callbacks = callbacks;
    container.innerHTML = "";
    this.panes = {
      1: new Pane(1, callbacks),
      2: new Pane(2, callbacks),
    };
    this.separator = document.createElement("div");
    this.separator.className = "pane-separator hidden";
    this.separator.addEventListener("pointerdown", (e) =>
      this.startSeparatorDrag(e),
    );
    container.append(this.panes[1].root, this.separator, this.panes[2].root);
    this.panes[2].root.classList.add("hidden");
  }

  /** Set the pane-1 share of the split width, clamped to the usable range. */
  setSplitRatio(ratio: number): void {
    if (!Number.isFinite(ratio)) {
      ratio = 0.5;
    }
    this.splitRatio = Math.min(0.85, Math.max(0.15, ratio));
    if (this.split) {
      this.applySplitRatio();
    }
  }

  private applySplitRatio(): void {
    this.panes[1].root.style.flexGrow = String(this.splitRatio);
    this.panes[2].root.style.flexGrow = String(1 - this.splitRatio);
  }

  private startSeparatorDrag(e: PointerEvent): void {
    if (e.button !== 0 || !this.split) {
      return;
    }
    const container = this.separator.parentElement;
    if (!container) {
      return;
    }
    e.preventDefault();
    this.separator.setPointerCapture(e.pointerId);
    this.separator.classList.add("resizing");
    const move = (ev: PointerEvent) => {
      const rect = container.getBoundingClientRect();
      if (rect.width <= 0) {
        return;
      }
      this.splitRatio = Math.min(
        0.85,
        Math.max(0.15, (ev.clientX - rect.left) / rect.width),
      );
      this.applySplitRatio();
      this.callbacks.onSplitResize(this.splitRatio);
    };
    const up = () => {
      this.separator.classList.remove("resizing");
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
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
    if (on) {
      this.applySplitRatio();
    } else {
      this.panes[1].root.style.flexGrow = "";
      this.panes[2].root.style.flexGrow = "";
    }
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
    el.dataset.tabId = String(tab.id);
    el.dataset.pane = String(tab.pane);
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
    el.addEventListener("pointerdown", (e) =>
      this.startTabDrag(e, tab, tabs, el),
    );
    el.addEventListener("click", () => {
      if (this.tabDragJustEnded) {
        return;
      }
      this.callbacks.onTabActivate(tab.id);
    });
    el.addEventListener("auxclick", (e) => {
      if ((e as MouseEvent).button === 1) {
        this.callbacks.onTabClose(tab.id);
      }
    });
    return el;
  }

  /**
   * Pointer-based tab drag & drop: reorder inside a tab strip, or move to
   * the other pane by dropping on its tab strip while split. Plain mouse
   * moves (no HTML5 drag) so Tauri's native file-drop handling (which
   * swallows DOM drag events) never interferes.
   */
  private startTabDrag(
    e: PointerEvent,
    tab: Tab,
    tabs: TabManager,
    source: HTMLElement,
  ): void {
    if (e.button !== 0) {
      return;
    }
    const startX = e.clientX;
    const startY = e.clientY;
    let dragging = false;
    let beforeId: number | null = null;
    let targetPane: PaneId | null = null;
    let indicator: HTMLElement | null = null;

    const clearIndicator = () => {
      if (indicator) {
        indicator.classList.remove("drop-before");
        indicator = null;
      }
    };

    const move = (ev: PointerEvent) => {
      if (
        !dragging &&
        Math.abs(ev.clientX - startX) < 4 &&
        Math.abs(ev.clientY - startY) < 4
      ) {
        return;
      }
      if (!dragging) {
        dragging = true;
        source.classList.add("dragging");
        source.setPointerCapture(e.pointerId);
      }
      clearIndicator();
      targetPane = null;
      beforeId = null;
      // Drop targets are tab bars; empty bar area = that pane's end.
      for (const bar of document.querySelectorAll<HTMLElement>(".pane-tabbar")) {
        const r = bar.getBoundingClientRect();
        if (
          ev.clientX < r.left ||
          ev.clientX > r.right ||
          ev.clientY < r.top ||
          ev.clientY > r.bottom
        ) {
          continue;
        }
        const paneEl = bar.closest("[data-pane]") as HTMLElement | null;
        if (paneEl?.dataset.pane === "2") {
          targetPane = 2;
        } else if (paneEl?.dataset.pane === "1") {
          targetPane = 1;
        }
        // Insert before the first tab whose left half is past the cursor.
        for (const el of bar.querySelectorAll<HTMLElement>(".tab")) {
          if (el === source || el.dataset.tabId == null) {
            continue;
          }
          const tr = el.getBoundingClientRect();
          if (ev.clientX < tr.left + tr.width / 2) {
            beforeId = Number(el.dataset.tabId);
            el.classList.add("drop-before");
            indicator = el;
            break;
          }
        }
        break;
      }
    };

    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      clearIndicator();
      if (dragging) {
        source.classList.remove("dragging");
        // The click that follows a finished drag must not activate the tab.
        // Browsers dispatch that click in the same task as pointerup, so a
        // timer-cleared flag suppresses exactly it and nothing later.
        this.tabDragJustEnded = true;
        window.setTimeout(() => {
          this.tabDragJustEnded = false;
        }, 0);
        if (targetPane != null) {
          tabs.reorder(tab.id, beforeId, targetPane);
        }
      }
    };

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
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
