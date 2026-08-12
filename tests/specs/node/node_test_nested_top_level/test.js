// deno-lint-ignore-file

// Calling the top-level `test()` from inside another test's body should
// register a subtest of the running test, matching Node.js, instead of
// throwing "Nested Deno.test() calls are not supported".
// https://github.com/denoland/deno/issues/35391

import assert from "node:assert";
import { test } from "node:test";

test("outer", async (t) => {
  // Not awaited, just like real-world suites (e.g. fastify) do it.
  test("inner declared with top-level test()", () => {
    assert.strictEqual(1 + 1, 2);
  });
});

test("outer with mixed nesting", async (t) => {
  let ran = false;
  // `t.test()` keeps working alongside the routed top-level `test()`.
  await t.test("via t.test()", () => {
    ran = true;
  });
  assert.ok(ran);

  test("via top-level test()", (t2) => {
    test("nested two levels deep", () => {
      assert.strictEqual(t2.name, "via top-level test()");
    });
  });
});

test("failing nested test fails its parent", () => {
  test("nested failure", () => {
    throw new Error("boom");
  });
});
