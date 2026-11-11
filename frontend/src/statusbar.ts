import type { Tab } from "./tabs";

export interface CursorInfo {
  lineNumber: number;
  column: number;
  selectionLength: number;
}

export interface StatusBarCallbacks {
  onEncoding(encoding: string): void;
  onEol(eol: string): void;
  onLanguage(language: string): void;
}

const ENCODINGS = ["UTF-8", "UTF-8 BOM", "UTF-16 LE", "UTF-16 BE", "Windows-1252"];
const EOLS = ["LF", "CRLF", "CR"];
const LANGUAGES = [
  "plaintext", "c", "cpp", "csharp", "css", "diff", "go", "html", "ini",
  "java", "javascript", "json", "kotlin", "less", "lua", "markdown",
  "objective-c", "perl", "php", "python", "r", "razor", "ruby", "rust",
  "scss", "shell", "sql", "swift", "typescript", "xml", "yaml",
];

export class StatusBar {
  private position: HTMLElement;
  private selection: HTMLElement;
  private spaces: HTMLElement;
  private size: HTMLElement;
  private dirty: HTMLElement;
  private encodingSelect: HTMLSelectElement;
  private eolSelect: HTMLSelectElement;
  private languageSelect: HTMLSelectElement;

  constructor(root: HTMLElement, callbacks: StatusBarCallbacks) {
    root.innerHTML = "";
    this.position = this.span(root);
    this.selection = this.span(root);
    this.spaces = this.span(root);
    this.dirty = this.span(root);

    const spacer = document.createElement("span");
    spacer.className = "spacer";
    root.appendChild(spacer);

    this.languageSelect = this.select(root, LANGUAGES, callbacks.onLanguage);
    this.encodingSelect = this.select(root, ENCODINGS, callbacks.onEncoding);
    this.eolSelect = this.select(root, EOLS, callbacks.onEol);
    this.size = this.span(root);
    this.update(null, { lineNumber: 1, column: 1, selectionLength: 0 });
  }

  private span(root: HTMLElement): HTMLElement {
    const el = document.createElement("span");
    root.appendChild(el);
    return el;
  }

  private select(
    root: HTMLElement,
    options: string[],
    onChange: (value: string) => void,
  ): HTMLSelectElement {
    const sel = document.createElement("select");
    for (const opt of options) {
      const el = document.createElement("option");
      el.value = opt;
      el.textContent = opt;
      sel.appendChild(el);
    }
    sel.addEventListener("change", () => onChange(sel.value));
    root.appendChild(sel);
    return sel;
  }

  update(tab: Tab | null, cursor: CursorInfo): void {
    this.position.textContent = `Ln ${cursor.lineNumber}, Col ${cursor.column}`;
    this.selection.textContent = `Sel ${cursor.selectionLength}`;
    this.spaces.textContent = tab ? `Spaces: ${tab.model.getOptions().tabSize}` : "";
    this.size.textContent = tab ? this.formatSize(tab.size) : "";
    this.dirty.textContent = tab ? "" : "";
    if (tab) {
      this.encodingSelect.value = tab.encoding;
      this.eolSelect.value = tab.eol;
      this.languageSelect.value = tab.language;
      [this.encodingSelect, this.eolSelect, this.languageSelect].forEach(
        (el) => (el.disabled = false),
      );
    } else {
      [this.encodingSelect, this.eolSelect, this.languageSelect].forEach(
        (el) => (el.disabled = true),
      );
    }
  }

  private formatSize(bytes: number): string {
    if (bytes < 1024) {
      return `${bytes} B`;
    }
    if (bytes < 1024 * 1024) {
      return `${(bytes / 1024).toFixed(1)} KB`;
    }
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
}
