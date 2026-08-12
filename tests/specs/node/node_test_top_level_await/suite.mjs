// Regression test for https://github.com/denoland/deno/issues/32326
// Top-level `await suite(...)` must not deadlock the module either.

import { suite, test } from "node:test";
import assert from "node:assert";

await suite("a suite", () => {
  test("first", () => {
    assert.strictEqual(1 + 1, 2);
  });
  test("second", () => {
    assert.ok(true);
  });
});
