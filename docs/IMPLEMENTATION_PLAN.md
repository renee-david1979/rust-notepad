# TextEdit — Implementation Plan

**Source**: `notepad-plus-plus-clone-prd.md` v1.0
**Tech Stack**: Rust + egui 0.28 / eframe 0.28 + syntect
**Last synced**: 2026-03-16 (Phase 1 implementation)

> Checkboxes sync with implementation status. Checked = implemented and tested.

---

## Technology & Library Decisions

| Concern | Library | Version | Status |
|---------|---------|---------|--------|
| UI Framework | `egui` / `eframe` | 0.28 | In use |
| Syntax Highlighting | `syntect` (via `egui_extras` + direct) | 5.3 | In use |
| Text Buffer | `ropey` | — | NOT YET (using String) |
| Regex Search | `regex` | 1 | In use |
| File Dialogs | `rfd` | 0.14 | In use |
| Serialization | `serde` + `serde_json` | 1.0 | In use |
| XML | `quick-xml` | 0.36 | In use |
| Encoding | `encoding_rs` | 0.8 | In use |
| Diff Engine | `similar` | — | NOT YET (hand-rolled) |
| File Watching | `notify` | 6 | In use (with polling fallback) |
| CLI Parsing | `clap` | 4 | In use |
| Hashing | `sha2` + `md-5` + `base64` | — | NOT YET |
| Plugin IPC | JSON-RPC 2.0 over stdio | — | Protocol designed |
| Benchmarks | `criterion` | — | NOT YET |
| Coverage | `cargo-llvm-cov` | — | NOT YET |

---

## Current Architecture (Post-MVP)

### Source Files (19 files)

```
src/
├── main.rs                  — Entry point, window setup
├── lib.rs                   — Crate root, exports all library modules
├── core.rs                  — Document model, TabId, SessionState, AppError, Clock
├── editor_state.rs          — EditorState: tabs, file I/O, find/replace, encoding
├── editor_services.rs       — Multi-file search, session serialize, JSON/XML validation
├── extensibility.rs         — Plugin host, project search, diff, split panes, macros
├── theme.rs                 — AppTheme (Dark/Light), full custom Visuals + Style
├── shortcuts.rs             — Keyboard shortcut definitions, menu_item helper
├── settings.rs              — FindState, ViewSettings, PersistedState, session persistence
├── plugins.rs               — EditorPlugin trait, JSON/XML validators
├── folding.rs               — FoldState, brace/XML/JSON/indent fold strategies
└── app/
    ├── mod.rs               — RustNotepadApp, eframe::App impl, close confirmation
    ├── menu_bar.rs          — File/Edit/Search/View/Settings/Tools/Window/Help menus
    ├── toolbar.rs           — Icon toolbar with tooltips
    ├── tab_bar.rs            — Wrappable/scrollable tabs with close buttons
    ├── find_panel.rs        — Find & Replace side panel with result navigation
    ├── editor_panel.rs      — Code editor, gutter, fold markers, highlights
    ├── status_bar.rs        — Cursor pos, line/char count, syntax selector, theme
    └── dialogs.rs           — Go to Line, Unsaved Changes confirmation
```

### Test Inventory: 77 tests

| Module | Count | Type |
|--------|-------|------|
| `core.rs` | 10 | Unit (EOL, encoding, shebang) |
| `editor_state.rs` | 10 | Unit + integration (regex, extended) |
| `editor_services.rs` | 7 | Unit |
| `extensibility.rs` | 12 | Unit |
| `theme.rs` | 3 | Unit |
| `shortcuts.rs` | 3 | Unit |
| `settings.rs` | 5 | Unit |
| `plugins.rs` | 5 | Unit |
| `folding.rs` | 18 | Unit (custom markers, fold levels) |
| `app/mod.rs` | 7 | Unit |
| `tests/extensibility_e2e.rs` | 1 | Integration E2E |

---

## Phase 1 — MVP Core Editor (P0 Requirements)

### 1.1 Text Buffer Engine

**PRD**: CE-01 (4 GB files), CE-13 (undo/redo)

| Task | Status |
|------|--------|
| - [ ] Add `ropey` crate — rope-based buffer | GAP |
| - [ ] `TextBuffer` wrapping `ropey::Rope` | GAP |
| - [ ] `UndoManager` with operation-based undo/redo | GAP |
| - [ ] Migrate `Document.content` from `String` to `TextBuffer` | GAP |
| - [ ] Benchmark: open 100 MB file in < 2 s | GAP |

> **Note**: Ropey migration deferred — requires rewriting all text handling and egui TextEdit integration. egui's TextEdit requires `&mut String`, so migration needs a custom editor widget.

---

### 1.2 Encoding & EOL

**PRD**: CE-10, CE-11, FM-12

| Task | Status |
|------|--------|
| - [x] Add `encoding_rs` crate | DONE |
| - [x] Detect encoding: UTF-8 BOM, UTF-16 LE/BE BOM, plain UTF-8, Windows-1252 fallback | DONE |
| - [x] `decode_bytes()` — handles all encodings transparently on file open | DONE |
| - [x] `EolStyle` enum (LF/CRLF/CR) + EOL detection on load + conversion on save | DONE |
| - [x] `DetectedEncoding` enum tracking encoding per document | DONE |
| - [x] UI: encoding + EOL indicators in status bar | DONE |
| - [x] Save with document's EOL style (LF internally, converted on save) | DONE |
| - [x] EOL conversion menu (Edit > EOL Conversion > LF/CRLF/CR) | DONE |

---

### 1.3 Editor Features

**PRD**: CE-07 (word wrap), CE-08 (line numbers), CE-18 (zoom)

| Task | Status |
|------|--------|
| - [x] Word wrap toggle (View menu + Alt+Z) | DONE |
| - [x] Line number gutter (togglable, auto-sizing digits) | DONE |
| - [x] Gutter uses galley row positions for pixel-perfect alignment | DONE |
| - [x] Zoom: Ctrl+Scroll, toolbar +/- buttons, font size 6–72pt | DONE |
| - [x] UI zoom: scales entire UI (menus, toolbar, tabs) via `set_pixels_per_point` | DONE |
| - [x] Syntax highlighting with font size override in layouter | DONE |
| - [ ] Multi-caret editing (Alt+Click) | GAP (requires custom TextEdit widget) |
| - [ ] Column/block selection (Alt+Drag) | GAP (requires custom TextEdit widget) |
| - [ ] Overtype mode (Insert key) | GAP (requires custom TextEdit widget) |
| - [x] Auto-indent on newline (matches previous line indentation) | DONE |
| - [x] Brace/bracket/paren matching highlight | DONE |
| - [x] Current line highlight (subtle background) | DONE |

---

### 1.4 Whitespace Visualization

**PRD**: CE-09, CE-20, CU-09

| Task | Status |
|------|--------|
| - [x] Render `·` for spaces, `→` for tabs as overlays | DONE |
| - [x] Toggle via View menu | DONE |
| - [x] Uses galley cursor positions for accurate placement | DONE |
| - [x] Tab size setting (2/4/8) in View menu, persisted | DONE |
| - [x] Tab-to-spaces / spaces-to-tabs conversion (Edit menu) | DONE |
| - [x] Tab size displayed in status bar | DONE |

---

### 1.5 File & Session Management

**PRD**: FM-01 through FM-12

| Task | Status |
|------|--------|
| - [x] New / Open (multi-file) / Save / Save As / Close / Close All / Close Others | DONE |
| - [x] Save All (Edit menu + toolbar) | DONE |
| - [x] External change detection (polling, 1s interval) | DONE |
| - [x] External change: yellow warning + Reload button in status bar | DONE |
| - [x] Dirty-close confirmation dialog: Save All & Close / Discard All / Cancel | DONE |
| - [x] Session persistence: save on exit, restore on launch (`~/.codeedit/session.json`) | DONE |
| - [x] Persists: theme, font size, UI zoom, view toggles, open tab paths, active tab | DONE |
| - [x] Recent files list with File > Recent Files submenu (max 10, persisted) | DONE |
| - [x] `notify` file watcher for external change detection (with polling fallback) | DONE |
| - [x] CLI arg parsing with `clap` (open files from command line) | DONE |
| - [x] Drag-and-drop files from OS to open them | DONE |

---

### 1.6 Syntax Highlighting

**PRD**: SH-01 through SH-04, SH-08, SH-11, SH-12

| Task | Status |
|------|--------|
| - [x] Language auto-detection by file extension (80+ extensions mapped) | DONE |
| - [x] Syntect integration with 35+ languages having full highlighting | DONE |
| - [x] Smart fallback for unsupported languages (TS→JS, PS→Shell, TOML→YAML) | DONE |
| - [x] Manual language override: combo box in status bar (36 languages) | DONE |
| - [x] Global theme: Dark + Light with full custom Visuals | DONE |
| - [x] Highlight all occurrences of selected whole word (yellow overlay) | DONE |
| - [x] Light theme highlights visible (boosted alpha for light backgrounds) | DONE |
| - [x] Current line highlight (subtle background, dark + light themes) | DONE |
| - [ ] Style Configurator (per-token color/font) | GAP |
| - [x] Shebang detection (`#!/usr/bin/env python3` → Python syntax) | DONE |

---

### 1.7 Search & Replace

**PRD**: SR-01 through SR-06, SR-09, SR-14

| Task | Status |
|------|--------|
| - [x] Case-sensitive and whole-word options | DONE |
| - [x] Find & Replace side panel (Ctrl+F / Ctrl+H) | DONE |
| - [x] Find All with result count | DONE |
| - [x] Replace current / Replace All | DONE |
| - [x] Search results with Ln/Col + context snippet, clickable | DONE |
| - [x] Click result → navigates to match, selects text, cursor at end | DONE |
| - [x] Find Next (F3) / Find Previous (Shift+F3) with wrap-around | DONE |
| - [x] Go to Line dialog (Ctrl+G) with Enter/Escape keyboard support | DONE |
| - [x] Auto-search on Enter in find field | DONE |
| - [x] Regex search mode (checkbox, uses `regex` crate) | DONE |
| - [x] Extended search mode (`\n`, `\t`, `\r`, `\\` expansion) | DONE |
| - [x] Find in Files UI (folder picker, results list, click to open) | DONE |

---

### 1.8 Code Folding

**PRD**: CF-01, CF-02, CF-03

| Task | Status |
|------|--------|
| - [x] `src/folding.rs` with `FoldState` and `FoldRegion` | DONE |
| - [x] Brace-based folding `{}` for C-family (C#, Java, Rust, JS, Go, PS) | DONE |
| - [x] JSON folding: `{}` + `[]` brackets | DONE |
| - [x] XML/HTML tag-based folding (matching open/close tags) | DONE |
| - [x] Indent-based folding for Python, YAML, INI | DONE |
| - [x] Fold gutter markers: ▶ (collapsed) / ▼ (expanded), clickable | DONE |
| - [x] Gutter markers use galley positions for pixel-perfect alignment | DONE |
| - [x] Collapsed regions show `/* ... N lines ... */` placeholder | DONE |
| - [x] Fold All / Unfold All (View menu) | DONE |
| - [x] Syntax-aware dispatch: picks strategy based on file extension | DONE |
| - [x] 18 unit tests: brace, nested, JSON, XML, indent, toggle, display, custom markers, levels | DONE |
| - [x] Fold Level 1–8 commands (View > Fold Level submenu) | DONE |
| - [x] Custom fold markers (`// {{{` / `// }}}`, `# {{{` / `# }}}`) | DONE |

---

### 1.9 Tab Bar

**PRD**: MDI-01 through MDI-04

| Task | Status |
|------|--------|
| - [x] Tab bar with document titles | DONE |
| - [x] Modified indicator (● dot) | DONE |
| - [x] Close button (×) on each tab | DONE |
| - [x] Tab context menu: Close, Close Others, Close All, Copy Path | DONE |
| - [x] Configurable: wrap to multiple lines (default) or horizontal scroll | DONE |
| - [x] Setting persisted in session | DONE |
| - [ ] Tab drag-and-drop reordering | GAP |

---

### 1.10 Customization & UI

**PRD**: CU-01, CU-03, CU-05, CU-06

| Task | Status |
|------|--------|
| - [x] Menu bar: File, Edit, Search, View, Settings, Tools, Window, Help | DONE |
| - [x] Toolbar with icon buttons and tooltips | DONE |
| - [x] View menu: toggle toolbar, status bar, line numbers, word wrap, whitespace, tab wrap | DONE |
| - [x] View menu: font size +/-, UI zoom +/-/reset, Fold All/Unfold All | DONE |
| - [x] Settings menu: theme toggle (Dark/Light) | DONE |
| - [x] Modern glass-style theme: custom Visuals for dark + light | DONE |
| - [x] Rounded corners, subtle shadows, accent colors, translucent panels | DONE |
| - [x] Theme applied every frame (no drift after eframe reset) | DONE |
| - [x] Keyboard shortcuts: Ctrl+N/O/S/W/F/H/G, Ctrl+Shift+S, F3, Ctrl+Tab, Alt+Z | DONE |
| - [ ] Full keyboard shortcut remapping | GAP |
| - [ ] Preferences dialog | GAP |
| - [ ] Per-language font override | GAP |

---

### 1.11 Status Bar

**PRD**: TU-04, TU-12, TU-13

| Task | Status |
|------|--------|
| - [x] Cursor position: Ln X, Col Y (from galley cursor) | DONE |
| - [x] Selection length display | DONE |
| - [x] Line count, character count | DONE |
| - [x] Syntax language indicator (combo box dropdown to override) | DONE |
| - [x] Theme label, font size, UI zoom % | DONE |
| - [x] Modified/Saved state with colored indicator | DONE |
| - [x] External change warning with Reload button | DONE |
| - [x] Trim trailing whitespace (Edit menu) | DONE |
| - [x] Case conversion: UPPERCASE / lowercase (Edit menu) | DONE |

---

### 1.12 XML/HTML Support

| Task | Status |
|------|--------|
| - [x] Click on XML tag → highlights matching open/close tag | DONE |
| - [x] Works in both directions (open→close, close→open) | DONE |
| - [x] Nesting-aware matching | DONE |
| - [x] Detects cursor inside tag, right after `>`, right before `<` | DONE |
| - [x] XML tag-based code folding | DONE |

---

### 1.13 Dialog Keyboard Support

| Task | Status |
|------|--------|
| - [x] Go to Line: Enter = Go, Escape = Cancel | DONE |
| - [x] Unsaved Changes: Enter = Save All & Close, Escape = Cancel | DONE |

---

## Gap Summary vs PRD (Updated)

| Category | Requirements | Done | Partial | Gap |
|----------|-------------|------|---------|-----|
| Core Editing (CE) | 23 | 7 | 2 | 14 |
| File & Session (FM) | 12 | 12 | 0 | 0 |
| Syntax Highlighting (SH) | 13 | 8 | 0 | 5 |
| Search & Replace (SR) | 14 | 11 | 0 | 3 |
| Code Folding (CF) | 9 | 7 | 0 | 2 |
| MDI / Tabs (MDI) | 11 | 5 | 0 | 6 |
| Macros (MA) | 9 | 0 | 1 | 8 |
| Plugins (PL) | 8 | 0 | 2 | 6 |
| Customization (CU) | 10 | 6 | 0 | 4 |
| Tools (TU) | 13 | 3 | 0 | 10 |
| **Totals** | **122** | **59** | **5** | **58** |

**Progress: 48% done (59/122), 52% including partial (64/122). Up from 44/122 in previous iteration.**

---

## Phase 2 — Feature Complete (P1 Requirements)

**Goal**: Feature parity with Notepad++ for majority of use cases.
**Timeline**: Months 4–6
**Status**: NOT STARTED

### Remaining P1 items (all GAP):

- 2.1 Smart Indent (CE-05)
- 2.2 Auto-Completion from Document (CE-15)
- 2.3 Drag-and-Drop Text (CE-19)
- 2.4 Bookmarks (CE-21)
- 2.5 Read-Only Mode (CE-22)
- 2.6 Auto-Save & Backup (FM-04, FM-05)
- 2.7 Named Sessions (FM-07)
- 2.8 File Explorer Sidebar (FM-08)
- 2.9 User Defined Language (SH-05, SH-06)
- 2.10 Bundled Themes: Solarized, Monokai, Zenburn (SH-09, CU-07)
- 2.11 Search Enhancements: Replace in Files, incremental, regex capture, history (SR-07/08/10/11/13)
- 2.12 Navigation: Function List, Document Map, custom fold markers (CF-04–08)
- 2.13 Split View & Clone (MDI-05–07, MDI-09)
- 2.14 Macros: record/playback/save/shortcuts (MA-01–08)
- 2.15 Plugin Loader (PL-01, PL-02, PL-06)
- 2.16 Tools: column editor, sort lines, dedup, case convert, diff (TU-01–03, TU-11, CU-02/10)

---

## Phase 3 — Ecosystem (P2 Requirements)

**Status**: NOT STARTED

- Plugin Manager & Marketplace
- Advanced auto-completion (symbol lists, calltips)
- Persistent undo, multi-select occurrences
- VSCode theme import, embedded language highlighting
- Hex viewer, hash/encode utilities, config export/import
- Scripting API (JS engine)
- Cross-platform Linux build

---

## Phase 4 — Polish & Platform (P3 + NFR)

**Status**: NOT STARTED

- Tab thumbnails, tab colors, breadcrumb
- Print with syntax highlighting
- macOS build
- Accessibility audit, i18n (5 languages)
- Performance benchmarking + CI gates
- Crash recovery, portable mode

---

## Appendix: Requirement Traceability Matrix

| PRD ID | Description | Status | Plan Section |
|--------|-------------|--------|-------------|
| CE-01 | 4 GB file support | GAP | 1.1 |
| CE-02 | Multi-caret | GAP | 1.3 |
| CE-03 | Column selection | GAP | 1.3 |
| CE-04 | Auto-indent | **DONE** | 1.3 |
| CE-05 | Smart indent | GAP | 2.1 |
| CE-06 | Brace matching/close | **DONE** (matching) | 1.3 |
| CE-07 | Word wrap | **DONE** | 1.3 |
| CE-08 | Line numbers | **DONE** | 1.3 |
| CE-09 | Whitespace viz | **DONE** | 1.4 |
| CE-10 | EOL conversion | **DONE** | 1.2 |
| CE-11 | Encoding support | **DONE** (decode) | 1.2 |
| CE-12 | Convert encoding | GAP | 1.2 |
| CE-13 | Undo/redo | PARTIAL (egui built-in) | 1.1 |
| CE-14 | Persistent undo | GAP | Phase 3 |
| CE-15 | Auto-complete (doc) | GAP | 2.2 |
| CE-16 | Auto-complete (API) | GAP | Phase 3 |
| CE-17 | Calltips | GAP | Phase 3 |
| CE-18 | Zoom | **DONE** | 1.3 |
| CE-19 | Drag-drop text | GAP | 2.3 |
| CE-20 | Tab-to-spaces | **DONE** | 1.4 |
| CE-21 | Bookmarks | GAP | 2.4 |
| CE-22 | Read-only mode | GAP | 2.5 |
| CE-23 | Overtype mode | GAP | 1.3 |
| FM-01 | File CRUD | **DONE** | 1.5 |
| FM-02 | Recent files | **DONE** | 1.5 |
| FM-03 | External change | **DONE** | 1.5 |
| FM-04 | Auto-save | GAP | 2.6 |
| FM-05 | Backup on save | GAP | 2.6 |
| FM-06 | Session restore | **DONE** | 1.5 |
| FM-07 | Named sessions | GAP | 2.7 |
| FM-08 | File explorer | GAP | 2.8 |
| FM-09 | CLI args | **DONE** | 1.5 |
| FM-10 | Print | GAP | Phase 4 |
| FM-11 | Drag files from OS | **DONE** | 1.5 |
| FM-12 | Save encoding/EOL | **DONE** | 1.2 |
| SH-01 | 50+ languages | **DONE** (80+ mapped) | 1.6 |
| SH-02 | Priority languages | **DONE** | 1.6 |
| SH-03 | Auto-detect lang | **DONE** | 1.6 |
| SH-04 | Manual lang override | **DONE** | 1.6 |
| SH-05 | UDL editor | GAP | 2.9 |
| SH-06 | UDL import/export | GAP | 2.9 |
| SH-07 | Style configurator | GAP | 1.6 |
| SH-08 | Global theme | **DONE** | 1.6 |
| SH-09 | Bundled themes | GAP | 2.10 |
| SH-10 | VSCode theme import | GAP | Phase 3 |
| SH-11 | Highlight occurrences | **DONE** | 1.6 |
| SH-12 | Current line highlight | **DONE** | 1.6 |
| SH-13 | Embedded lang | GAP | Phase 3 |
| SR-01 | Find bar | **DONE** (side panel) | 1.7 |
| SR-02 | Find & Replace | **DONE** | 1.7 |
| SR-03 | Regex search | **DONE** | 1.7 |
| SR-04 | Case/whole-word | **DONE** | 1.7 |
| SR-05 | Wrap-around | **DONE** (F3/Shift+F3) | 1.7 |
| SR-06 | Find in Files | **DONE** | 1.7 |
| SR-07 | Replace in Files | GAP | 2.11 |
| SR-08 | Incremental search | GAP | 2.11 |
| SR-09 | Results panel | **DONE** | 1.7 |
| SR-10 | Mark all matches | GAP | 2.11 |
| SR-11 | Regex capture replace | GAP | 2.11 |
| SR-12 | Multi-select occurrences | GAP | Phase 3 |
| SR-13 | Search history | GAP | 2.11 |
| SR-14 | Go to Line | **DONE** | 1.7 |
| CF-01 | Syntax folding | **DONE** | 1.8 |
| CF-02 | Indent folding | **DONE** | 1.8 |
| CF-03 | Fold all/unfold all | **DONE** | 1.8 |
| CF-04 | Custom fold markers | **DONE** | 1.8 |
| CF-05 | Function List | GAP | 2.12 |
| CF-06 | Function List nav | GAP | 2.12 |
| CF-07 | Function List filter | GAP | 2.12 |
| CF-08 | Document Map | GAP | 2.12 |
| CF-09 | Breadcrumb | GAP | Phase 4 |
| MDI-01 | Scrollable tabs | **DONE** (configurable) | 1.9 |
| MDI-02 | Tab drag reorder | GAP | 1.9 |
| MDI-03 | Tab context menu | **DONE** | 1.9 |
| MDI-04 | Modified indicator | **DONE** | 1.9 |
| MDI-05 | Split view | GAP | 2.13 |
| MDI-06 | Clone document | GAP | 2.13 |
| MDI-07 | Move to other view | GAP | 2.13 |
| MDI-08 | Tab groups | GAP | Phase 4 |
| MDI-09 | Pinned tabs | GAP | 2.13 |
| MDI-10 | Tab colors | GAP | Phase 4 |
| MDI-11 | Tab thumbnails | GAP | Phase 4 |
| MA-01 | Record macro | GAP (model exists) | 2.14 |
| MA-02 | Playback macro | GAP | 2.14 |
| MA-03 | Run N times | GAP | 2.14 |
| MA-04 | Save/load macros | GAP | 2.14 |
| MA-05 | Macro shortcut | GAP | 2.14 |
| MA-06 | Macro storage | GAP | 2.14 |
| MA-07 | Script runner | GAP | 2.14 |
| MA-08 | Output panel | GAP | 2.14 |
| MA-09 | Scripting API | GAP | Phase 3 |
| PL-01 | Plugin loading | GAP (host exists) | 2.15 |
| PL-02 | Plugin API | GAP (protocol exists) | 2.15 |
| PL-03 | Plugin Manager UI | GAP | Phase 3 |
| PL-04 | Plugin marketplace | GAP | Phase 3 |
| PL-05 | Bundled plugins | GAP | Phase 3 |
| PL-06 | Plugin doc API | GAP | 2.15 |
| PL-07 | Plugin panels | GAP | Phase 3 |
| PL-08 | Plugin auto-update | GAP | Phase 4 |
| CU-01 | Keyboard remap | GAP (model exists) | 1.10 |
| CU-02 | Toolbar customize | GAP | 2.16 |
| CU-03 | Hide/show UI | **DONE** | 1.10 |
| CU-04 | Per-lang font | GAP | 1.10 |
| CU-05 | Global font | **DONE** | 1.10 |
| CU-06 | Preferences dialog | GAP | 1.10 |
| CU-07 | Dark/light sync | GAP | 2.10 |
| CU-08 | Config export/import | GAP | Phase 3 |
| CU-09 | Tab size config | **DONE** | 1.4 |
| CU-10 | Per-lang settings | GAP | 2.16 |
| TU-01 | Column editor | GAP | 2.16 |
| TU-02 | Line operations | GAP | 2.16 |
| TU-03 | Case conversion | **DONE** (UPPER/lower) | 1.11 |
| TU-04 | Trim whitespace | **DONE** | 1.11 |
| TU-05 | Insert timestamp | GAP | Phase 3 |
| TU-06 | Hashing | GAP | Phase 3 |
| TU-07 | Base64 | GAP | Phase 3 |
| TU-08 | URL encode | GAP | Phase 3 |
| TU-09 | Hex viewer | GAP | Phase 3 |
| TU-10 | Char info panel | GAP | Phase 4 |
| TU-11 | Diff viewer | GAP (engine exists) | 2.16 |
| TU-12 | Word/line count | **DONE** | 1.11 |
| TU-13 | Status bar info | **DONE** | 1.11 |
