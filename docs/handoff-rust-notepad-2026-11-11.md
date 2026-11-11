# Handoff: rust-notepad (RustNotepad editor)

**Date:** 2026-11-11 · **Status: IMPLEMENTED AND VERIFIED through v0.2.0
scope plus the three nice-to-haves (file drag-drop, resizable split
separator, tab reordering — commit `91c3847`).**
This is not an unexecuted plan — the app is built, tested (51/51), bundled,
and live-verified in the running macOS app. The remaining items below are
roadmap/optional work plus decisions only the user can make. Every fact
below is a snapshot: re-verify before acting.

**Resume procedure:** read sections 1–2 (locks), then verify section 3
facts live, then pick tasks from section 5. Never present anything from
this file as verified system state without re-checking it.

---

## 1. Requirement decisions already settled (do NOT re-litigate)

- **Stack is final:** Tauri 2 (Rust backend) + Monaco Editor (TS/Vite
  frontend) + process-based plugin host. The repo's original egui
  prototype was deliberately deleted (git history preserves it); the
  original blueprint is archived at `docs/STACK_BLUEPRINT.md`.
- **Plugin protocol is final:** JSON-RPC 2.0, one JSON object per line on
  stdin/stdout, argv-based spawn (NEVER a shell), per-plugin capability
  gating, per-request timeout (default 5 s, clamped to 60 s), stray stdout
  lines skipped (≤32/request), wrong-id response = kill + respawn,
  `"result": null` = success. Spec: `docs/PLUGIN_PROTOCOL.md`. Any protocol
  change must update that doc + `plugins/toolkit/` +
  `src-tauri/tests/plugin_host_e2e.rs` together.
- **Data-loss policy is absolute:** a tab is never closed if the user asked
  to save and the save failed or the Save-As dialog was cancelled
  (`saveTab`/`saveTabAs` return booleans and every close path checks).
  Exit/window-close prompts per dirty tab and flushes the session first.
  Session JSON stores unsaved drafts (untitled + dirty file tabs), cursor
  position, and pane layout; restored drafts come back dirty.
- **Split view model:** Notepad++-style — two panes, each with its own tab
  strip; "Move Tab to Other View" (no clone-to-both-views feature; not
  requested).
- **Diff model:** Monaco's built-in diff engine (`createDiffEditor`) for
  visual compare (vs saved / vs tab / vs file on disk). NO custom diff
  algorithm code. Plugin-based compare returns standard unified diffs
  (toolkit `text.diff` via Python `difflib`). User explicitly wants wheels
  reused, not reinvented — keep it that way.
- **Search semantics:** literal terms are regex-escaped and matched with
  Unicode `\b` word semantics; case-insensitive by default; directory
  symlinks NEVER followed (cycle + read-escape guard); caps: 20 000
  scanned files, 2 MiB/file, depth 32.
- **Security posture:** restrictive webview CSP in
  `src-tauri/tauri.conf.json` (verified working with Monaco + Tauri IPC);
  `dompurify` forced to `3.4.17` via npm `overrides` in `package.json`
  (Monaco 0.57.0 pins a vulnerable 3.4.15). Keep both.
- **User style:** beginner-friendly, concise replies; honest trade-offs
  before recommendations; answer challenges ("why not X?") directly, then
  re-offer options; "review" ≠ "fix"; never report unexecuted work as done.

## 2. Verification gate (run after EVERY change — from CLAUDE.md)

```bash
cd ~/Projects/rust-notepad/src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets        # includes real-subprocess plugin e2e (needs python3)
cd ~/Projects/rust-notepad && npm run build   # tsc --noEmit + vite build
```

## 3. Live facts (snapshot 2026-11-11 — RE-VERIFY each)

| Fact | Value | Re-verify with |
| --- | --- | --- |
| Repo | `~/Projects/rust-notepad`, branch `rebuild` (the rebuild work), clean tree; local `main` = upstream snapshot (`3fb872d`) | `git -C ~/Projects/rust-notepad status` |
| History | rebuild = all commits after upstream tip `3fb872d` (10 at last count) | `git log --oneline 3fb872d..HEAD` |
| Pushed? | **Pushed to the fork `renee-david1979/rust-notepad`** (git remote `fork`): branch `rebuild` = the rebuild work (verified against `git rev-parse HEAD`); fork `main` = upstream tip `3fb872d`. Upstream `smartpcr/rust-notepad` (remote `origin`) is owned by Xiaodong Li (GitHub `smartpcr`, the original author) — read-only for us; he can pull from the fork if he wants the work upstream. | `git ls-remote fork main rebuild` |
| Tests | 51/51 green (43 unit + 8 real-subprocess plugin e2e in `src-tauri/tests/plugin_host_e2e.rs`) | gate commands above |
| Bundles | `target/release/bundle/macos/RustNotepad.app` (9.50 MiB), `target/release/bundle/dmg/RustNotepad_0.2.0_aarch64.dmg` (5.89 MB file on disk) | `ls target/release/bundle/` (target/ is disposable — rebuild with `npm run tauri build`) |
| Toolchain | rustc 1.96.0, node 26.13.0, npm 12.2.2, full Xcode at `/Applications/Xcode.app` | `rustc --version; node --version` |
| Frontend pins | monaco-editor 0.57.0, vite 8.3.8, typescript 7.0.2, @tauri-apps/api 2.12.4 | `package.json` + `npm ls` |
| npm audit | 0 vulnerabilities (with dompurify override) | `npm audit --audit-level=high` |
| Session file | `~/Library/Application Support/com.smartpcr.rust-notepad/session.json` (includes `split_ratio` for split widths) | `cat` it |
| App ID / version | `com.smartpcr.rust-notepad`, 0.2.0 | `src-tauri/tauri.conf.json` |
| Live UI verified | split view, diff view (saved-vs-buffer, red/green), plugin `text.word_count` round trip, draft restore + dirty dot, tab drag reorder (within-pane + cross-pane drop), split-separator resize, `split_ratio` + tab-order restore after restart — all real mouse-driven checks in the running bundle via `computer_use`. NOT manually checked yet: Finder→window file drop (tool can't drive cross-window drags) | relaunch app + `computer_use` capture |

## 4. Existing machinery to build on (exact paths)

- `src-tauri/src/encoding.rs` — encoding/EOL detection + byte-exact
  round-trip file I/O (UTF-8/BOM, UTF-16 LE/BE, Windows-1252 with `?`
  replacement), heavily unit-tested.
- `src-tauri/src/plugins/host.rs` — JSON-RPC transport + PluginHost
  (capability gate, timeout kill, desync reset, Drop-kill);
  `plugins/manager.rs` — discovery + per-plugin `Arc<Mutex<PluginHost>>`.
- `src-tauri/src/search.rs` — Find-in-Files (regex/case/word toggles,
  symlink-safe walk).
- `src-tauri/src/{watcher,commands,persistence,syntax,error}.rs` — file
  change events (canonical-path fallback for deletions), Tauri IPC surface
  (blocking work on `spawn_blocking`), session schema (serde-default
  tolerant), Monaco language mapping.
- `frontend/src/` — `main.ts` (actions/menus/shortcuts/session/restore),
  `tabs.ts` (pane-aware TabManager), `panes.ts` (split container),
  `diff.ts` (Monaco diff wrapper), `find.ts`, `panels.ts`, `menus.ts`,
  `statusbar.ts`, `monaco-setup.ts` (workers via
  `monaco-editor/editor/editor.worker?worker` style paths — 0.57 exports
  map!), `types.ts` (IPC mirrors, snake_case).
- `frontend/src/panes.ts` also owns the drag machinery: `startSeparatorDrag`
  (split resize; pointer capture + pointercancel-safe), `startTabDrag`
  (tab reorder and cross-pane drop; POINTER-based because `dragDropEnabled:
  true` swallows DOM drag events), with `TabManager.reorder`/`placeInPane`
  in `tabs.ts` and split widths persisted as `split_ratio: Option<f32>`
  (serde-default tolerant) in `persistence.rs`.
- `plugins/toolkit/` — reference plugin (Python stdlib): host.ping,
  text.word_count, text.validate_json/xml, text.format_json, text.diff,
  text.echo. Test fixture for the e2e suite.
- `docs/PLUGIN_PROTOCOL.md`, `README.md`, `CLAUDE.md`, `.github/workflows/ci.yml`.
- Build: `npm install && npm run tauri dev` / `npm run tauri build`.

## 5. Remaining tasks (IN ORDER; each self-contained)

1. **Push — DONE (fork model), do not re-ask.** The work lives on the
   fork `renee-david1979/rust-notepad`, branch `rebuild` (remote `fork`).
   Upstream `origin` (`smartpcr/rust-notepad`) belongs to the original
   author Xiaodong Li and is READ-ONLY for us: never push to `origin`
   unless he grants `renee-david1979` write access first. Routine push
   command: `git push fork rebuild`.
2. **User decision — cut a release?** `.github/workflows/release.yml` is
   the standard `tauri-apps/tauri-action@v0` template and is UNVERIFIED —
   it only runs on a `v*.*.*` tag push. If the user wants releases: tag on
   `rebuild` and `git push fork v0.2.0`, watch the workflow in the FORK,
   fix template mismatches (it builds app/dmg, msi/exe, deb/AppImage).
3. **macOS code signing / notarization** (optional): the bundle is
   unsigned (Gatekeeper: right-click → Open). Needs an Apple Developer ID
   — a user-supplied credential; do not attempt without one.
4. **Roadmap features (user picks, order advisory):** macro
   record/playback; function list / document map; plugin manager UI
   (install/enable/disable beyond current reload); hex viewer; diff-viewer
   improvements (inline unified-diff rendering); UDL editor; i18n. PRD
   with full specs: `docs/notepad-plus-plus-clone-prd.md`. For each: extend
   the existing module named in section 4 — e.g. macros hook the
   `main.ts` action registry; a hex viewer is a new pane-mode reading via
   `commands.rs`.
5. **Nice-to-haves — DONE 2026-11-11** (`91c3847`): drag-and-drop files
   onto the window (opens into the pane under the cursor; folders skipped
   with a toast; 20-file cap); resizable split separator (persisted
   `split_ratio`, clamped 0.15–0.85 in the frontend, stored verbatim by
   Rust); tab reordering (within a strip, cross-pane by dropping on the
   other strip while split). Live-verified in the bundle: reorder,
   cross-pane drop, separator resize, and restore of ratio + order after
   restart. Session restore now re-enables split for saved pane-2 tabs
   (pre-existing restore bug fixed). Caveat: the Finder→window FILE drop
   could not be driven end-to-end by `computer_use` (cross-window drag;
   window-raise not consented) — worth one manual Finder drag.
6. **Follow-ups (small):** `TabManager.reorder` unit tests via
   `node --experimental-test-module-mocks --test` with
   `mock.module('./monaco-setup')` (tdd-guide rec 2 — GUI-verified but
   not automated; tabs.ts imports monaco-setup so plain `node --test`
   cannot load it as-is); untitled tabs renumber 1/2/3 on restore
   (cosmetic, pre-existing); `release.yml` still unverified (item 2).

## 6. Constraints and pitfalls (hard-won, specific)

- **Monaco 0.57 is not the old monaco:** the package exports map rewrites
  subpaths (`.` → `esm/vs/`), so workers import as
  `monaco-editor/editor/editor.worker?worker` (NOT
  `monaco-editor/esm/vs/...`). `tabSize` is a MODEL option
  (`model.updateOptions`), not an editor option. `findMatches` takes
  `searchScope: IRange | IRange[]` (pass `model.getFullModelRange()`).
- **`@tauri-apps/api` event module is singular:** `@tauri-apps/api/event`,
  not `.../events`. Tauri command args: use single-word names (`path`,
  `params`, `options`) — frontend `ipc.ts` mirrors `types.ts` snake_case
  fields exactly; a rename on either side breaks silently at runtime.
- **Session JSON must stay serde-default tolerant** (`#[serde(default)]`
  on every new field) or old sessions stop loading (there is a test for
  this).
- **The write/edit tools refuse to overwrite files modified externally
  (e.g. by `cargo fmt`) without a fresh `read_file` first** — run `cargo
  fmt` deliberately at the END of edits, not between them.
- **GUI verification:** terminal has no Screen Recording/Accessibility
  TCC — screenshots and clicks must go through the `computer_use` tool.
  Webview AX exposure is flaky: ALWAYS `capture` fresh immediately before
  clicking an element index (indices invalidate between captures), and
  prefer `element` clicks over coordinates.
- **Plugin e2e tests need `python3`** and skip politely without it; they
  spawn real processes — do not run them under a cargo lock held by a
  concurrent `tauri build` (builds and tests serialize on `target/`).
- **Long builds** (`npm run tauri build`, ~3–5 min) should run as
  background terminal jobs with log files (`/tmp/rn-*.log` pattern was
  used) — never poll; continue other work and read the log on completion.
- **CLAUDE.md is a protected agent-instruction file** — edits to it were
  blocked by approval; leave as-is (its fmt/clippy rule is still accurate).
- **`computer_use` coordinate spaces:** drag/click `coordinate=` is in
  WINDOW SCREENSHOT pixels (1:1 with the capture image); the SOM `AX*`
  bounds are SCREEN coordinates — screenshot y = AX y − window origin y
  (a 54 px offset cost one missed drag). `drag` requires
  `delivery_mode: 'foreground'` on macOS (background drag returns
  `background_unavailable`); `focus_app` with `raise_window: true` needs
  explicit user consent. For pixel-exact hit targets (e.g. the 5 px
  separator strip), scan the capture with PIL from `~/crawl4ai-venv`
  rather than trusting vision estimates.
