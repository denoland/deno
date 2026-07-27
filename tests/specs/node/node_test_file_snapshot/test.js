// deno-lint-ignore-file
//
// Regression coverage for `t.assert.fileSnapshot(value, path[, options])`
// (Node.js issue parity: https://github.com/denoland/deno/issues/35413).
//
// The test seeds each expected snapshot file inline with `fs.writeFileSync`
// and then asserts that `fileSnapshot` reads it back and matches. That way
// the spec runs hermetically in a single `deno test` invocation without
// depending on `--update-snapshots` behavior.

import assert from "node:assert";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";

const SNAP_DIR = join(import.meta.dirname, "snapshots");
mkdirSync(SNAP_DIR, { recursive: true });

function writeSnap(path, content) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content);
}

test("default JSON serializer matches stored snapshot", (t) => {
  const path = join(SNAP_DIR, "default.json");
  writeSnap(path, JSON.stringify({ value1: 1, value2: 2 }, null, 2));
  t.assert.fileSnapshot({ value1: 1, value2: 2 }, path);
});

test("mismatch throws an AssertionError", (t) => {
  const path = join(SNAP_DIR, "mismatch.json");
  writeSnap(path, JSON.stringify({ a: 1 }, null, 2));
  assert.throws(
    () => t.assert.fileSnapshot({ a: 2 }, path),
    (err) => err && err.name === "AssertionError",
  );
});

test("missing file surfaces --test-update-snapshots hint", (t) => {
  const path = join(SNAP_DIR, "does-not-exist.json");
  try {
    rmSync(path, { force: true });
  } catch { /* */ }
  assert.throws(
    () => t.assert.fileSnapshot({ a: 1 }, path),
    (err) =>
      err &&
      err.code === "ERR_INVALID_STATE" &&
      /Cannot read snapshot file/.test(err.message) &&
      /--test-update-snapshots/.test(err.message),
  );
});

test("custom serializer pipeline is applied left-to-right", (t) => {
  const path = join(SNAP_DIR, "pipeline.txt");
  // Pipeline: identity -> wrap in tag -> uppercase.
  writeSnap(path, "<HELLO>");
  t.assert.fileSnapshot("hello", path, {
    serializers: [
      (v) => v,
      (v) => `<${v}>`,
      (v) => v.toUpperCase(),
    ],
  });
});

test("validates `path` argument", (t) => {
  assert.throws(
    () => t.assert.fileSnapshot({}, 123),
    (err) => err && err.code === "ERR_INVALID_ARG_TYPE",
  );
});

test("validates `options` argument", (t) => {
  assert.throws(
    () => t.assert.fileSnapshot({}, "ignored", "not an object"),
    (err) => err && err.code === "ERR_INVALID_ARG_TYPE",
  );
});

test("validates `options.serializers` must be an array", (t) => {
  assert.throws(
    () =>
      t.assert.fileSnapshot({}, "ignored", {
        serializers: "not an array",
      }),
    (err) =>
      err &&
      err.code === "ERR_INVALID_ARG_TYPE" &&
      /options\.serializers/.test(err.message),
  );
});

test("validates each serializer is a function", (t) => {
  assert.throws(
    () =>
      t.assert.fileSnapshot({}, "ignored", {
        serializers: [(v) => v, "not a function"],
      }),
    (err) =>
      err &&
      err.code === "ERR_INVALID_ARG_TYPE" &&
      /options\.serializers\[1\]/.test(err.message),
  );
});

test("rejects non-string serializer output", (t) => {
  assert.throws(
    () =>
      t.assert.fileSnapshot({}, "ignored", {
        serializers: [() => 42],
      }),
    (err) =>
      err &&
      err.code === "ERR_INVALID_STATE" &&
      /did not generate a string/.test(err.message),
  );
});

test("counts against t.plan", (t) => {
  const path = join(SNAP_DIR, "plan.json");
  writeSnap(path, JSON.stringify({ x: 1 }, null, 2));
  t.plan(1);
  t.assert.fileSnapshot({ x: 1 }, path);
});
