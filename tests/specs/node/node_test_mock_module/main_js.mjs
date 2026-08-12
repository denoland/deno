// Copyright 2018-2026 the Deno authors. MIT license.
// A `.js` file in a "type": "module" package is ESM: its default export and
// named exports must be exposed independently (not merged like a CJS module),
// and export names that are not valid identifiers (e.g. "foo-bar") must work.
import assert from "node:assert/strict";
import { mock } from "node:test";

const original = await import("./basic-js.js");
assert.strictEqual(original.string, "original js string");

const ctx = mock.module("./basic-js.js", {
  namedExports: {
    fn() {
      return 42;
    },
    "foo-bar": "hyphenated",
  },
  defaultExport: "mock default",
});

const mocked = await import("./basic-js.js");
assert.strictEqual(mocked.string, undefined);
assert.strictEqual(mocked.fn(), 42);
// Non-identifier export names are reachable via the namespace.
assert.strictEqual(mocked["foo-bar"], "hyphenated");
// ESM default is independent, not the merged named-exports object.
assert.strictEqual(mocked.default, "mock default");

ctx.restore();

const restored = await import("./basic-js.js");
assert.strictEqual(restored.string, "original js string");

console.log("js ok");
