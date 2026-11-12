# Handoff: rust-notepad (RustNotepad editor)

**Date:** 2026-11-11 (updated 2026-11-12) · **Status: IMPLEMENTED AND
VERIFIED through v0.2.0 scope plus the three nice-to-haves (file
drag-drop, resizable split separator, tab reordering — `91c3847`), the
follow-ups (`fe90eaa`), the v0.2.0 release run (`666c386`), the quit-
interception fix (`7d09a7f`), the plugin manager (`d841009`), and the
diff-viewer improvements (`788ea3c`). Tip: `788ea3c`.**
This is not an unexecuted plan — the app is built, tested (61 Rust + 17
node tests), bundled, and live-verified in the running macOS app. The
remaining items below are roadmap/optional work plus decisions only the
user can make. Every fact below is a snapshot: re-verify before acting.

**Resume procedure (cold session):** read sections 1–2 (locks), then
verify section 3 facts live, then take the top item from section 5 (the
user picks which; the TODO block at its top lists what is genuinely
open). Never present anything from this file as verified system state
without re-checking it. The file is self-contained; named skills
(`screenshot-free-gui`, `tauri-app-development`) are optional depth, not
required reading.

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
  visual compare (vs saved / vs tab / vs file on disk) with an options
  toolbar (inline/side-by-side, whitespace, collapse-unchanged). NO
  custom diff algorithm code: unified-patch TEXT export is jsdiff
  (`frontend/src/patch.ts`, user-approved option "2" 2026-11-12);
  plugin-based compare returns standard unified diffs (toolkit
  `text.diff` via Python `difflib`, kept as the plugin-side demo).
  User explicitly wants wheels reused, not reinvented — keep it that way.
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
| History | rebuild = all commits after upstream tip `3fb872d` (20 at `788ea3c`) | `git log --oneline 3fb872d..HEAD` |
| Pushed? | **Pushed to the fork `renee-david1979/rust-notepad`** (git remote `fork`): branch `rebuild` = the rebuild work (verified against `git rev-parse HEAD`); fork `main` = upstream tip `3fb872d`. Upstream `smartpcr/rust-notepad` (remote `origin`) is owned by Xiaodong Li (GitHub `smartpcr`, the original author) — read-only for us; he can pull from the fork if he wants the work upstream. | `git ls-remote fork main rebuild` |
| Tests | 61 Rust green (53 unit + 8 real-subprocess plugin e2e in `src-tauri/tests/plugin_host_e2e.rs`) + 17 node unit tests (`npm test`: tsc type-check + `node --test`) | gate commands above + `npm test` |
| Bundles | `target/release/bundle/macos/RustNotepad.app`, `target/release/bundle/dmg/RustNotepad_0.2.0_aarch64.dmg` — built from the `788ea3c` tree (current) during the diff live-verify | `ls target/release/bundle/` (target/ is disposable — rebuild with `npx tauri build`) |
| Release | tag `v0.2.0` on `666c386`, pushed to `fork`; Release workflow green on all 3 OS legs (run `43094512673`); **DRAFT GitHub release with 7 assets** (dmg, app.tar.gz, deb, rpm, AppImage, msi, exe) — **publish manually** on the fork's Releases page. `gh release` CANNOT see it: the fine-grained PAT lacks Contents scope (drafts invisible; `POST /releases` = 403). Verify via run logs ("Found draft release with tag v0.2.0") or the owner's browser | release workflow logs / owner's browser |
| Toolchain | rustc 1.96.0, node 26.13.0, npm 12.2.2, full Xcode at `/Applications/Xcode.app` | `rustc --version; node --version` |
| Frontend pins | monaco-editor 0.57.0, vite 8.3.8, typescript 7.0.2, @tauri-apps/api 2.12.4, diff (jsdiff) 9.0.0 | `package.json` + `npm ls` |
| npm audit | 0 vulnerabilities (with dompurify override) | `npm audit --audit-level=high` |
| Session file | `~/Library/Application Support/com.smartpcr.rust-notepad/session.json` (includes `split_ratio` for split widths) | `cat` it |
| App ID / version | `com.smartpcr.rust-notepad`, 0.2.0 | `src-tauri/tauri.conf.json` |
| Live UI verified | split view; diff view (saved-vs-buffer, vs-tab, vs-disk with toolbar toggles, Copy-as-patch verified via `pbpaste`, banner Compare + hide-on-success); plugin manager (folder-picker install of a fixture, capability invoke end-to-end, disable/enable round-trips with `plugins.json` checks, remove with confirm modal + disk cleanup); quit flow prompts per dirty tab in order (window close AND ⌘Q/menu-quit); draft restore + dirty dot; tab drag reorder (within-pane + cross-pane); split-separator resize + `split_ratio`/tab-order restore. NOT manually checked: Finder→window file drop (tools can't drive cross-window drags — worth one manual drag) | relaunch app + `computer_use` (AX mode) or skill `screenshot-free-gui` |

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
  `diff.ts` (Monaco diff wrapper + options toolbar + patch copy/export),
  `find.ts`, `panels.ts`, `menus.ts`,
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
- Build: `npm install && npx tauri dev` / `npx tauri build` (there is NO
  `tauri` npm script — `npm run tauri` / `npm tauri` both fail).
- `frontend/src/patch.ts` — pure unified-patch builder (jsdiff v9
  `createTwoFilesPatch`), used by the diff view's Copy-as-patch / Export
  actions and unit-tested in `frontend/tests/patch.test.ts`.
- `frontend/tests/tabs.test.ts` + `frontend/tests/patch.test.ts` +
  `tsconfig.test.json` — frontend unit tests via `npm test`; mocked
  Monaco (see the `mock.module` pitfall in section 6).

## 5. Remaining tasks (IN ORDER; each self-contained)

**📌 OPEN as of `788ea3c` — pick with the user (nothing in flight; tree
clean and pushed):** (a) publish the draft v0.2.0 release — item 2, the
user's browser only; (b) roadmap features — item 4's list, the user
picks; (c) one manual Finder→window file drag — item 5's caveat;
(d) parked nits — end of section 6. Everything else is DONE below.

**✅ DONE 2026-11-12: diff-viewer improvements** (`788ea3c`). Scope (user picked "2" = Monaco built-in knobs + jsdiff):
`diff.ts` toolbar (Inline/Side-by-side, Ignore-whitespace,
Collapse-unchanged toggles + `renderIndicators`); "Copy as patch" /
"Export .patch…" via the pure `patch.ts` (jsdiff `createTwoFilesPatch`,
6 unit tests in `frontend/tests/patch.test.ts`); external-change banner
gained a **Compare** button (`comparePathWithTab` → `compareWithDisk`)
closing PRD FM-03. ECC reviews: typescript-reviewer REQUEST CHANGES
(unhandled promise rejections outside try — HIGH; dispose listener leak;
magic-height CSS) + code-reviewer APPROVE WITH NITS (banner Compare
opened in the active pane instead of the tab's own; banner dismissed on
failure; compare-path duplication) — ALL findings fixed (full try/catch
+ logged errors, dispose clears host DOM, flex layout, tabs.activate
before diff, banner hides only on success, shared comparePathWithTab
helper with model-dispose-on-throw, 2 edge tests incl. argument-order
guard). Deferred nits: toggle-state persistence across views; the
toolkit `text.diff` plugin path stays as the plugin-side compare demo.
Live-verified (screenshot-free method): diff toolbar renders, Inline
toggle flips the label, Copy-as-patch lands a real unified patch in the
clipboard (`pbpaste` verified), file-changed fires both paths (clean →
auto-reload + toast; dirty → banner), banner Compare opens
"(on disk) ↔ buffer" diff and dismisses only on success. Gate green:
61 Rust + 17 node tests.

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
   NOTE: the plugin manager UI (install/enable/disable/remove) from this
   roadmap was DONE 2026-11-12 — Plugins panel toolbar + per-plugin
   toggles, plugins.json disabled list, staged/atomic install with path
   guards, remove limited to installed plugins. Remaining roadmap is
   item 4's list.
4. **Roadmap features (user picks, order advisory):** macro
   record/playback; function list / document map; hex viewer; UDL
   editor; i18n. (Diff-viewer improvements were DONE 2026-11-12 — see
   the block at the top of this section.) PRD
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
- **GUI verification (updated 2026-11-12): use the screenshot-free AX
  method — skill `screenshot-free-gui`** (AX text captures, directed
  background clicks that never steal focus, osascript for native panels,
  state truth from files/CLI). osascript now HOLDS Accessibility (user
  grant 2026-11-12; probe first). Traps that skill records:
  `capture_after=true` always returns screenshots (never use it when
  avoiding them); System Events `entire contents` is flaky on webview
  content (prefer the driver's AX tree there, osascript for native
  panels); always make scripts RETURN what they clicked. Driver basics
  still hold: fresh `capture` before every `element` click (indices die
  with captures), `element` over coordinates.
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
- **Session JSON surgery for tests:** draft CONTENT lives under nested
  keys (a key-based scrub missed it once — search by VALUE for test
  strings), and `recent_files` keeps paths after their tabs close —
  clean both when removing test fixtures. Tab entries and recents are
  separate structures; check the whole JSON for residue.
- **Monaco silently drops synthetic typing** (CGEvent `type` reports 0
  chars delivered on its textarea). To dirty a buffer programmatically,
  use the app's own actions: `set_value` on the find/replace AXTextFields
  + Replace All works (live-proven). `set_value` on webview fields goes
  through the AX API and is not subject to the event filtering.
- **Native dialogs (NSOpenPanel etc.): drive them with ONE atomic
  osascript script** (set frontmost, ⌘⇧G, type path, Return — with
  delays), not interleaved per-key tool calls: `delivery_mode:
  'foreground'` restores focus between calls and desyncs the sheet.
  Sheets don't appear in `get name of windows` — `list_windows` sees
  them. A file path in go-to-folder opens the file directly.
- **Parked nits (deliberately deferred, low value):** diff toolbar
  toggle-state persistence across views; consolidating the toolkit
  `text.diff` plugin path with the jsdiff patch export (kept separate
  for now — the plugin path is the live plugin-host demo).
