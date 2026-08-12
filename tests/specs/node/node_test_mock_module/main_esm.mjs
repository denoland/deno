// Copyright 2018-2026 the Deno authors. MIT license.
// Mocking an ESM module imported by an ESM importer: named exports and the
// default export are exposed independently, the default cache behavior gives a
// fresh module on every import, and restore() brings the original back.
import assert from "node:assert/strict";
import { mock } from "node:test";

const original = await import("./basic-esm.mjs");
assert.strictEqual(original.string, "original esm string");
assert.strictEqual(original.fn, undefined);

const ctx = mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 42;
    },
  },
  defaultExport: "mock default",
});

const mocked = await import("./basic-esm.mjs");
assert.strictEqual(mocked.string, undefined);
assert.strictEqual(mocked.fn(), 42);
assert.strictEqual(mocked.default, "mock default");

// Not cached by default: every import is a fresh module namespace.
assert.notStrictEqual(mocked, await import("./basic-esm.mjs"));

ctx.restore();

const restored = await import("./basic-esm.mjs");
assert.strictEqual(restored.string, "original esm string");
assert.strictEqual(restored.fn, undefined);

console.log("esm ok");
