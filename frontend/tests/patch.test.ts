// buildUnifiedPatch unit tests (pure jsdiff wrapper — no Monaco mock needed).
import { test } from "node:test";
import assert from "node:assert/strict";

import { buildUnifiedPatch } from "../src/patch.ts";

test("patch carries both titles and a hunk header", () => {
  const patch = buildUnifiedPatch("old.txt", "one\ntwo\n", "new.txt", "one\nTWO\n");
  assert.ok(patch.includes("--- old.txt"), `titles in patch:\n${patch}`);
  assert.ok(patch.includes("+++ new.txt"));
  assert.ok(patch.includes("@@"), `hunk header in patch:\n${patch}`);
});

test("changed lines appear as minus/plus pairs", () => {
  const patch = buildUnifiedPatch("a", "keep\nchange me\n", "b", "keep\nchanged\n");
  assert.ok(patch.includes("-change me"), `minus line in patch:\n${patch}`);
  assert.ok(patch.includes("+changed"), `plus line in patch:\n${patch}`);
  assert.ok(!patch.includes("-keep"), `context line is not a change:\n${patch}`);
});

test("identical texts produce a patch with no change lines", () => {
  const patch = buildUnifiedPatch("a", "same\ntext\n", "b", "same\ntext\n");
  const changeLines = patch
    .split("\n")
    .filter((line) => line.startsWith("+") || line.startsWith("-"))
    .filter((line) => !line.startsWith("+++") && !line.startsWith("---"));
  assert.deepEqual(changeLines, [], `no changes expected:\n${patch}`);
});

test("added and removed lines are distinguishable", () => {
  const patch = buildUnifiedPatch("a", "gone\n", "b", "added\n");
  assert.ok(patch.includes("-gone"));
  assert.ok(patch.includes("+added"));
});

test("empty original and missing trailing newline are handled", () => {
  const fromEmpty = buildUnifiedPatch("a", "", "b", "new\n");
  assert.ok(fromEmpty.includes("+new"), `addition from empty:\n${fromEmpty}`);

  // jsdiff marks the no-newline case; the patch must still be produced.
  const noEol = buildUnifiedPatch("a", "x", "b", "x\ny");
  assert.ok(noEol.includes("+y"), `change without trailing newline:\n${noEol}`);
});

test("argument order: old text is the minus side, new text the plus side", () => {
  const patch = buildUnifiedPatch("old-name", "OLD-ONLY", "new-name", "NEW-ONLY");
  assert.ok(patch.includes("--- old-name"), `old title first:\n${patch}`);
  assert.ok(patch.includes("+++ new-name"), `new title second:\n${patch}`);
  assert.ok(patch.includes("-OLD-ONLY"), `old text on the minus side:\n${patch}`);
  assert.ok(patch.includes("+NEW-ONLY"), `new text on the plus side:\n${patch}`);
});
