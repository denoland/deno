// deno-lint-ignore-file

import assert from "node:assert";
import test, {
  after,
  afterEach,
  before,
  beforeEach,
  describe,
  it,
} from "node:test";

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

// --- t.before / t.after run once around all subtests (issue #35390) ---
test("t.before and t.after run once", async (t) => {
  let befores = 0;
  let afters = 0;
  t.before(() => {
    befores++;
  });
  t.after(() => {
    // Must not have run yet: after() fires after the parent test finishes.
    assert.strictEqual(afters, 0);
    afters++;
  });

  await t.test("sub 1", () => {
    // before() ran exactly once before the first subtest; after() has not run.
    assert.strictEqual(befores, 1);
    assert.strictEqual(afters, 0);
  });
  await t.test("sub 2", () => {
    assert.strictEqual(befores, 1);
    assert.strictEqual(afters, 0);
  });

  // Still inside the parent body, so after() has not fired yet.
  assert.strictEqual(afters, 0);
});

// --- nested suite beforeEach/afterEach cascade (issue #35404) ---
describe("cascade outer", () => {
  const order = [];

  beforeEach(() => order.push("before-outer"));
  afterEach(() => order.push("after-outer"));

  describe("cascade inner", () => {
    beforeEach(() => order.push("before-inner"));
    afterEach(() => order.push("after-inner"));

    it("runs every enclosing beforeEach, outermost-first", () => {
      assert.deepStrictEqual(order, ["before-outer", "before-inner"]);
    });
  });

  after(() => {
    // After the inner test finished, both suites' afterEach hooks ran,
    // innermost-first.
    assert.deepStrictEqual(order, [
      "before-outer",
      "before-inner",
      "after-inner",
      "after-outer",
    ]);
  });
});
