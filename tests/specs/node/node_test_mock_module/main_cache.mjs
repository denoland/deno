// Copyright 2018-2026 the Deno authors. MIT license.
// The `cache` option controls whether repeated imports of a mocked module
// return the same instance (cache: true) or a fresh one (cache: false, the
// default).
import assert from "node:assert/strict";
import { mock } from "node:test";

// cache: true keeps a single mocked instance.
const cached = mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 1;
    },
  },
  cache: true,
});
const a = await import("./basic-esm.mjs");
const b = await import("./basic-esm.mjs");
assert.strictEqual(a, b);
assert.strictEqual(a.fn(), 1);
cached.restore();

// cache: false (explicit) gives a fresh instance every time.
const fresh = mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 2;
    },
  },
  cache: false,
});
const c = await import("./basic-esm.mjs");
const d = await import("./basic-esm.mjs");
assert.notStrictEqual(c, d);
assert.strictEqual(c.fn(), 2);
assert.strictEqual(d.fn(), 2);
fresh.restore();

console.log("cache ok");
