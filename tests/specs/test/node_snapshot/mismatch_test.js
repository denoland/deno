import { test } from "node:test";

test("mismatch", (t) => {
  t.assert.snapshot("actual value");
});
