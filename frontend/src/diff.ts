import { save as saveDialog } from "@tauri-apps/plugin-dialog";

import { ipc } from "./ipc";
import { monaco } from "./monaco-setup";
import { buildUnifiedPatch } from "./patch";

export interface DiffSpec {
  original: monaco.editor.ITextModel;
  modified: monaco.editor.ITextModel;
  originalTitle: string;
  modifiedTitle: string;
  /** Dispose the model when the diff view closes (temp models only). */
  ownsOriginal: boolean;
  ownsModified: boolean;
}

/**
 * Compare view built on Monaco's diff editor (the same engine as VS
 * Code), with a toolbar exposing the built-in view options — inline
 * (unified) vs side-by-side, ignore whitespace, collapse unchanged
 * regions, +/- change indicators — plus unified-patch copy/export.
 */
export class DiffView {
  private host: HTMLElement;
  private editor: monaco.editor.IStandaloneDiffEditor;
  private spec: DiffSpec | null = null;
  private sideBySide = true;
  private ignoreWhitespace = false;
  private collapseUnchanged = true;
  private toggles: Record<"view" | "whitespace" | "collapse", HTMLButtonElement>;
  private patchButtons: HTMLButtonElement[] = [];
  private status: HTMLElement;
  private statusTimer: number | undefined;

  constructor(host: HTMLElement) {
    this.host = host;
    host.innerHTML = "";
    const toolbar = document.createElement("div");
    toolbar.className = "diff-toolbar";

    this.toggles = {
      view: this.toggleButton("Side by side", () => {
        this.sideBySide = !this.sideBySide;
        this.editor.updateOptions({ renderSideBySide: this.sideBySide });
        this.syncToolbar();
      }),
      whitespace: this.toggleButton("Ignore whitespace", () => {
        this.ignoreWhitespace = !this.ignoreWhitespace;
        this.editor.updateOptions({ ignoreTrimWhitespace: this.ignoreWhitespace });
        this.syncToolbar();
      }),
      collapse: this.toggleButton("Collapse unchanged", () => {
        this.collapseUnchanged = !this.collapseUnchanged;
        this.editor.updateOptions({
          hideUnchangedRegions: { enabled: this.collapseUnchanged },
        });
        this.syncToolbar();
      }),
    };
    const copy = this.actionButton("Copy as patch", () => void this.copyPatch());
    const exportBtn = this.actionButton("Export .patch…", () => void this.exportPatch());
    this.patchButtons.push(copy, exportBtn);
    this.status = document.createElement("span");
    this.status.className = "diff-status";
    toolbar.append(
      this.toggles.view,
      this.toggles.whitespace,
      this.toggles.collapse,
      copy,
      exportBtn,
      this.status,
    );

    const editorHost = document.createElement("div");
    editorHost.className = "diff-editor";
    host.append(toolbar, editorHost);

    this.editor = monaco.editor.createDiffEditor(editorHost, {
      readOnly: true,
      originalEditable: false,
      renderSideBySide: this.sideBySide,
      renderIndicators: true,
      hideUnchangedRegions: { enabled: this.collapseUnchanged },
      automaticLayout: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
    });
    this.syncToolbar();
  }

  private toggleButton(label: string, onClick: () => void): HTMLButtonElement {
    const button = document.createElement("button");
    button.className = "diff-toggle";
    button.textContent = label;
    button.addEventListener("click", onClick);
    return button;
  }

  private actionButton(label: string, onClick: () => void): HTMLButtonElement {
    const button = document.createElement("button");
    button.className = "diff-action";
    button.textContent = label;
    button.addEventListener("click", onClick);
    return button;
  }

  private syncToolbar(): void {
    this.toggles.view.textContent = this.sideBySide ? "Side by side" : "Inline";
    this.toggles.view.classList.toggle("active", !this.sideBySide);
    this.toggles.whitespace.classList.toggle("active", this.ignoreWhitespace);
    this.toggles.collapse.classList.toggle("active", this.collapseUnchanged);
    for (const button of this.patchButtons) {
      button.disabled = this.spec === null;
    }
  }

  private flash(message: string): void {
    this.status.textContent = message;
    window.clearTimeout(this.statusTimer);
    this.statusTimer = window.setTimeout(() => {
      this.status.textContent = "";
    }, 2500);
  }

  private patchText(): string | null {
    if (!this.spec) {
      return null;
    }
    return buildUnifiedPatch(
      this.spec.originalTitle,
      this.spec.original.getValue(),
      this.spec.modifiedTitle,
      this.spec.modified.getValue(),
    );
  }

  private async copyPatch(): Promise<void> {
    try {
      const patch = this.patchText();
      if (patch === null) {
        return;
      }
      await navigator.clipboard.writeText(patch);
      this.flash("Patch copied ✓");
    } catch (err) {
      console.error("patch copy failed", err);
      this.flash("Copy failed");
    }
  }

  private async exportPatch(): Promise<void> {
    try {
      const patch = this.patchText();
      if (patch === null) {
        return;
      }
      const path = await saveDialog({
        defaultPath: "changes.patch",
        filters: [{ name: "Patch", extensions: ["patch", "diff"] }],
      });
      if (typeof path !== "string" || !path) {
        return;
      }
      await ipc.saveFile(path, patch, "UTF-8", "LF");
      this.flash("Patch saved ✓");
    } catch (err) {
      console.error("patch export failed", err);
      this.flash("Export failed");
    }
  }

  setModel(spec: DiffSpec): void {
    this.disposeModels();
    this.spec = spec;
    this.editor.setModel({ original: spec.original, modified: spec.modified });
    this.syncToolbar();
  }

  updateOptions(options: monaco.editor.IEditorOptions): void {
    this.editor.updateOptions(options);
  }

  private disposeModels(): void {
    if (!this.spec) {
      return;
    }
    if (this.spec.ownsOriginal) {
      this.spec.original.dispose();
    }
    if (this.spec.ownsModified) {
      this.spec.modified.dispose();
    }
    this.spec = null;
  }

  dispose(): void {
    this.disposeModels();
    window.clearTimeout(this.statusTimer);
    this.editor.dispose();
    // Drop the toolbar DOM (and its listeners) so a disposed view is not
    // reachable through its buttons; a new DiffView rebuilds the content.
    this.host.innerHTML = "";
  }
}
