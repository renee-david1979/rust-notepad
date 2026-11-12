# Handoff: rust-notepad (RustNotepad editor)

**Date:** 2026-11-11 · **Status: IMPLEMENTED AND VERIFIED through v0.2.0
scope plus the three nice-to-haves (file drag-drop, resizable split
separator, tab reordering — commit `91c3847`), the follow-ups and the
v0.2.0 release run (commits `fe90eaa`, `666c386`).**
This is not an unexecuted plan — the app is built, tested (52 Rust + 11
node tests), bundled, and live-verified in the running macOS app. The
remaining items below are roadmap/optional work plus decisions only the
user can make. Every fact below is a snapshot: re-verify before acting.

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
  Exit/window-close prompts per dirty tab and flushes the session first —
  including ⌘Q / Quit menu since `7d09a7f`: the native Quit item is custom
  (tao has no `applicationShouldTerminate`, so the stock Quit terminates
  uninterceptably and `ExitRequested` fires only after the window is gone)
  and routes through the frontend's guarded flow; `exit_app` +
  `EXIT_APPROVED` is the only way out. Live-verified (prompt, Cancel keeps
  the app alive, clean exit on approval). KNOWN tao LIMITATION: Dock-Quit
  and system logout/shutdown terminate without prompting — the debounced
  draft autosave is the safety net there. Full design rationale:
  skill `tauri-app-development` (`references/exit-guard.md`).
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
npm test          # frontend unit tests (tsc + node --test)
```

## 3. Live facts (snapshot 2026-11-11 — RE-VERIFY each)

| Fact | Value | Re-verify with |
| --- | --- | --- |
| Repo | `~/Projects/rust-notepad`, branch `rebuild` (the rebuild work), clean tree; local `main` = upstream snapshot (`3fb872d`) | `git -C ~/Projects/rust-notepad status` |
| History | rebuild = all commits after upstream tip `3fb872d` (13 at last count) | `git log --oneline 3fb872d..HEAD` |
| Pushed? | **Pushed to the fork `renee-david1979/rust-notepad`** (git remote `fork`): branch `rebuild` = the rebuild work (verified against `git rev-parse HEAD`); fork `main` = upstream tip `3fb872d`. Upstream `smartpcr/rust-notepad` (remote `origin`) is owned by Xiaodong Li (GitHub `smartpcr`, the original author) — read-only for us; he can pull from the fork if he wants the work upstream. | `git ls-remote fork main rebuild` |
| Tests | 52/52 Rust green (44 unit + 8 real-subprocess plugin e2e in `src-tauri/tests/plugin_host_e2e.rs`) + 11 node unit tests (`npm test`: tsc type-check + `node --test`) | gate commands above + `npm test` |
| Bundles | `target/release/bundle/macos/RustNotepad.app` (9.50 MiB), `target/release/bundle/dmg/RustNotepad_0.2.0_aarch64.dmg` (5.89 MB file on disk) | `ls target/release/bundle/` (target/ is disposable — rebuild with `npm run tauri build`) |
| Release | tag `v0.2.0` on `666c386`, pushed to `fork`; Release workflow green on all 3 OS legs (run `43094512673`); **DRAFT GitHub release with 7 assets** (dmg, app.tar.gz, deb, rpm, AppImage, msi, exe) — **publish manually** on the fork's Releases page. `gh release` CANNOT see it: the fine-grained PAT lacks Contents scope (drafts invisible; `POST /releases` = 403). Verify via run logs ("Found draft release with tag v0.2.0") or the owner's browser | release workflow logs / owner's browser |
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
- `frontend/tests/tabs.test.ts` + `tsconfig.test.json` — `TabManager`
  unit tests (reorder, untitled naming) via `npm test`; mocked Monaco
  (see the `mock.module` pitfall in section 6).

## 5. Remaining tasks (IN ORDER; each self-contained)

1. **Push — DONE (fork model), do not re-ask.** The work lives on the
   fork `renee-david1979/rust-notepad`, branch `rebuild` (remote `fork`).
   Upstream `origin` (`smartpcr/rust-notepad`) belongs to the original
   author Xiaodong Li and is READ-ONLY for us: never push to `origin`
   unless he grants `renee-david1979` write access first. Routine push
   command: `git push fork rebuild`.
2. **Release — DONE 2026-11-11 (draft awaiting manual publish).** User
   chose to cut a release. `release.yml` (tauri-action template) is now
   exercised and green on all three OS legs (run `43094512673`). Mismatches
   it needed fixed: `bundle.targets: "all"` in `tauri.conf.json` (was
   macOS-only `["app","dmg"]` — zero bundles on Linux/Windows → "No
   artifacts were found"), `icons/icon.ico` listed in `bundle.icon`
   (Windows msi/nsis require a listed .ico), Node 26 pins in both
   workflows. Tag `v0.2.0` = `666c386` on the fork (an earlier stale local
   + remote tag pointed into upstream history at `bc16c6f` — corrected).
   The workflow attaches 7 assets (dmg, app.tar.gz, deb, rpm, AppImage,
   msi, exe) to a **DRAFT** release — **the user publishes it** on
   https://github.com/renee-david1979/rust-notepad/releases (our PAT can't
   see drafts, section 6). Future releases: tag + `git push fork <tag>`.
3. **macOS code signing / notarization — user decision 2026-11-11: SKIP
   (unsigned is fine).** The bundle stays unsigned (Gatekeeper:
   right-click → Open). Revisit only with a user-supplied Apple Developer
   ID; do not attempt without one.
4. **Roadmap features (user picks, order advisory):** macro
   record/playback; function list / document map; plugin manager UI
   (install/enable/disable beyond current reload); hex viewer; diff-viewer
   improvements (inline unified-diff rendering); UDL editor; i18n. PRD
   with full specs: `docs/notepad-plus-plus-clone-prd.md`. For each: extend
   the existing module named in section 4 — e.g. macros hook the
   `main.ts` action registry; a hex viewer is a new pane-mode reading via
   `commands.rs`. User decision 2026-11-11: none for now.
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
6. **Follow-ups — DONE 2026-11-11 (`fe90eaa`).** `TabManager.reorder` +
   untitled-naming unit tests in `frontend/tests/tabs.test.ts` (11 tests;
   `npm test` = type-check via `tsconfig.test.json` +
   `node --experimental-test-module-mocks --test`); untitled tab titles
   now persist in the session (`SavedTab.title`, serde-default tolerant)
   so names survive restarts instead of being renumbered 1/2/3, and
   `createUntitled` numbering is collision-free (next free "Untitled N",
   safe integers only). Live-verified in the bundle 2026-11-11: after
   closing "Untitled 1" the next new tab is "Untitled 4" (no duplicate
   names), and a restart keeps titles 2/3/4 (no 1/2/3 renumbering) with
   drafts restored dirty and their content intact. The ⌘Q prompt gap that
   this check uncovered was FIXED in `7d09a7f` (custom native Quit item →
   guarded flow; see section 1). ECC
   reviews: code-reviewer, rust-reviewer, typescript-reviewer all
   APPROVE WITH NITS, all nits addressed (tests type-checked + in CI,
   restore routes saved titles through `createUntitled`'s title param).
   The rec-2 plan was corrected on the way: see the `mock.module` pitfall
   in section 6.

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
- **Node's `mock.module` cannot resolve extensionless specifiers** (its
  loader resolves like a normal ESM import — `mock.module('./monaco-setup')`
  fails `ERR_MODULE_NOT_FOUND`). Mocked modules must be imported with an
  explicit `.ts` extension (see `tabs.ts` import) plus
  `allowImportingTsExtensions` in tsconfig. The mock option is `exports`
  (`namedExports` is deprecated). Requires Node ≥ 23.6 (type stripping) —
  `package.json` `engines` says so; CI/release workflows pin Node 26.
- **A fork runs NO workflows until Actions is activated on the fork**
  ("Workflows don't run in forked repositories by default" — docs.github
  events-that-trigger-workflows), even though `GET /actions/permissions`
  already reports `enabled: true`. Symptom: tag pushes create zero runs.
  Fix: `gh api -X PUT repos/<owner>/<repo>/actions/permissions -F
  enabled=true -f allowed_actions=all`, then delete + re-push the tag
  (a force-updated existing tag did not fire a push event).
- **The fine-grained PAT has Administration but NOT Contents scope.**
  `gh release` shows empty/"not found" for drafts and `POST /releases` is
  403 ("Resource not accessible by personal access token") — that does
  NOT mean a release is missing. The workflow's `GITHUB_TOKEN`
  (contents: write) manages releases; verify via its logs ("Found draft
  release with tag ...") or the owner's browser.
- **Cross-platform release bundles need `bundle.targets: "all"`** in
  `tauri.conf.json` (Tauri filters by platform automatically), and
  Windows bundling fails ("Couldn't find a .ico icon") unless
  `icons/icon.ico` is listed in `bundle.icon` — the bundler only
  considers listed icons even when the file exists.
- **A macOS runner job can sit queued ~15 min and get auto-cancelled with
  zero steps** (seen once; an identical re-run scheduled fine). Re-run or
  re-push the tag before suspecting the workflow.
- **Before GUI-testing the bundle, `ps` for a stale instance** — `open`
  silently focuses an existing process still running the OLD binary, and
  the test then exercises stale code. Quit it (⌘Q) and relaunch.
- **Exit-guard design and the tao termination model** (why the stock Quit
  can't be intercepted, the custom-menu fix, Dock-Quit limitation) is
  captured in the `tauri-app-development` skill — load it before touching
  quit/close/menu code here.
