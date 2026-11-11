# CodeEdit — Product Requirements Document
**A Modern Notepad++ Clone**
Version 1.0 | Status: Draft

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Feature Analysis & Grouping](#2-feature-analysis--grouping)
3. [Functional Requirements](#3-functional-requirements)
   - 3.1 [Core Editing](#31-core-editing)
   - 3.2 [File & Session Management](#32-file--session-management)
   - 3.3 [Syntax Highlighting & Language Support](#33-syntax-highlighting--language-support)
   - 3.4 [Search & Replace](#34-search--replace)
   - 3.5 [Code Folding & Structure Navigation](#35-code-folding--structure-navigation)
   - 3.6 [Multi-Document Interface (MDI)](#36-multi-document-interface-mdi)
   - 3.7 [Macros & Automation](#37-macros--automation)
   - 3.8 [Plugin System](#38-plugin-system)
   - 3.9 [Customization & Themes](#39-customization--themes)
   - 3.10 [Tools & Utilities](#310-tools--utilities)
4. [Non-Functional Requirements](#4-non-functional-requirements)
5. [Architecture Considerations](#5-architecture-considerations)
6. [Phased Delivery Roadmap](#6-phased-delivery-roadmap)
7. [Out of Scope](#7-out-of-scope)

---

## 1. Executive Summary

**CodeEdit** is a cross-platform, high-performance source code and text editor modeled after Notepad++. The target audience is developers, sysadmins, and power users who need a lightweight yet feature-rich editor with syntax highlighting, regex search, macro recording, and an extensible plugin ecosystem — without the overhead of a full IDE.

| Attribute        | Value                                      |
|------------------|--------------------------------------------|
| Platform         | Windows (primary), Linux, macOS (stretch)  |
| Tech Stack       | TBD (see §5)                               |
| Primary Language | C++ or Rust (native) / Electron (fallback) |
| License          | GPL-compatible open source                 |
| Target Users     | Developers, sysadmins, power users         |

---

## 2. Feature Analysis & Grouping

Notepad++ features decomposed into logical groups:

| # | Group | Key Features |
|---|-------|--------------|
| 1 | **Core Editing** | Multi-caret, column/block selection, auto-indent, brace matching, word wrap, EOL conversion, encoding detection |
| 2 | **File & Session** | New/Open/Save/Reload, multi-session, file monitoring, recent files, workspace/project |
| 3 | **Syntax Highlighting** | 80+ built-in languages, UDL (User Defined Language), color themes per language |
| 4 | **Search & Replace** | Find/Replace dialog, regex (PCRE), Find in Files, incremental search, result highlighting |
| 5 | **Code Folding** | Fold/unfold by indent or syntax, fold all/unfold all, custom fold markers |
| 6 | **MDI / Tabs** | Tabbed interface, split-view (horizontal + vertical), clone document, tab drag-and-drop |
| 7 | **Macros & Automation** | Record/playback macro, save/load macros, run on all lines/files, scripting (NppExec) |
| 8 | **Plugin System** | Plugin Manager, official plugin API, auto-update plugins, community marketplace |
| 9 | **Customization** | Keyboard shortcuts, toolbar, style configurator, theme/color scheme, font/size per language |
| 10 | **Tools & Utilities** | MD5/SHA hash, base64 encode/decode, hex viewer, column editor, line operations, document map, function list |

---

## 3. Functional Requirements

Priority legend: 🔴 P0 (MVP must-have) · 🟡 P1 (launch target) · 🟢 P2 (v2) · 🔵 P3 (nice-to-have)

---

### 3.1 Core Editing

The text editing engine is the foundation of the product. Must match or exceed Scintilla-level performance.

| ID | Requirement | Priority |
|----|-------------|----------|
| CE-01 | Support files up to 4 GB without degraded performance | 🔴 P0 |
| CE-02 | Multi-caret editing (add caret by Alt+Click or Ctrl+Alt+Down) | 🔴 P0 |
| CE-03 | Column/block selection via Alt+drag | 🔴 P0 |
| CE-04 | Auto-indent: maintain indentation level on newline | 🔴 P0 |
| CE-05 | Smart indent: increase indent after `{`, `[`, `(` based on language | 🟡 P1 |
| CE-06 | Brace/bracket/quote auto-close and matching highlight | 🔴 P0 |
| CE-07 | Word wrap with configurable wrap column | 🔴 P0 |
| CE-08 | Line numbers gutter (togglable) | 🔴 P0 |
| CE-09 | Whitespace & EOL visualization (show/hide spaces, tabs, CRLF) | 🔴 P0 |
| CE-10 | EOL conversion: Windows (CRLF), Unix (LF), macOS classic (CR) | 🔴 P0 |
| CE-11 | Encoding support: UTF-8, UTF-16 LE/BE, ANSI, detect BOM | 🔴 P0 |
| CE-12 | Convert encoding between any supported format | 🟡 P1 |
| CE-13 | Undo/redo with unlimited history (per document) | 🔴 P0 |
| CE-14 | Persistent undo across save operations | 🟢 P2 |
| CE-15 | Auto-completion of words from current document | 🟡 P1 |
| CE-16 | Auto-completion from API/symbol lists (per language) | 🟢 P2 |
| CE-17 | Calltips / parameter hints for functions | 🟢 P2 |
| CE-18 | Zoom in/out (Ctrl+Scroll) per document | 🔴 P0 |
| CE-19 | Drag-and-drop text within and between documents | 🟡 P1 |
| CE-20 | Tab-to-spaces conversion and vice versa (configurable per language) | 🔴 P0 |
| CE-21 | Mark/bookmark lines; navigate between bookmarks | 🟡 P1 |
| CE-22 | Read-only mode toggle per document | 🟡 P1 |
| CE-23 | Overtype (insert vs overwrite) toggle via Insert key | 🔴 P0 |

---

### 3.2 File & Session Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FM-01 | New, Open, Save, Save As, Save All, Close, Close All | 🔴 P0 |
| FM-02 | Recent files list (configurable max count) | 🔴 P0 |
| FM-03 | Detect external file changes; prompt reload or diff | 🔴 P0 |
| FM-04 | Auto-save on configurable interval | 🟡 P1 |
| FM-05 | Backup file on save (create `.bak` copy) | 🟡 P1 |
| FM-06 | Session save/restore: reopen all tabs with cursor positions | 🔴 P0 |
| FM-07 | Multiple named sessions; switch between sessions | 🟡 P1 |
| FM-08 | Workspace / folder tree panel (File Explorer sidebar) | 🟡 P1 |
| FM-09 | Open from command line with line number flag (`-n LINE`) | 🔴 P0 |
| FM-10 | Print document with syntax highlighting preserved | 🟢 P2 |
| FM-11 | Drag files from OS file manager into tab bar | 🔴 P0 |
| FM-12 | Save file with different encoding / EOL than detected | 🔴 P0 |

---

### 3.3 Syntax Highlighting & Language Support

| ID | Requirement | Priority |
|----|-------------|----------|
| SH-01 | Built-in highlighting for ≥50 languages at launch | 🔴 P0 |
| SH-02 | Priority languages: C, C++, C#, Java, Python, JavaScript, TypeScript, HTML, CSS, XML, JSON, YAML, Markdown, PowerShell, Bash, SQL, Go, Rust | 🔴 P0 |
| SH-03 | Language auto-detection by file extension | 🔴 P0 |
| SH-04 | Manual language override per document | 🔴 P0 |
| SH-05 | User Defined Language (UDL) editor: define keywords, operators, comment styles, delimiters | 🟡 P1 |
| SH-06 | UDL import/export as XML | 🟡 P1 |
| SH-07 | Style Configurator: change color, font, bold/italic per token class per language | 🔴 P0 |
| SH-08 | Global theme (color scheme) that applies across all languages | 🔴 P0 |
| SH-09 | Bundled themes: Default Dark, Solarized, Monokai, Zenburn, Obsidian | 🟡 P1 |
| SH-10 | Import VSCode `.json` theme format | 🟢 P2 |
| SH-11 | Highlight all occurrences of selected word | 🔴 P0 |
| SH-12 | Current line highlight | 🔴 P0 |
| SH-13 | Embedded language highlighting (e.g., JS inside HTML) | 🟢 P2 |

---

### 3.4 Search & Replace

| ID | Requirement | Priority |
|----|-------------|----------|
| SR-01 | Find bar (inline, non-modal, Ctrl+F) | 🔴 P0 |
| SR-02 | Find & Replace dialog (Ctrl+H): forward, backward, replace one/all | 🔴 P0 |
| SR-03 | Search modes: Normal, Extended (`\n`, `\t`, `\0`), PCRE Regex | 🔴 P0 |
| SR-04 | Case-sensitive and whole-word options | 🔴 P0 |
| SR-05 | Wrap-around search | 🔴 P0 |
| SR-06 | Find in Files: search across directory/file-glob with results panel | 🔴 P0 |
| SR-07 | Replace in Files with preview before applying | 🟡 P1 |
| SR-08 | Incremental search with real-time highlight | 🟡 P1 |
| SR-09 | Search result panel: clickable results with file/line/context | 🔴 P0 |
| SR-10 | Mark all matches (highlight without replacing) | 🟡 P1 |
| SR-11 | Named capture groups in regex replace (`$1`, `${name}`) | 🟡 P1 |
| SR-12 | Multi-select all occurrences of search term (creates multi-carets) | 🟢 P2 |
| SR-13 | Persistent search history (last N terms) | 🟡 P1 |
| SR-14 | Go to line / Go to column dialog | 🔴 P0 |

---

### 3.5 Code Folding & Structure Navigation

| ID | Requirement | Priority |
|----|-------------|----------|
| CF-01 | Syntax-based folding for supported languages (braces, keywords) | 🔴 P0 |
| CF-02 | Indent-based folding for Python, YAML, etc. | 🔴 P0 |
| CF-03 | Fold/unfold single block; fold/unfold all; fold level 1–8 | 🔴 P0 |
| CF-04 | Custom fold markers via comment syntax (e.g., `// {{{` / `// }}}`) | 🟡 P1 |
| CF-05 | Function List panel: parse and list all functions/classes/methods | 🟡 P1 |
| CF-06 | Function List click to navigate to definition | 🟡 P1 |
| CF-07 | Function List search filter | 🟢 P2 |
| CF-08 | Document Map: minimap of document with viewport indicator | 🟡 P1 |
| CF-09 | Breadcrumb / scope indicator in status bar | 🟢 P2 |

---

### 3.6 Multi-Document Interface (MDI)

| ID | Requirement | Priority |
|----|-------------|----------|
| MDI-01 | Tabbed document interface; scroll tab bar when overflow | 🔴 P0 |
| MDI-02 | Tab reordering via drag-and-drop | 🔴 P0 |
| MDI-03 | Tab context menu: close, close others, close to right, copy path | 🔴 P0 |
| MDI-04 | Modified-file indicator on tab (dot or asterisk) | 🔴 P0 |
| MDI-05 | Split view: split editor horizontally or vertically | 🟡 P1 |
| MDI-06 | Clone document into split pane (same buffer, two views) | 🟡 P1 |
| MDI-07 | Move document to other view (2-pane swap) | 🟡 P1 |
| MDI-08 | Tab groups / panel layout persistence | 🟢 P2 |
| MDI-09 | Pinned tabs (prevent accidental close) | 🟡 P1 |
| MDI-10 | Tab color coding (user-assignable) | 🔵 P3 |
| MDI-11 | Thumbnail tab preview on hover | 🔵 P3 |

---

### 3.7 Macros & Automation

| ID | Requirement | Priority |
|----|-------------|----------|
| MA-01 | Record macro: capture all keystrokes and commands | 🔴 P0 |
| MA-02 | Stop recording and play back macro | 🔴 P0 |
| MA-03 | Run macro N times or until end of file | 🟡 P1 |
| MA-04 | Save macro with user-defined name; load saved macros | 🟡 P1 |
| MA-05 | Assign keyboard shortcut to saved macro | 🟡 P1 |
| MA-06 | Macro storage format: XML (portable, editable by hand) | 🟡 P1 |
| MA-07 | Built-in script runner: execute shell/bat command on current file | 🟡 P1 |
| MA-08 | Output panel showing command stdout/stderr | 🟡 P1 |
| MA-09 | Scripting API (JavaScript or Python) for advanced automation | 🟢 P2 |

---

### 3.8 Plugin System

| ID | Requirement | Priority |
|----|-------------|----------|
| PL-01 | Plugin loading from DLL/shared-lib in `plugins/` directory | 🟡 P1 |
| PL-02 | Well-documented C plugin API (mirroring Notepad++ NPPM API) | 🟡 P1 |
| PL-03 | Plugin Manager UI: browse, install, update, remove plugins | 🟢 P2 |
| PL-04 | Plugin registry / marketplace (hosted manifest) | 🟢 P2 |
| PL-05 | Bundled: NppExec (command runner), Compare, JSON Viewer, XML Tools | 🟢 P2 |
| PL-06 | Plugin API: access to document text, cursor, selections, menus | 🟡 P1 |
| PL-07 | Plugin API: dockable panel registration | 🟢 P2 |
| PL-08 | Plugin auto-update on launch | 🔵 P3 |

---

### 3.9 Customization & Themes

| ID | Requirement | Priority |
|----|-------------|----------|
| CU-01 | Full keyboard shortcut remapping for all commands | 🔴 P0 |
| CU-02 | Toolbar customization: add/remove/reorder buttons | 🟡 P1 |
| CU-03 | Hide/show toolbar, menu bar, status bar, tab bar independently | 🔴 P0 |
| CU-04 | Per-language font and font size override | 🔴 P0 |
| CU-05 | Global default font and size | 🔴 P0 |
| CU-06 | Preferences dialog with organized categories | 🔴 P0 |
| CU-07 | Dark mode / light mode system sync | 🟡 P1 |
| CU-08 | Export / import full configuration as a ZIP | 🟢 P2 |
| CU-09 | Tab size and indent mode (tabs vs spaces) per language | 🔴 P0 |
| CU-10 | Language-specific settings profile | 🟡 P1 |

---

### 3.10 Tools & Utilities

| ID | Requirement | Priority |
|----|-------------|----------|
| TU-01 | Column editor: fill column with incremented numbers or static text | 🟡 P1 |
| TU-02 | Line operations: sort lines (asc/desc, case), remove duplicate lines, remove blank lines | 🟡 P1 |
| TU-03 | Convert case: UPPER, lower, Title Case, camelCase, snake_case | 🟡 P1 |
| TU-04 | Trim trailing whitespace | 🔴 P0 |
| TU-05 | Insert timestamp / date | 🟢 P2 |
| TU-06 | MD5, SHA-1, SHA-256 hash of file or selection | 🟢 P2 |
| TU-07 | Base64 encode/decode selection | 🟢 P2 |
| TU-08 | URL encode/decode selection | 🟢 P2 |
| TU-09 | Hex viewer / editor mode for binary files | 🟢 P2 |
| TU-10 | Character info panel (Unicode codepoint, HTML entity) | 🔵 P3 |
| TU-11 | Diff two open documents side by side | 🟡 P1 |
| TU-12 | Word count / character count / line count in status bar | 🔴 P0 |
| TU-13 | Status bar: current line, column, total lines, encoding, EOL, file size | 🔴 P0 |

---

## 4. Non-Functional Requirements

| Category | Requirement |
|----------|-------------|
| **Performance** | Open a 100 MB plain-text file in < 2 seconds; syntax highlighting render latency < 16 ms (60 fps scrolling) |
| **Memory** | Idle memory usage < 50 MB for a single open file; < 200 MB for 20 tabs |
| **Startup** | Cold start < 500 ms; warm start < 200 ms |
| **Stability** | Zero data loss on crash — crash-recovery should restore unsaved content |
| **Compatibility** | Windows 10/11 (x64); stretch: Linux (GTK), macOS 12+ |
| **Accessibility** | Full keyboard navigation; screen reader compatible (MSAA / AT-SPI) |
| **Portability** | Portable mode: all config stored in app directory, no registry writes |
| **Localization** | UTF-8 source; UI string externalization for i18n (5 launch languages) |
| **Security** | No network calls except plugin manager; files opened with user privileges only |
| **Licensing** | GPL v3-compatible; all bundled libraries must be compatible |

---

## 5. Architecture Considerations

### Option A — Native C++ (Recommended)
- **Rendering**: Scintilla or Scintillua as the editing component (battle-tested, used by Notepad++ itself)
- **UI Framework**: Win32 API + custom theming layer, or Qt 6 for cross-platform
- **Plugin ABI**: C-compatible DLL interface (mirrors NPPM)
- **Pros**: Smallest binary, fastest cold start, full OS integration
- **Cons**: Harder to cross-compile, UI polish requires more work

### Option B — Rust + Native UI
- **Rendering**: Custom Rope-based buffer (ropey crate) + custom rendering via wgpu or Direct2D
- **UI Framework**: Xilem / Druid / Slint
- **Pros**: Memory safety, modern tooling, great cross-platform story
- **Cons**: Editor-quality text rendering is still immature in Rust ecosystem

### Option C — Electron / Tauri (Not Recommended for MVP)
- Familiar web stack but 150–200 MB baseline memory; startup 1–3 s — violates NFR targets

**Recommendation**: Start with **Option A (C++ + Scintilla + Qt 6)** to maximize compatibility with the Notepad++ plugin ecosystem and hit performance targets. Evaluate Rust for v2 rewrite of the buffer layer.

### Key Components

```
┌─────────────────────────────────────────────────────────┐
│                     CodeEdit Process                    │
│  ┌──────────┐  ┌──────────────┐  ┌───────────────────┐ │
│  │  UI Shell │  │ Editor Core  │  │  Plugin Host      │ │
│  │  (Qt 6)  │  │ (Scintilla)  │  │  (DLL loader)     │ │
│  └────┬─────┘  └──────┬───────┘  └────────┬──────────┘ │
│       │               │                   │             │
│  ┌────▼───────────────▼───────────────────▼──────────┐  │
│  │              Application Controller               │  │
│  │  TabManager │ SessionMgr │ SearchEngine │ MacroMgr │  │
│  └────────────────────────┬──────────────────────────┘  │
│                           │                             │
│  ┌────────────────────────▼──────────────────────────┐  │
│  │              Config / Persistence Layer            │  │
│  │   XML/INI settings │ SQLite session DB │ Themes    │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 6. Phased Delivery Roadmap

### Phase 1 — MVP (Month 1–3)
**Goal**: A usable daily-driver editor for developers on Windows.

- All P0 requirements from §3.1–3.6, 3.9 (core settings), 3.10 (TU-12, TU-13, TU-04)
- 20 built-in languages, 2 themes (light/dark)
- Basic session restore, recent files
- Find/Replace with regex, Find in Files
- Installer + portable mode

### Phase 2 — Feature Complete (Month 4–6)
**Goal**: Feature parity with Notepad++ for the majority of use cases.

- All P1 requirements
- 50+ languages, 5 themes
- Split view, clone document
- Macro record/playback + save
- Plugin loader API + 3 bundled plugins
- Function List, Document Map
- UDL editor

### Phase 3 — Ecosystem (Month 7–9)
**Goal**: Healthy plugin ecosystem and power-user features.

- All P2 requirements
- Plugin Manager + marketplace
- Advanced scripting (JS/Python)
- Hex viewer, hashing utilities
- Diff viewer
- Configuration import/export
- Cross-platform (Linux build)

### Phase 4 — Polish & Platform (Month 10–12)
**Goal**: Production quality; community adoption.

- P3 items (tab thumbnails, tab colors, character panel)
- macOS build
- Accessibility audit
- i18n (5 languages)
- Performance benchmarking + profiling pass

---

## 7. Out of Scope

The following are explicitly **not** in scope for v1.0:

- Git integration / source control panel
- Integrated terminal / shell emulator
- LSP (Language Server Protocol) support — that's IDE territory
- Remote file editing (SSH/SFTP) — plugin territory
- Collaborative/multi-user editing
- Mobile or web versions
- Built-in debugger or compiler
- AI code completion

---

*Last updated: 2026-03-16*
