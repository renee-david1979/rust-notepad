import { ipc } from "./ipc";
import { monaco } from "./monaco-setup";
import type { BottomPanel, FindResultRow } from "./panels";
import type { TabManager } from "./tabs";

const WORD_SEPARATORS = "~!@#$%^&*()-=+[{]}\\|;:'\",.<>/? ";

export interface FindUi {
  askFolder(initial: string): Promise<string | null>;
  showProjectResults(rows: FindResultRow[]): void;
  openPath(path: string): Promise<void>;
}

/**
 * Notepad++-style docked find/replace panel: find next/prev, replace,
 * replace-all (current tab or all open tabs), find-all across open tabs,
 * and find-in-files via the backend.
 */
export class FindController {
  private query = "";
  private replacement = "";
  private matchCase = false;
  private wholeWord = false;
  private useRegex = false;
  private lastFolder = "";

  private panel: HTMLElement;
  private input!: HTMLInputElement;
  private replaceInput!: HTMLInputElement;
  private replaceRow!: HTMLElement;
  private statusEl!: HTMLElement;

  constructor(
    container: HTMLElement,
    private tabs: TabManager,
    private getEditor: () => monaco.editor.IStandaloneCodeEditor | null,
    private bottom: BottomPanel,
    private ui: FindUi,
  ) {
    this.panel = container;
    this.build();
  }

  private build(): void {
    this.panel.innerHTML = "";

    const row1 = document.createElement("div");
    row1.className = "find-row";
    row1.innerHTML = `<label>Find what:</label>`;
    this.input = document.createElement("input");
    this.input.type = "text";
    this.input.id = "find-input";
    row1.appendChild(this.input);
    row1.appendChild(this.button("Find Next", () => this.findNext()));
    row1.appendChild(this.button("Find Previous", () => this.findPrev()));
    row1.appendChild(this.button("Find All in Tabs", () => this.findAllInTabs()));
    row1.appendChild(this.button("Find in Files…", () => void this.findInFiles()));
    const closeBtn = this.button("×", () => this.close());
    closeBtn.style.marginLeft = "auto";
    row1.appendChild(closeBtn);

    const row2 = document.createElement("div");
    row2.className = "find-row";
    row2.innerHTML = `<label>Replace with:</label>`;
    this.replaceInput = document.createElement("input");
    this.replaceInput.type = "text";
    this.replaceInput.id = "replace-input";
    row2.appendChild(this.replaceInput);
    row2.appendChild(this.button("Replace", () => this.replaceCurrent()));
    row2.appendChild(this.button("Replace All", () => this.replaceAllInCurrent()));
    row2.appendChild(this.button("Replace All in Tabs", () => this.replaceAllInTabs()));
    this.replaceRow = row2;

    const row3 = document.createElement("div");
    row3.className = "find-row";
    const opts = document.createElement("div");
    opts.className = "find-options";
    opts.appendChild(this.checkbox("Match case", (v) => (this.matchCase = v)));
    opts.appendChild(this.checkbox("Whole word", (v) => (this.wholeWord = v)));
    opts.appendChild(this.checkbox("Regular expression", (v) => (this.useRegex = v)));
    row3.appendChild(opts);

    const row4 = document.createElement("div");
    row4.className = "find-row";
    this.statusEl = document.createElement("div");
    this.statusEl.className = "find-status";
    row4.appendChild(this.statusEl);

    this.panel.append(row1, row2, row3, row4);

    this.input.addEventListener("input", () => (this.query = this.input.value));
    this.replaceInput.addEventListener("input", () => (this.replacement = this.replaceInput.value));
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        e.shiftKey ? this.findPrev() : this.findNext();
      } else if (e.key === "Escape") {
        this.close();
      }
    });
    this.replaceInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        this.replaceCurrent();
      } else if (e.key === "Escape") {
        this.close();
      }
    });
  }

  private button(label: string, onClick: () => void): HTMLButtonElement {
    const b = document.createElement("button");
    b.textContent = label;
    b.addEventListener("click", onClick);
    return b;
  }

  private checkbox(label: string, onChange: (v: boolean) => void): HTMLLabelElement {
    const wrap = document.createElement("label");
    const box = document.createElement("input");
    box.type = "checkbox";
    box.addEventListener("change", () => onChange(box.checked));
    wrap.append(box, document.createTextNode(label));
    return wrap;
  }

  open(mode: "find" | "replace"): void {
    this.panel.classList.remove("hidden");
    this.replaceRow.classList.toggle("hidden", mode !== "replace");
    const editor = this.getEditor();
    const selection = editor?.getSelection();
    if (editor && selection && !selection.isEmpty()) {
      const selected = editor.getModel()?.getValueInRange(selection) ?? "";
      if (selected && !selected.includes("\n")) {
        this.query = selected;
        this.input.value = selected;
      }
    }
    this.input.focus();
    this.input.select();
  }

  close(): void {
    this.panel.classList.add("hidden");
    this.getEditor()?.focus();
  }

  private setStatus(text: string): void {
    this.statusEl.textContent = text;
  }

  private validateQuery(): boolean {
    if (!this.query) {
      this.setStatus("Enter a search term.");
      return false;
    }
    if (this.useRegex) {
      try {
        new RegExp(this.query);
      } catch (err) {
        this.setStatus(`Invalid regular expression: ${String(err)}`);
        return false;
      }
    }
    return true;
  }

  private matchesIn(tab: TabManager["tabs"][number]): monaco.editor.FindMatch[] {
    try {
      return tab.model.findMatches(
        this.query,
        tab.model.getFullModelRange(),
        this.useRegex,
        this.matchCase,
        this.wholeWord ? WORD_SEPARATORS : null,
        false,
      );
    } catch {
      return [];
    }
  }

  private cursor(): monaco.IPosition {
    return this.getEditor()?.getPosition() ?? { lineNumber: 1, column: 1 };
  }

  private reveal(tab: TabManager["tabs"][number], match: monaco.editor.FindMatch): void {
    const editor = this.getEditor();
    if (!editor || editor.getModel() !== tab.model) {
      this.tabs.activate(tab.id);
    }
    const ed = this.getEditor();
    if (!ed) {
      return;
    }
    ed.setSelection(match.range);
    ed.revealRangeInCenter(match.range, monaco.editor.ScrollType.Smooth);
    ed.focus();
    this.setStatus(
      `Match at Ln ${match.range.startLineNumber}, Col ${match.range.startColumn}`,
    );
  }

  findNext(): void {
    const tab = this.tabs.active();
    if (!tab || !this.validateQuery()) {
      return;
    }
    const matches = this.matchesIn(tab);
    if (matches.length === 0) {
      this.setStatus("No results.");
      return;
    }
    const pos = this.cursor();
    const next =
      matches.find(
        (m) =>
          m.range.startLineNumber > pos.lineNumber ||
          (m.range.startLineNumber === pos.lineNumber &&
            m.range.startColumn >= pos.column),
      ) ?? matches[0];
    this.setStatus(`Match ${matches.indexOf(next) + 1} of ${matches.length}`);
    this.reveal(tab, next);
  }

  findPrev(): void {
    const tab = this.tabs.active();
    if (!tab || !this.validateQuery()) {
      return;
    }
    const matches = this.matchesIn(tab);
    if (matches.length === 0) {
      this.setStatus("No results.");
      return;
    }
    const pos = this.cursor();
    const prev =
      [...matches]
        .reverse()
        .find(
          (m) =>
            m.range.startLineNumber < pos.lineNumber ||
            (m.range.startLineNumber === pos.lineNumber &&
              m.range.startColumn < pos.column),
        ) ?? matches[matches.length - 1];
    this.setStatus(`Match ${matches.indexOf(prev) + 1} of ${matches.length}`);
    this.reveal(tab, prev);
  }

  private expandReplacement(match: monaco.editor.FindMatch): string {
    if (!this.useRegex || !match.matches) {
      return this.replacement;
    }
    return this.replacement.replace(/\$(\d{1,2})/g, (_all, group: string) => {
      const idx = Number(group);
      return match.matches?.[idx] ?? "";
    });
  }

  replaceCurrent(): void {
    const tab = this.tabs.active();
    const editor = this.getEditor();
    if (!tab || !editor || !this.validateQuery()) {
      return;
    }
    const selection = editor.getSelection();
    const matches = this.matchesIn(tab);
    const current = selection
      ? matches.find((m) => monaco.Range.equalsRange(m.range, selection))
      : undefined;
    if (current) {
      editor.executeEdits("find-replace", [
        { range: current.range, text: this.expandReplacement(current) },
      ]);
    }
    this.findNext();
  }

  replaceAllInCurrent(): void {
    const tab = this.tabs.active();
    const editor = this.getEditor();
    if (!tab || !editor || !this.validateQuery()) {
      return;
    }
    const matches = this.matchesIn(tab);
    if (matches.length === 0) {
      this.setStatus("No results.");
      return;
    }
    const edits = matches.map((m) => ({
      range: m.range,
      text: this.expandReplacement(m),
    }));
    editor.executeEdits("find-replace-all", edits);
    this.setStatus(`Replaced ${matches.length} occurrence(s).`);
  }

  replaceAllInTabs(): void {
    if (!this.validateQuery()) {
      return;
    }
    let total = 0;
    for (const tab of this.tabs.tabs) {
      const matches = this.matchesIn(tab);
      if (matches.length > 0) {
        const edits = matches.map((m) => ({
          range: m.range,
          text: this.expandReplacement(m),
        }));
        tab.model.pushEditOperations([], edits, () => null);
        total += matches.length;
      }
    }
    this.setStatus(`Replaced ${total} occurrence(s) across open tabs.`);
  }

  findAllInTabs(): void {
    if (!this.validateQuery()) {
      return;
    }
    const rows: FindResultRow[] = [];
    for (const tab of this.tabs.tabs) {
      for (const match of this.matchesIn(tab)) {
        const line = match.range.startLineNumber;
        const text = tab.model.getLineContent(line).trim();
        rows.push({
          label: `${tab.title}:${line}`,
          text,
          jump: () => this.reveal(tab, match),
        });
      }
    }
    this.setStatus(`${rows.length} result(s) in ${this.tabs.tabs.length} tab(s).`);
    this.ui.showProjectResults(rows);
    this.bottom.show("find-results");
  }

  async findInFiles(): Promise<void> {
    if (!this.validateQuery()) {
      return;
    }
    const folder = await this.ui.askFolder(this.lastFolder || ".");
    if (!folder) {
      return;
    }
    this.lastFolder = folder;
    try {
      const options = {
        include_extensions: [],
        exclude_dirs: [],
        max_results: 500,
      };
      const hits = await ipc.findInFiles(folder, this.query, options);
      const rows: FindResultRow[] = hits.map((hit) => ({
        label: `${hit.file}:${hit.line}`,
        text: hit.text,
        jump: () => void this.jumpToProjectHit(hit),
      }));
      this.setStatus(`${hits.length} result(s) under ${folder}.`);
      this.ui.showProjectResults(rows);
      this.bottom.show("find-results");
    } catch (err) {
      this.setStatus(`Find in Files failed: ${String(err)}`);
    }
  }

  private async jumpToProjectHit(hit: { file: string; line: number }): Promise<void> {
    await this.ui.openPath(hit.file);
    const editor = this.getEditor();
    const tab = this.tabs.active();
    if (editor && tab && tab.path === hit.file) {
      const range = new monaco.Range(hit.line, 1, hit.line, 1);
      editor.setSelection(range);
      editor.revealLineInCenter(hit.line, monaco.editor.ScrollType.Smooth);
      editor.focus();
    }
  }
}
