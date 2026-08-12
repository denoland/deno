// Regression test for https://github.com/denoland/deno/issues/32326
// Top-level `await test(...)` must not deadlock the module.

import test from "node:test";
import assert from "node:assert";

await test("a test", () => {
  assert.strictEqual(1 + 1, 2);
});

await test("another test", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1));
  assert.ok(true);
});
