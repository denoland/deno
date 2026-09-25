import { suite, test } from "node:test";

// Mirrors the reproduction from denoland/deno#35402.
suite("suite of snapshot tests", () => {
  test("snapshot test", (t) => {
    t.assert.snapshot(5);
  });
});

test("multiple snapshots", (t) => {
  t.assert.snapshot({ a: 1, b: [2, 3] });
  t.assert.snapshot("hello");
});
