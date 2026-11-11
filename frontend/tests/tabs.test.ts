// TabManager unit tests, run with:
//   node --experimental-test-module-mocks --test frontend/tests/
// tabs.ts imports "./monaco-setup.ts" at runtime, so the whole Monaco
// surface is replaced with a minimal fake via module mocking (plain
// `node --test` cannot load monaco-setup — it pulls in the full editor).
import { test, mock } from "node:test";
import assert from "node:assert/strict";

function fakeModel(content: string) {
  let value = content;
  const model = {
    disposed: false,
    getValue: () => value,
    setValue: (next: string) => {
      value = next;
    },
    setEOL: () => {},
    dispose: () => {
      model.disposed = true;
    },
  };
  return model;
}

const fakeMonaco = {
  Uri: {
    parse: (value: string) => ({ toString: () => value }),
    file: (path: string) => ({ toString: () => path }),
  },
  editor: {
    EndOfLineSequence: { LF: 0, CRLF: 1 },
    createModel: (content: string) => fakeModel(content),
    setModelLanguage: () => {},
  },
};

mock.module("../src/monaco-setup.ts", {
  exports: { monaco: fakeMonaco },
});

const { TabManager } = await import("../src/tabs.ts");

function managerWithUntitled(count: number, pane: 1 | 2 = 1) {
  const manager = new TabManager();
  const created = [];
  for (let i = 0; i < count; i++) {
    created.push(manager.createUntitled("", pane));
  }
  return { manager, created };
}

// ---------------------------------------------------------------------------
// Untitled naming
// ---------------------------------------------------------------------------

test("untitled titles are sequential on creation", () => {
  const { manager } = managerWithUntitled(3);
  assert.deepEqual(
    manager.tabs.map((t) => t.title),
    ["Untitled 1", "Untitled 2", "Untitled 3"],
  );
});

test("closing an untitled tab never makes the next title collide", () => {
  const { manager } = managerWithUntitled(3);
  manager.close(manager.tabs[0].id);
  // The old count+1 scheme named this "Untitled 2", duplicating the tab
  // that is still open. The next free number must be used instead.
  const created = manager.createUntitled();
  assert.equal(created.title, "Untitled 4");
  const titles = manager.tabs.map((t) => t.title);
  assert.equal(new Set(titles).size, titles.length, `duplicate titles in ${titles}`);
});

test("an explicit title (session restore) is kept verbatim", () => {
  const manager = new TabManager();
  const restored = manager.createUntitled("draft", 1, "Untitled 7");
  assert.equal(restored.title, "Untitled 7");
  // Generated numbering continues past the restored number.
  assert.equal(manager.createUntitled().title, "Untitled 8");
});

test("file tabs do not affect untitled numbering", () => {
  const manager = new TabManager();
  manager.openFile({
    path: "/tmp/notes.txt",
    title: "notes.txt",
    content: "hi",
    encoding: "UTF-8",
    eol: "LF",
    language: "plaintext",
    size: 2,
    mtime_ms: 0,
  });
  assert.equal(manager.createUntitled().title, "Untitled 1");
});

// ---------------------------------------------------------------------------
// reorder (drag & drop)
// ---------------------------------------------------------------------------

test("reorder moves a tab before another in the same pane", () => {
  const { manager, created } = managerWithUntitled(3);
  let changes = 0;
  manager.onChange = () => {
    changes++;
  };
  manager.reorder(created[2].id, created[0].id, 1);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [created[2].id, created[0].id, created[1].id],
  );
  assert.equal(changes, 1);
});

test("reorder with beforeId null appends after the pane's last tab", () => {
  const { manager, created } = managerWithUntitled(3);
  manager.reorder(created[0].id, null, 1);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [created[1].id, created[2].id, created[0].id],
  );
});

test("reorder with an unknown beforeId also appends at the pane end", () => {
  const { manager, created } = managerWithUntitled(3);
  manager.reorder(created[0].id, 999, 1);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [created[1].id, created[2].id, created[0].id],
  );
});

test("reorder is stable when dropped back into the same spot", () => {
  const { manager, created } = managerWithUntitled(3);
  manager.reorder(created[1].id, created[2].id, 1);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [created[0].id, created[1].id, created[2].id],
  );
});

test("reorder onto the other pane's strip moves the tab there", () => {
  const manager = new TabManager();
  const a = manager.createUntitled("", 1);
  const b = manager.createUntitled("", 1);
  const c = manager.createUntitled("", 2);
  let changes = 0;
  manager.onChange = () => {
    changes++;
  };
  manager.reorder(a.id, c.id, 2);
  assert.equal(a.pane, 2);
  assert.deepEqual(
    manager.tabsIn(2).map((t) => t.id),
    [a.id, c.id],
  );
  // The moved tab becomes active in its new pane; the old pane falls back
  // to its first remaining tab and the active pane follows the drop.
  assert.equal(manager.activeByPane[2], a.id);
  assert.equal(manager.activeByPane[1], b.id);
  assert.equal(manager.activePane, 2);
  assert.equal(changes, 1);
});

test("reorder with an unknown tab id is a no-op", () => {
  const { manager, created } = managerWithUntitled(2);
  let changes = 0;
  manager.onChange = () => {
    changes++;
  };
  manager.reorder(999, created[0].id, 1);
  assert.equal(changes, 0);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [created[0].id, created[1].id],
  );
});

test("reorder keeps each pane's own order when panes interleave", () => {
  const manager = new TabManager();
  const a = manager.createUntitled("", 1);
  const b = manager.createUntitled("", 2);
  const c = manager.createUntitled("", 1);
  const d = manager.createUntitled("", 2);
  // Drop pane-2's first tab after pane-2's last tab.
  manager.reorder(b.id, null, 2);
  assert.deepEqual(
    manager.tabsIn(1).map((t) => t.id),
    [a.id, c.id],
  );
  assert.deepEqual(
    manager.tabsIn(2).map((t) => t.id),
    [d.id, b.id],
  );
});
