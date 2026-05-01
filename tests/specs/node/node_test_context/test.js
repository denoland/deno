// deno-lint-ignore-file

import assert from "node:assert";
import test, { describe, it, before, after, beforeEach, afterEach } from "node:test";

// --- t.name ---
test("t.name returns the test name", (t) => {
  assert.strictEqual(t.name, "t.name returns the test name");
});

// --- t.fullName ---
test("t.fullName with nested tests", async (t) => {
  await t.test("child", async (t) => {
    assert.strictEqual(t.name, "child");
    assert.strictEqual(
      t.fullName,
      "t.fullName with nested tests > child",
    );
    await t.test("grandchild", (t) => {
      assert.strictEqual(
        t.fullName,
        "t.fullName with nested tests > child > grandchild",
      );
    });
  });
});

// --- t.signal ---
test("t.signal is an AbortSignal", (t) => {
  assert.ok(t.signal instanceof AbortSignal);
  assert.strictEqual(t.signal.aborted, false);
});

// --- t.plan with assertions ---
test("t.plan with assertions passes", (t) => {
  t.plan(2);
  t.assert.ok(true);
  t.assert.strictEqual(1, 1);
});

// --- t.plan with subtests ---
test("t.plan with subtests passes", async (t) => {
  t.plan(2);
  await t.test("sub 1", () => {});
  await t.test("sub 2", () => {});
});

// --- t.plan failure (count mismatch) ---
test("t.plan fails on count mismatch", (t) => {
  t.plan(2);
  t.assert.ok(true);
  // only 1 of 2 expected -- should fail at _checkPlan()
});

// --- suite-level hooks ---
describe("suite with hooks", () => {
  let count = 0;

  before(() => {
    count = 0;
  });

  beforeEach(() => {
    count++;
  });

  afterEach(() => {});

  after(() => {
    assert.strictEqual(count, 2);
  });

  it("hook test 1", () => {});
  it("hook test 2", () => {});
});

// --- t.beforeEach / t.afterEach ---
test("t.beforeEach and t.afterEach", async (t) => {
  let count = 0;
  t.beforeEach(() => {
    count++;
  });
  t.afterEach(() => {});

  await t.test("sub 1", () => {});
  await t.test("sub 2", () => {});

  assert.strictEqual(count, 2);
});
