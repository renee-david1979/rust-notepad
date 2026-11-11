# Rust Notepad (Notepad++-like)

A cross-platform desktop editor written in Rust. This project is best approached as a **Notepad++-like editor** rather than a strict clone, with Rust handling native app logic and a mature editor engine handling advanced text editing UX.

## Recommended stack

- **Tauri** for desktop shell + packaging (Windows/macOS/Linux).
- **Rust backend** for file I/O, file watching, session restore, settings, plugin host, and native integrations.
- **Monaco Editor** or **CodeMirror 6** for the editing surface (syntax highlighting, search UX, folding, multi-language support, tabs workflow).

## Why this approach

Pure Rust GUI stacks like `egui`/`iced` can build editors, but recreating a full code-editing experience from scratch is costly (selection model, IME behavior, performance on large files, advanced search UX, language tooling integration).

Using Rust + Tauri + Monaco/CodeMirror gets you to a production-quality Notepad++-style experience much faster.

## Feature feasibility

The following feature set is realistic and a good fit for this architecture:

- Multi-tab editor
- Close all / close others / close saved
- External change detection with reload prompt or auto-reload
- Syntax highlighting for many languages
- JSON/XML support
- Find / find-all / replace / replace-all
- Plugins/extensions
- Cross-platform packaging

## Suggested architecture

### 1) App shell

- Tauri application shell
- Rust commands exposed to frontend
- Native menus, file dialogs, recent files, window state

### 2) Editor core

**Frontend editor engine**
- Monaco for stronger IDE-like defaults
- CodeMirror 6 for lighter, highly composable behavior

**Rust backend services**
- File I/O
- File-watch reload logic
- Session persistence
- Plugin discovery/execution
- Search across tabs/folders

### 3) Tab/document model

Each tab should track:

- file path
- dirty state
- last modified timestamp/hash
- encoding
- language mode
- cursor/selection state

### 4) External change handling

Use Rust file watching (`notify` crate):

- If tab is clean: auto reload
- If dirty: show reload/keep edits prompt
- Optional policy: always reload if unchanged in editor

### 5) Syntax and language support

Two paths:

- **Monaco/CodeMirror path**: strongest out-of-the-box language + search UX.
- **Pure Rust path**: `syntect` (+ optional `tree-sitter`) for highlighting/parsing, with more custom editor work required.

## Plugin model recommendation

Prefer cross-platform, safer plugin boundaries over native DLL-style plugins.

### Option A: process-based plugins (recommended)

- Plugin executable per extension
- JSON-RPC over stdin/stdout or local sockets
- Works well with Tauri command/event bridge

### Option B: WASM plugins

Good for text transforms, formatting, validation helpers.

### Option C: dynamic libraries

Possible but not ideal initially due to ABI/versioning/security complexity.

## JSON/XML extension scope

Treat JSON/XML support as:

- syntax highlighting
- format/minify
- validation
- optional tree view and query helpers (XPath/JSONPath)

Example plugin commands:

- Format JSON
- Validate JSON
- Pretty XML
- Validate XML
- Convert XML ↔ JSON

## Search model

Support search at three scopes:

- current tab
- all open tabs
- folder/project

Implementation split:

- current tab search in editor engine
- multi-file search in Rust backend (literal/regex)
- structured results grouped by file/line

## Roadmap

### Phase 1 (MVP)

- Open/save/save as
- Multi-tab + dirty markers
- Close/close all/close others
- Syntax highlighting
- Current-tab find/replace
- External-change auto-reload flow

### Phase 2

- Find in open tabs
- Session restore
- Recent files
- JSON/XML formatting + validation
- Theme and keybinding customization

### Phase 3

- Plugin SDK
- Project-wide search
- Split panes
- Diff view
- Command palette/macros

## Bottom line

**Rust is an excellent choice** for this project.

For a serious Notepad++-style result, the strongest path is:

**Rust + Tauri + Monaco/CodeMirror + process-based plugin host**.

That delivers cross-platform support and advanced editing behavior without rebuilding the hardest parts of the editor from first principles.
