import { test } from "node:test";

test("no snapshot file yet", (t) => {
  t.assert.snapshot(42);
});
