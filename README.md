# RustNotepad

A serious Notepad++-style desktop editor: **Tauri 2** shell, **Monaco
Editor** as the editing surface, and a **process-based plugin host**
(JSON-RPC 2.0 over stdio — plugins are external programs in any language).

## What works today

- **Multi-tab editing** — one Monaco model per document, dirty markers,
  close / close-others / close-saved / close-all, per-tab undo & scroll state
- **Files** — New / Open / Save / Save As / Reload from Disk, native dialogs,
  recent-files menu, encoding-aware round-trips: UTF-8, UTF-8 BOM,
  UTF-16 LE/BE (with BOM), Windows-1252; EOL detection (LF/CRLF/CR) kept
  across saves and switchable per file
- **External change detection** — directory watching (survives atomic
  save-and-replace edits); clean files auto-reload, dirty files get a
  Reload / Overwrite / Dismiss banner
- **Syntax highlighting** — Monaco's full language set (60+ languages),
  extension + shebang detection, per-file language override in the status bar
- **Find / Replace** — Notepad++-style docked panel: find next/prev,
  replace, replace-all, replace-all-in-open-tabs, find-all-in-open-tabs,
  find-in-files (backend search with directory/extension filters),
  match case / whole word / regex (with `$1` group expansion)
- **Session persistence** — open tabs (including unsaved untitled buffers),
  active tab, recent files and all settings survive restarts
- **Settings & themes** — dark/light themes, font zoom, word wrap, line
  numbers, whitespace rendering, minimap, tab size, auto-indent
- **Plugins** — external processes speaking JSON-RPC over stdio, discovered
  from plugin directories, capability-gated, timeout-protected (a hung
  plugin is killed, never the editor). See [`docs/PLUGIN_PROTOCOL.md`](docs/PLUGIN_PROTOCOL.md).
  A bundled Python sample plugin (`plugins/toolkit/`) provides word counts,
  JSON/XML validation and JSON formatting, and doubles as the protocol's
  reference implementation and test fixture.
- **Status bar** — line/col, selection length, encoding, EOL, language,
  tab size, file size — encoding/EOL/language switchable in place

## Stack

| Layer | Choice |
| --- | --- |
| Desktop shell & packaging | Tauri 2 (Rust) |
| Editing surface | Monaco Editor (TypeScript, Vite) |
| Backend services | Rust: file I/O + encodings, watcher, session, search, plugin host |
| Plugin host | One process per plugin, JSON-RPC 2.0 lines over stdin/stdout |

## Build & run

Prerequisites: Rust (1.77+), Node 20+, and on macOS the Xcode command line
tools (Tauri needs the system webview toolchain).

```bash
npm install                 # frontend dependencies + Tauri CLI
npm run tauri dev           # run the app in dev mode (Vite + hot reload)
npm run tauri build         # produce a .app and .dmg
```

Tests and checks:

```bash
cd src-tauri
cargo test --all-targets    # unit tests + plugin-host e2e (spawns real processes)
cargo fmt --all -- --check
cargo clippy -- -D warnings

cd ..
npm run build               # tsc --noEmit + vite production build
```

The plugin-host integration tests spawn real child processes (the bundled
Python plugin plus deliberately misbehaving plugins) and verify the JSON-RPC
round trip, capability gating, timeout kills and crash recovery. They skip
politely when `python3` is unavailable.

## Project layout

```
frontend/               TypeScript UI (Monaco, tabs, find panel, menus)
  src/main.ts           wiring: actions, shortcuts, session, events
  src/tabs.ts           tab/model manager
  src/find.ts           find/replace controller
plugins/toolkit/        sample plugin (Python, stdlib only)
docs/PLUGIN_PROTOCOL.md plugin protocol specification
docs/                   PRD, implementation plan, original stack blueprint
src-tauri/
  src/encoding.rs       encoding & EOL detection + round-trip file I/O
  src/search.rs         find-in-tabs + find-in-files
  src/plugins/          manifest loading, child-process transport, RPC host,
                        discovery/lifecycle manager
  src/watcher.rs        external change detection
  src/commands.rs       Tauri IPC surface
  tests/                integration tests (real subprocess plugin e2e)
```

## Installing plugins

Drop a directory containing a `plugin.json` into the app's `plugins/`
directory (see `docs/PLUGIN_PROTOCOL.md` for discovery paths and the wire
protocol), then use **Tools → Reload Plugins**. Run a capability on the
active document from **Tools → (plugin) → (capability)** or the Plugins
panel.

## Roadmap (from the PRD, not yet built)

Macro record/playback, split view, function list / document map, plugin
manager & marketplace, hex viewer, diff viewer, UDL editor, i18n. The
original PRD and implementation plan live in `docs/`.
