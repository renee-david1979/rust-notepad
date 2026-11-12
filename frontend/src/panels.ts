import type { PluginStatus } from "./types";

export interface FindResultRow {
  label: string;
  text: string;
  jump: () => void;
}

export type PluginRunner = (name: string, method: string) => void;

export interface PluginActions {
  run: PluginRunner;
  toggle: (name: string, enabled: boolean) => void;
  remove: (name: string) => void;
  install: () => void;
  reload: () => void;
}

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

  renderPlugins(status: PluginStatus, actions: PluginActions): void {
    this.plugins.innerHTML = "";
    for (const warning of status.warnings) {
      const el = document.createElement("div");
      el.className = "plugin-warning";
      el.textContent = `⚠ ${warning}`;
      this.plugins.appendChild(el);
    }
    // Toolbar: install a plugin from a folder, or re-scan the plugin dirs.
    const toolbar = document.createElement("div");
    toolbar.className = "plugin-toolbar";
    const install = document.createElement("button");
    install.textContent = "Install Plugin…";
    install.className = "primary";
    install.addEventListener("click", () => actions.install());
    const reload = document.createElement("button");
    reload.textContent = "Reload Plugins";
    reload.addEventListener("click", () => actions.reload());
    toolbar.append(install, reload);
    this.plugins.appendChild(toolbar);
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
      card.className = plugin.enabled ? "plugin-card" : "plugin-card disabled";
      const title = document.createElement("h4");
      title.textContent = `${plugin.name} ${plugin.version}${plugin.running ? "  • running" : ""}${plugin.enabled ? "" : "  • disabled"}`;
      const meta = document.createElement("div");
      meta.className = "plugin-meta";
      meta.textContent = `${plugin.description || "(no description)"} — ${plugin.dir}`;
      // Management row: enable/disable every plugin, remove installed ones.
      const manage = document.createElement("div");
      manage.className = "plugin-caps";
      const toggle = document.createElement("button");
      toggle.textContent = plugin.enabled ? "Disable" : "Enable";
      toggle.addEventListener("click", () => actions.toggle(plugin.name, !plugin.enabled));
      manage.appendChild(toggle);
      if (plugin.removable) {
        const remove = document.createElement("button");
        remove.textContent = "Remove";
        remove.className = "danger";
        remove.title = "Delete the installed plugin files";
        remove.addEventListener("click", () => actions.remove(plugin.name));
        manage.appendChild(remove);
      }
      const caps = document.createElement("div");
      caps.className = "plugin-caps";
      for (const capability of plugin.capabilities) {
        const btn = document.createElement("button");
        btn.textContent = capability;
        btn.title = "Run on the active document";
        btn.disabled = !plugin.enabled;
        btn.addEventListener("click", () => actions.run(plugin.name, capability));
        caps.appendChild(btn);
      }
      card.append(title, meta, manage, caps);
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
