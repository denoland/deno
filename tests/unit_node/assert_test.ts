// Copyright 2018-2025 the Deno authors. MIT license.
import * as assert from "node:assert";

Deno.test("[node/assert] .throws() compares Error instance", () => {
  assert.throws(
    () => {
      throw new Error("FAIL");
    },
    Error,
  );

  assert.throws(
    () => {
      throw new TypeError("FAIL");
    },
    TypeError,
  );
});

Deno.test("[node/assert] deepStrictEqual(0, -0)", () => {
  assert.throws(
    () => {
      assert.deepStrictEqual(0, -0);
    },
  );
});

Deno.test("[node/assert] CallTracker correctly exported", () => {
  assert.strictEqual(typeof assert.CallTracker, "function");
  assert.strictEqual(typeof assert.default.CallTracker, "function");
  assert.strictEqual(assert.CallTracker, assert.default.CallTracker);
});

Deno.test("[node/assert] error message from strictEqual should be the same as AssertionError message", () => {
  const { message } = new assert.AssertionError({
    actual: 1,
    expected: 2,
    operator: "strictEqual",
  });

  assert.throws(
    () => {
      assert.strictEqual(1, 2);
    },
    { message },
  );
});

Deno.test("[node/assert] deepStrictEqual throws for different Number objects", () => {
  // Test case from issue #31172
  assert.throws(
    () => {
      assert.deepStrictEqual(new Number(1), new Number(2));
    },
    assert.AssertionError,
  );
});

Deno.test("[node/assert] deepStrictEqual passes for equal Number objects", () => {
  // Equal Number objects should pass
  assert.doesNotThrow(() => {
    assert.deepStrictEqual(new Number(1), new Number(1));
  });
});
