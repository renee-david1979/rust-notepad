import type { PluginStatus } from "./types";

export interface FindResultRow {
  label: string;
  text: string;
  jump: () => void;
}

export type PluginRunner = (name: string, method: string) => void;

/** Bottom dock: Find Results and Plugins tabs. */
export class BottomPanel {
  private root: HTMLElement;
  private findResults: HTMLElement;
  private plugins: HTMLElement;

  constructor(root: HTMLElement) {
    this.root = root;
    this.findResults = document.getElementById("panel-find-results") as HTMLElement;
    this.plugins = document.getElementById("panel-plugins") as HTMLElement;

    root.querySelectorAll<HTMLButtonElement>(".panel-tab").forEach((btn) => {
      btn.addEventListener("click", () => {
        const panel = btn.dataset.panel;
        if (panel === "find-results" || panel === "plugins") {
          this.show(panel);
        }
      });
    });
    document.getElementById("panel-close")?.addEventListener("click", () => this.hide());
  }

  show(panel: "find-results" | "plugins"): void {
    this.root.classList.remove("hidden");
    this.root.querySelectorAll(".panel-tab").forEach((btn) => {
      btn.classList.toggle("active", (btn as HTMLElement).dataset.panel === panel);
    });
    this.findResults.classList.toggle("hidden", panel !== "find-results");
    this.plugins.classList.toggle("hidden", panel !== "plugins");
  }

  hide(): void {
    this.root.classList.add("hidden");
  }

  renderFindResults(rows: FindResultRow[]): void {
    this.findResults.innerHTML = "";
    if (rows.length === 0) {
      const empty = document.createElement("div");
      empty.className = "result-row";
      empty.textContent = "No results.";
      this.findResults.appendChild(empty);
      return;
    }
    for (const row of rows) {
      const el = document.createElement("div");
      el.className = "result-row";
      const loc = document.createElement("span");
      loc.className = "r-loc";
      loc.textContent = row.label;
      const text = document.createElement("span");
      text.className = "r-text";
      text.textContent = row.text;
      el.append(loc, text);
      el.addEventListener("click", row.jump);
      this.findResults.appendChild(el);
    }
  }

  renderPlugins(status: PluginStatus, onRun: PluginRunner): void {
    this.plugins.innerHTML = "";
    for (const warning of status.warnings) {
      const el = document.createElement("div");
      el.className = "plugin-warning";
      el.textContent = `⚠ ${warning}`;
      this.plugins.appendChild(el);
    }
    if (status.plugins.length === 0) {
      const empty = document.createElement("div");
      empty.className = "plugin-card";
      empty.textContent =
        "No plugins found. Install plugins into the app config plugins/ directory (see docs/PLUGIN_PROTOCOL.md), then use Tools → Reload Plugins.";
      this.plugins.appendChild(empty);
      return;
    }
    for (const plugin of status.plugins) {
      const card = document.createElement("div");
      card.className = "plugin-card";
      const title = document.createElement("h4");
      title.textContent = `${plugin.name} ${plugin.version}${plugin.running ? "  • running" : ""}`;
      const meta = document.createElement("div");
      meta.className = "plugin-meta";
      meta.textContent = `${plugin.description || "(no description)"} — ${plugin.dir}`;
      const caps = document.createElement("div");
      caps.className = "plugin-caps";
      for (const capability of plugin.capabilities) {
        const btn = document.createElement("button");
        btn.textContent = capability;
        btn.title = "Run on the active document";
        btn.addEventListener("click", () => onRun(plugin.name, capability));
        caps.appendChild(btn);
      }
      card.append(title, meta, caps);
      this.plugins.appendChild(card);
    }
  }

  renderPluginOutput(name: string, method: string, output: unknown, isError: boolean): void {
    const pre = document.createElement("div");
    pre.className = "plugin-output";
    pre.textContent = `${name} → ${method}\n${JSON.stringify(output, null, 2)}`;
    if (isError) {
      pre.style.color = "#e06c60";
    }
    this.plugins.prepend(pre);
  }
}
