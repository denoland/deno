// deno-lint-ignore-file

// A top-level `test()` body must run before `process.nextTick` / `setImmediate`
// callbacks scheduled by synchronous code that appears *after* the `test()`
// declaration, matching Node.js.
// https://github.com/denoland/deno/issues/35608

import { test } from "node:test";
import assert from "node:assert";

let nextTickRan = false;
let setImmediateRan = false;

test("body runs before post-declaration nextTick/setImmediate", () => {
  assert.strictEqual(nextTickRan, false);
  assert.strictEqual(setImmediateRan, false);
});

process.nextTick(() => {
  nextTickRan = true;
});
setImmediate(() => {
  setImmediateRan = true;
});
