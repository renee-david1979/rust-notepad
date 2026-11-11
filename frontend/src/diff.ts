import { monaco } from "./monaco-setup";

export interface DiffSpec {
  original: monaco.editor.ITextModel;
  modified: monaco.editor.ITextModel;
  originalTitle: string;
  modifiedTitle: string;
  /** Dispose the model when the diff view closes (temp models only). */
  ownsOriginal: boolean;
  ownsModified: boolean;
}

/** Side-by-side read-only compare built on Monaco's diff editor. */
export class DiffView {
  private editor: monaco.editor.IStandaloneDiffEditor;
  private spec: DiffSpec | null = null;

  constructor(host: HTMLElement) {
    this.editor = monaco.editor.createDiffEditor(host, {
      readOnly: true,
      originalEditable: false,
      renderSideBySide: true,
      automaticLayout: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
    });
  }

  setModel(spec: DiffSpec): void {
    this.disposeModels();
    this.spec = spec;
    this.editor.setModel({ original: spec.original, modified: spec.modified });
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
    this.editor.dispose();
  }
}
