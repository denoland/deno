import assert from "node:assert/strict";
import test from "node:test";
import { storage } from "./storage.mjs";

test("the imported module keeps the importing test's async context", async () => {
  await storage.run("importing test", async () => {
    const { contexts } = await import("./dependency.mjs");
    assert.deepEqual(contexts, ["importing test", "importing test"]);
    assert.equal(storage.getStore(), "importing test");
  });
  assert.equal(storage.getStore(), undefined);
});

test("the next test has no inherited async context", () => {
  assert.equal(storage.getStore(), undefined);
});
