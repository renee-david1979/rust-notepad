import "./styles.css";

import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import { FindController } from "./find";
import { ipc } from "./ipc";
import { buildMenuBar, type MenuItem } from "./menus";
import { defineThemes, monaco } from "./monaco-setup";
import { BottomPanel, type FindResultRow } from "./panels";
import { StatusBar, type CursorInfo } from "./statusbar";
import { TabManager, type Tab } from "./tabs";
import {
  defaultSettings,
  type FileChangedPayload,
  type PersistedState,
  type PluginStatus,
} from "./types";

const tabs = new TabManager();
let settings: PersistedState = defaultSettings();
let recentFiles: string[] = [];
let editor: monaco.editor.IStandaloneCodeEditor;
let findController: FindController;
let statusBar: StatusBar;
let bottomPanel: BottomPanel;
let pluginCache: PluginStatus | null = null;
let lastActiveTabId: number | null = null;

function $(id: string): HTMLElement {
  return document.getElementById(id) as HTMLElement;
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function errorMessage(err: unknown): string {
  return String((err as { message?: string })?.message ?? err);
}

// ---------------------------------------------------------------------------
// Toast + modal helpers
// ---------------------------------------------------------------------------

let toastTimer: number | undefined;

function toast(message: string): void {
  let el = document.getElementById("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    document.body.appendChild(el);
  }
  el.textContent = message;
  el.classList.remove("hidden");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => el?.classList.add("hidden"), 3200);
}

interface ModalAction {
  label: string;
  primary?: boolean;
  run: () => void;
}

function showModal(title: string, body: HTMLElement, actions: ModalAction[]): void {
  $("modal-title").textContent = title;
  const bodyEl = $("modal-body");
  bodyEl.innerHTML = "";
  bodyEl.appendChild(body);
  const actionsEl = $("modal-actions");
  actionsEl.innerHTML = "";
  for (const action of actions) {
    const btn = document.createElement("button");
    btn.textContent = action.label;
    if (action.primary) {
      btn.classList.add("primary");
    }
    btn.addEventListener("click", () => {
      hideModal();
      action.run();
    });
    actionsEl.appendChild(btn);
  }
  $("modal-overlay").classList.remove("hidden");
}

function hideModal(): void {
  $("modal-overlay").classList.add("hidden");
}

function promptText(title: string, initial: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "text";
    input.value = initial;
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        hideModal();
        resolve(input.value);
      } else if (e.key === "Escape") {
        e.preventDefault();
        hideModal();
        resolve(null);
      }
    });
    showModal(title, input, [
      { label: "Cancel", run: () => resolve(null) },
      { label: "OK", primary: true, run: () => resolve(input.value) },
    ]);
    input.focus();
    input.select();
  });
}

function confirmClose(tab: Tab): Promise<"save" | "discard" | "cancel"> {
  return new Promise((resolve) => {
    const body = document.createElement("p");
    body.textContent = `Save changes to "${tab.title}" before closing?`;
    showModal("Unsaved changes", body, [
      { label: "Cancel", run: () => resolve("cancel") },
      { label: "Don't Save", run: () => resolve("discard") },
      { label: "Save", primary: true, run: () => resolve("save") },
    ]);
  });
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

function applySettings(): void {
  editor.updateOptions({
    fontSize: settings.font_size,
    minimap: { enabled: settings.show_minimap },
    wordWrap: settings.word_wrap ? "on" : "off",
    renderWhitespace: settings.show_whitespace ? "all" : "none",
    lineNumbers: settings.show_line_numbers ? "on" : "off",
    tabSize: settings.tab_size,
    autoIndent: settings.auto_indent ? "full" : "none",
  });
  monaco.editor.setTheme(settings.theme === "light" ? "rn-light" : "rn-dark");
  document.body.classList.toggle("light", settings.theme === "light");
}

function toggleSetting(
  key: "show_line_numbers" | "word_wrap" | "show_whitespace" | "show_minimap",
): void {
  settings[key] = !settings[key];
  applySettings();
  persistSessionSoon();
}

function zoomIn(): void {
  settings.font_size = Math.min(72, settings.font_size + 1);
  applySettings();
  persistSessionSoon();
}

function zoomOut(): void {
  settings.font_size = Math.max(6, settings.font_size - 1);
  applySettings();
  persistSessionSoon();
}

// ---------------------------------------------------------------------------
// Session persistence
// ---------------------------------------------------------------------------

function buildSession(): PersistedState {
  return {
    theme: settings.theme,
    font_size: settings.font_size,
    show_line_numbers: settings.show_line_numbers,
    word_wrap: settings.word_wrap,
    show_whitespace: settings.show_whitespace,
    show_minimap: settings.show_minimap,
    tab_size: settings.tab_size,
    auto_indent: settings.auto_indent,
    auto_reload: settings.auto_reload,
    open_tabs: tabs.tabs.map((t) => ({
      path: t.path,
      // Untitled buffers always persist their content; file tabs persist
      // a draft copy only while dirty (crash recovery for unsaved edits).
      content: t.path && !tabs.isDirty(t) ? null : t.model.getValue(),
      cursor_line: t.cursorLine,
      cursor_column: t.cursorColumn,
    })),
    active_tab: Math.max(0, tabs.tabs.findIndex((t) => t.id === tabs.activeId)),
    recent_files: recentFiles,
  };
}

function persistSession(): void {
  void ipc.saveSession(buildSession()).catch(() => undefined);
}

async function persistSessionNow(): Promise<void> {
  window.clearTimeout(persistTimer);
  await ipc.saveSession(buildSession()).catch(() => undefined);
}

let persistTimer: number | undefined;

function persistSessionSoon(): void {
  window.clearTimeout(persistTimer);
  persistTimer = window.setTimeout(persistSession, 400);
}

function addRecent(path: string): void {
  recentFiles = [path, ...recentFiles.filter((p) => p !== path)].slice(0, 12);
  persistSessionSoon();
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

function renderTabs(): void {
  const bar = $("tabbar");
  bar.innerHTML = "";
  for (const tab of tabs.tabs) {
    const el = document.createElement("div");
    el.className = "tab" + (tab.id === tabs.activeId ? " active" : "");
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
      void closeTab(tab);
    });
    el.append(dot, title, closeBtn);
    el.addEventListener("click", () => tabs.activate(tab.id));
    el.addEventListener("auxclick", (e) => {
      if ((e as MouseEvent).button === 1) {
        void closeTab(tab);
      }
    });
    bar.appendChild(el);
  }
}

function updateWindowTitle(): void {
  const tab = tabs.active();
  const dirty = tab ? tabs.isDirty(tab) : false;
  const name = tab ? tab.title : "RustNotepad";
  void getCurrentWindow().setTitle(`${dirty ? "● " : ""}${name} — RustNotepad`);
}

function cursorInfo(): CursorInfo {
  const pos = editor.getPosition() ?? { lineNumber: 1, column: 1 };
  const sel = editor.getSelection();
  const model = editor.getModel();
  const selectionLength = sel && model ? model.getValueInRange(sel).length : 0;
  return { lineNumber: pos.lineNumber, column: pos.column, selectionLength };
}

function onTabsChanged(): void {
  if (lastActiveTabId !== null && lastActiveTabId !== tabs.activeId) {
    const prev = tabs.byId(lastActiveTabId);
    if (prev) {
      prev.viewState = editor.saveViewState();
    }
  }
  const tab = tabs.active();
  if (tab) {
    if (editor.getModel() !== tab.model) {
      editor.setModel(tab.model);
      if (tab.viewState) {
        editor.restoreViewState(tab.viewState);
      }
    }
  } else {
    editor.setModel(null);
  }
  lastActiveTabId = tabs.activeId;
  renderTabs();
  updateWindowTitle();
  statusBar.update(tab, cursorInfo());
  void syncWatch();
  persistSessionSoon();
}

async function syncWatch(): Promise<void> {
  try {
    await ipc.watchFiles(
      tabs.tabs.map((t) => t.path).filter((p): p is string => p !== null),
    );
  } catch {
    // Watcher failures are non-fatal; mtime checks still cover reloads.
  }
}

// ---------------------------------------------------------------------------
// File actions
// ---------------------------------------------------------------------------

async function openPath(path: string): Promise<void> {
  try {
    const payload = await ipc.readFile(path);
    tabs.openFile(payload);
    addRecent(path);
  } catch (err) {
    toast(`Could not open ${path}: ${errorMessage(err)}`);
  }
}

async function openFilesAction(): Promise<void> {
  const selected = await openDialog({ multiple: true });
  if (!selected) {
    return;
  }
  const paths = Array.isArray(selected) ? selected : [selected];
  for (const path of paths) {
    await openPath(path);
  }
}

async function saveTab(tab: Tab): Promise<boolean> {
  if (!tab.path) {
    return saveTabAs(tab);
  }
  try {
    const payload = await ipc.saveFile(
      tab.path,
      tab.model.getValue(),
      tab.encoding,
      tab.eol,
    );
    tabs.markSaved(tab, payload);
    addRecent(tab.path);
    renderTabs();
    updateWindowTitle();
    dismissBannerIfFor(tab);
    return true;
  } catch (err) {
    toast(`Save failed: ${errorMessage(err)}`);
    return false;
  }
}

async function saveTabAs(tab: Tab): Promise<boolean> {
  const path = await saveDialog({ defaultPath: tab.path ?? tab.title });
  if (!path) {
    return false;
  }
  const owner = tabs.byPath(path);
  if (owner && owner.id !== tab.id) {
    toast(`${basename(path)} is already open in another tab — close or save that tab first.`);
    return false;
  }
  try {
    const payload = await ipc.saveFile(
      path,
      tab.model.getValue(),
      tab.encoding,
      tab.eol,
    );
    tab.path = path;
    tab.title = basename(path);
    tabs.markSaved(tab, payload);
    addRecent(path);
    renderTabs();
    updateWindowTitle();
    void syncWatch();
    return true;
  } catch (err) {
    toast(`Save failed: ${errorMessage(err)}`);
    return false;
  }
}

async function saveActive(): Promise<void> {
  const tab = tabs.active();
  if (tab) {
    await saveTab(tab);
  }
}

async function saveAll(): Promise<void> {
  for (const tab of [...tabs.tabs]) {
    await saveTab(tab);
  }
}

async function closeTab(tab: Tab): Promise<boolean> {
  if (tabs.isDirty(tab)) {
    const choice = await confirmClose(tab);
    if (choice === "cancel") {
      return false;
    }
    // Never destroy work the user asked to keep: if the save fails or the
    // Save As dialog is cancelled, the tab stays open.
    if (choice === "save" && !(await saveTab(tab))) {
      return false;
    }
  }
  tabs.close(tab.id);
  return true;
}

async function closeActive(): Promise<void> {
  const tab = tabs.active();
  if (tab) {
    await closeTab(tab);
  }
}

async function closeOthers(): Promise<void> {
  const active = tabs.active();
  if (!active) {
    return;
  }
  for (const tab of [...tabs.tabs]) {
    if (tab.id !== active.id && !(await closeTab(tab))) {
      break;
    }
  }
}

async function closeAllTabs(): Promise<void> {
  for (const tab of [...tabs.tabs]) {
    if (!(await closeTab(tab))) {
      break;
    }
  }
}

async function reloadTab(tab: Tab): Promise<void> {
  if (!tab.path) {
    return;
  }
  try {
    const payload = await ipc.readFile(tab.path);
    tabs.applyReload(tab, payload);
    renderTabs();
    updateWindowTitle();
    statusBar.update(tab, cursorInfo());
  } catch (err) {
    toast(`Reload failed: ${errorMessage(err)}`);
  }
}

// ---------------------------------------------------------------------------
// External change banner
// ---------------------------------------------------------------------------

let bannerTab: Tab | null = null;

function showExtBanner(tab: Tab): void {
  bannerTab = tab;
  $("ext-banner-text").textContent = `${tab.title} changed on disk`;
  $("ext-banner").classList.remove("hidden");
}

function hideExtBanner(): void {
  $("ext-banner").classList.add("hidden");
  bannerTab = null;
}

function dismissBannerIfFor(tab: Tab): void {
  if (bannerTab === tab) {
    hideExtBanner();
  }
}

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

function renderPluginList(status: PluginStatus): void {
  pluginCache = status;
  bottomPanel.renderPlugins(status, (name, method) => void runPluginCapability(name, method));
}

async function refreshPlugins(): Promise<void> {
  try {
    renderPluginList(await ipc.listPlugins());
    bottomPanel.show("plugins");
  } catch (err) {
    toast(`Plugin listing failed: ${errorMessage(err)}`);
  }
}

async function reloadPluginsAction(): Promise<void> {
  try {
    const status = await ipc.reloadPlugins();
    renderPluginList(status);
    bottomPanel.show("plugins");
    toast(`Plugins reloaded (${status.plugins.length} found)`);
  } catch (err) {
    toast(`Plugin reload failed: ${errorMessage(err)}`);
  }
}

async function runPluginCapability(name: string, method: string): Promise<void> {
  const tab = tabs.active();
  const params = {
    text: tab?.model.getValue() ?? "",
    path: tab?.path ?? null,
    language: tab?.language ?? "plaintext",
  };
  bottomPanel.show("plugins");
  try {
    const result = await ipc.invokePlugin(name, method, params);
    bottomPanel.renderPluginOutput(name, method, result, false);
  } catch (err) {
    bottomPanel.renderPluginOutput(name, method, { error: errorMessage(err) }, true);
  }
}

// ---------------------------------------------------------------------------
// Menus, toolbar, keyboard
// ---------------------------------------------------------------------------

function fileMenuItems(): MenuItem[] {
  return [
    { label: "New", shortcut: "⌘N", action: () => tabs.createUntitled() },
    { label: "Open…", shortcut: "⌘O", action: () => void openFilesAction() },
    {
      label: "Open Recent",
      submenu: () =>
        recentFiles.length > 0
          ? recentFiles.map((path) => ({
              label: basename(path),
              action: () => void openPath(path),
            }))
          : [{ label: "(empty)", disabled: () => true }],
    },
    { separator: true },
    { label: "Save", shortcut: "⌘S", action: () => void saveActive() },
    {
      label: "Save As…",
      shortcut: "⇧⌘S",
      action: () => {
        const tab = tabs.active();
        if (tab) {
          void saveTabAs(tab);
        }
      },
    },
    { label: "Save All", action: () => void saveAll() },
    {
      label: "Reload from Disk",
      action: () => {
        const tab = tabs.active();
        if (tab) {
          void reloadTab(tab);
        }
      },
    },
    { separator: true },
    { label: "Close Tab", shortcut: "⌘W", action: () => void closeActive() },
    {
      label: "Close Others",
      action: () => void closeOthers(),
    },
    { label: "Close Saved", action: () => tabs.closeSaved() },
    { label: "Close All", action: () => void closeAllTabs() },
    { separator: true },
    { label: "Exit", action: () => void getCurrentWindow().close() },
  ];
}

function editMenuItems(): MenuItem[] {
  return [
    {
      label: "Undo",
      shortcut: "⌘Z",
      action: () => editor.trigger("menu", "undo", null),
    },
    {
      label: "Redo",
      shortcut: "⇧⌘Z",
      action: () => editor.trigger("menu", "redo", null),
    },
    { separator: true },
    {
      label: "Cut",
      shortcut: "⌘X",
      action: () => document.execCommand("cut"),
    },
    {
      label: "Copy",
      shortcut: "⌘C",
      action: () => document.execCommand("copy"),
    },
    {
      label: "Paste",
      shortcut: "⌘V",
      action: () => void pasteAction(),
    },
    { separator: true },
    { label: "Find…", shortcut: "⌘F", action: () => findController.open("find") },
    { label: "Replace…", shortcut: "⌥⌘F", action: () => findController.open("replace") },
    {
      label: "Find Next",
      shortcut: "F3",
      action: () => findController.findNext(),
    },
    {
      label: "Find Previous",
      shortcut: "⇧F3",
      action: () => findController.findPrev(),
    },
    {
      label: "Find All in Open Tabs",
      action: () => findController.findAllInTabs(),
    },
    { label: "Find in Files…", action: () => void findController.findInFiles() },
    { separator: true },
    { label: "Go to Line…", shortcut: "⌘G", action: () => void goToLineAction() },
    {
      label: "Select All",
      shortcut: "⌘A",
      action: () => editor.trigger("menu", "editor.action.selectAll", null),
    },
  ];
}

function viewMenuItems(): MenuItem[] {
  return [
    {
      label: "Theme",
      submenu: [
        {
          label: "Dark",
          checked: () => settings.theme === "dark",
          action: () => {
            settings.theme = "dark";
            applySettings();
            persistSessionSoon();
          },
        },
        {
          label: "Light",
          checked: () => settings.theme === "light",
          action: () => {
            settings.theme = "light";
            applySettings();
            persistSessionSoon();
          },
        },
      ],
    },
    { separator: true },
    {
      label: "Word Wrap",
      checked: () => settings.word_wrap,
      action: () => toggleSetting("word_wrap"),
    },
    {
      label: "Line Numbers",
      checked: () => settings.show_line_numbers,
      action: () => toggleSetting("show_line_numbers"),
    },
    {
      label: "Whitespace",
      checked: () => settings.show_whitespace,
      action: () => toggleSetting("show_whitespace"),
    },
    {
      label: "Minimap",
      checked: () => settings.show_minimap,
      action: () => toggleSetting("show_minimap"),
    },
    { separator: true },
    { label: "Zoom In", shortcut: "⌘=", action: zoomIn },
    { label: "Zoom Out", shortcut: "⌘-", action: zoomOut },
    {
      label: "Reset Zoom",
      action: () => {
        settings.font_size = 14;
        applySettings();
        persistSessionSoon();
      },
    },
  ];
}

function toolsMenuItems(): MenuItem[] {
  const items: MenuItem[] = [];
  if (pluginCache && pluginCache.plugins.length > 0) {
    for (const plugin of pluginCache.plugins) {
      items.push({
        label: `${plugin.name} ${plugin.version}`,
        submenu: plugin.capabilities.map((capability) => ({
          label: capability,
          action: () => void runPluginCapability(plugin.name, capability),
        })),
      });
    }
    items.push({ separator: true });
  } else {
    items.push({ label: "Load Plugin List", action: () => void refreshPlugins() });
    items.push({ separator: true });
  }
  items.push({ label: "Show Plugin Panel", action: () => void refreshPlugins() });
  items.push({ label: "Reload Plugins", action: () => void reloadPluginsAction() });
  return items;
}

function helpMenuItems(): MenuItem[] {
  return [
    {
      label: "About RustNotepad",
      action: () => {
        const body = document.createElement("p");
        body.textContent =
          "RustNotepad — a Notepad++-style editor. Tauri + Monaco + a process-based " +
          "JSON-RPC plugin host. Plugins: see docs/PLUGIN_PROTOCOL.md.";
        showModal("About RustNotepad", body, [{ label: "Close", primary: true, run: () => undefined }]);
      },
    },
  ];
}

async function pasteAction(): Promise<void> {
  try {
    const text = await navigator.clipboard.readText();
    const selection = editor.getSelection();
    if (text && selection) {
      editor.executeEdits("paste", [{ range: selection, text }]);
      editor.focus();
    }
  } catch {
    toast("Paste from the menu is not permitted here — use ⌘V / Ctrl+V.");
  }
}

async function goToLineAction(): Promise<void> {
  const value = await promptText("Go to Line", "");
  const line = Number(value);
  if (!value || !Number.isFinite(line) || line < 1) {
    return;
  }
  const model = editor.getModel();
  const clamped = Math.min(Math.floor(line), model?.getLineCount() ?? 1);
  editor.setPosition({ lineNumber: clamped, column: 1 });
  editor.revealLineInCenter(clamped, monaco.editor.ScrollType.Smooth);
  editor.focus();
}

function buildToolbar(): void {
  const tb = $("toolbar");
  tb.innerHTML = "";
  const addButton = (label: string, title: string, run: () => void): void => {
    const btn = document.createElement("button");
    btn.textContent = label;
    btn.title = title;
    btn.addEventListener("click", run);
    tb.appendChild(btn);
  };
  const sep = (): void => {
    const el = document.createElement("div");
    el.className = "tb-sep";
    tb.appendChild(el);
  };

  addButton("New", "New file (⌘N)", () => tabs.createUntitled());
  addButton("Open", "Open file (⌘O)", () => void openFilesAction());
  addButton("Save", "Save (⌘S)", () => void saveActive());
  sep();
  addButton("Cut", "Cut (⌘X)", () => document.execCommand("cut"));
  addButton("Copy", "Copy (⌘C)", () => document.execCommand("copy"));
  addButton("Paste", "Paste (⌘V)", () => void pasteAction());
  sep();
  addButton("Find", "Find (⌘F)", () => findController.open("find"));
  addButton("Replace", "Replace (⌥⌘F)", () => findController.open("replace"));
  sep();
  addButton("Wrap", "Toggle word wrap", () => toggleSetting("word_wrap"));
  addButton("Theme", "Toggle light/dark theme", () => {
    settings.theme = settings.theme === "light" ? "dark" : "light";
    applySettings();
    persistSessionSoon();
  });
}

function wireKeyboard(): void {
  window.addEventListener(
    "keydown",
    (e) => {
      const mod = e.metaKey || e.ctrlKey;
      if (e.key === "F3") {
        e.preventDefault();
        e.stopPropagation();
        e.shiftKey ? findController.findPrev() : findController.findNext();
        return;
      }
      if (!mod) {
        return;
      }
      const key = e.key.toLowerCase();
      const handled = (): void => {
        e.preventDefault();
        e.stopPropagation();
      };
      switch (key) {
        case "s":
          handled();
          e.shiftKey ? void saveTabAs(tabs.active() ?? tabs.createUntitled()) : void saveActive();
          break;
        case "o":
          handled();
          void openFilesAction();
          break;
        case "n":
          handled();
          tabs.createUntitled();
          break;
        case "w":
          handled();
          void closeActive();
          break;
        case "f":
          handled();
          findController.open(e.altKey ? "replace" : "find");
          break;
        case "h":
          handled();
          findController.open("replace");
          break;
        case "g":
          handled();
          void goToLineAction();
          break;
        case "=":
        case "+":
          handled();
          zoomIn();
          break;
        case "-":
          handled();
          zoomOut();
          break;
        default:
          break;
      }
    },
    true,
  );
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

async function start(): Promise<void> {
  defineThemes();

  editor = monaco.editor.create($("editor-host"), {
    model: null,
    theme: "rn-dark",
    fontSize: 14,
    automaticLayout: true,
    smoothScrolling: true,
    scrollBeyondLastLine: false,
    renderLineHighlight: "all",
  });

  bottomPanel = new BottomPanel($("bottom-panel"));
  statusBar = new StatusBar($("statusbar"), {
    onEncoding: (encoding) => {
      const tab = tabs.active();
      if (tab) {
        tab.encoding = encoding;
        persistSessionSoon();
      }
    },
    onEol: (eol) => {
      const tab = tabs.active();
      if (tab) {
        tab.eol = eol;
        persistSessionSoon();
      }
    },
    onLanguage: (language) => {
      const tab = tabs.active();
      if (tab) {
        tabs.setLanguage(tab, language);
      }
    },
  });

  findController = new FindController($("find-panel"), tabs, () => editor, bottomPanel, {
    askFolder: (initial) => promptText("Find in Files — folder path", initial),
    showProjectResults: (rows: FindResultRow[]) => bottomPanel.renderFindResults(rows),
    openPath: (path: string) => openPath(path),
  });

  tabs.onChange = onTabsChanged;

  buildMenuBar($("menubar"), [
    { label: "File", items: fileMenuItems },
    { label: "Edit", items: editMenuItems },
    { label: "View", items: viewMenuItems },
    { label: "Tools", items: toolsMenuItems },
    { label: "Help", items: helpMenuItems },
  ]);
  buildToolbar();
  wireKeyboard();

  editor.onDidChangeCursorSelection(() => {
    const tab = tabs.active();
    const info = cursorInfo();
    if (tab) {
      tab.cursorLine = info.lineNumber;
      tab.cursorColumn = info.column;
    }
    statusBar.update(tab, info);
  });
  editor.onDidChangeModelContent(() => {
    renderTabs();
    updateWindowTitle();
    persistSessionSoon();
  });

  $("ext-reload").addEventListener("click", () => {
    if (bannerTab) {
      void reloadTab(bannerTab);
    }
    hideExtBanner();
  });
  $("ext-overwrite").addEventListener("click", () => {
    if (bannerTab) {
      void saveTab(bannerTab);
    }
    hideExtBanner();
  });
  $("ext-dismiss").addEventListener("click", hideExtBanner);

  // Restore session.
  let session: PersistedState;
  try {
    session = await ipc.loadSession();
  } catch {
    session = defaultSettings();
  }
  settings = session;
  recentFiles = session.recent_files ?? [];
  applySettings();

  for (const saved of session.open_tabs ?? []) {
    if (saved.path) {
      const before = tabs.tabs.length;
      await openPath(saved.path);
      const tab =
        tabs.tabs.length > before
          ? tabs.tabs[tabs.tabs.length - 1]
          : tabs.byPath(saved.path);
      if (tab) {
        // Recover unsaved draft content (crash safety): the disk content
        // stays in savedContent so the tab remains dirty.
        if (saved.content != null && saved.content !== tab.model.getValue()) {
          tab.model.setValue(saved.content);
        }
        tab.cursorLine = saved.cursor_line || 1;
        tab.cursorColumn = saved.cursor_column || 1;
      }
    } else {
      const tab = tabs.createUntitled(saved.content ?? "");
      if (saved.content) {
        // A restored untitled buffer is unsaved work: keep it dirty so
        // closing it prompts instead of silently dropping the draft.
        tab.savedContent = "";
      }
      tab.cursorLine = saved.cursor_line || 1;
      tab.cursorColumn = saved.cursor_column || 1;
    }
  }
  if (tabs.tabs.length === 0) {
    tabs.createUntitled();
  }
  const active = tabs.tabs[session.active_tab ?? 0];
  if (active && active.id !== tabs.activeId) {
    tabs.activate(active.id);
  }
  const activeTab = tabs.active();
  if (activeTab) {
    const lineCount = editor.getModel()?.getLineCount() ?? 1;
    const position = {
      lineNumber: Math.min(activeTab.cursorLine, lineCount),
      column: activeTab.cursorColumn,
    };
    editor.setPosition(position);
    editor.revealLineInCenter(position.lineNumber, monaco.editor.ScrollType.Smooth);
    editor.focus();
  }
  // Restored drafts flip dirty state after their tab was rendered.
  renderTabs();
  updateWindowTitle();

  await syncWatch();

  // Prompt for unsaved work on any close path (Exit menu, ⌘Q, window
  // close) and flush the session before the window goes away.
  const appWindow = getCurrentWindow();
  await appWindow.onCloseRequested(async (event) => {
    event.preventDefault();
    for (const tab of [...tabs.tabs]) {
      if (tabs.isDirty(tab)) {
        const choice = await confirmClose(tab);
        if (choice === "cancel") {
          return;
        }
        if (choice === "save" && !(await saveTab(tab))) {
          return;
        }
      }
    }
    await persistSessionNow();
    await appWindow.destroy();
  });

  await listen<FileChangedPayload>("file-changed", (event) => {
    const tab = tabs.byPath(event.payload.path);
    if (!tab) {
      return;
    }
    if (event.payload.mtime_ms === tab.mtimeMs) {
      return; // echo of our own save
    }
    tab.mtimeMs = event.payload.mtime_ms;
    if (!tabs.isDirty(tab)) {
      if (settings.auto_reload) {
        void reloadTab(tab);
        toast(`${tab.title} reloaded (changed on disk)`);
      } else {
        showExtBanner(tab);
      }
      return;
    }
    showExtBanner(tab);
  });

  void refreshPlugins();
}

void start();
