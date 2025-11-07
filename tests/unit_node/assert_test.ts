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

Deno.test("[node/assert] deepStrictEqual with Number objects", () => {
  // Different Number objects should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new Number(1), new Number(2));
    },
    assert.AssertionError,
  );

  // Same Number objects should not throw
  assert.doesNotThrow(() => {
    assert.deepStrictEqual(new Number(1), new Number(1));
  });

  // Number object vs primitive should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new Number(1), 1);
    },
    assert.AssertionError,
  );
});

Deno.test("[node/assert] deepStrictEqual with String objects", () => {
  // Different String objects should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new String("hello"), new String("world"));
    },
    assert.AssertionError,
  );

  // Same String objects should not throw
  assert.doesNotThrow(() => {
    assert.deepStrictEqual(new String("hello"), new String("hello"));
  });

  // String object vs primitive should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new String("hello"), "hello");
    },
    assert.AssertionError,
  );
});

Deno.test("[node/assert] deepStrictEqual with Boolean objects", () => {
  // Different Boolean objects should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new Boolean(true), new Boolean(false));
    },
    assert.AssertionError,
  );

  // Same Boolean objects should not throw
  assert.doesNotThrow(() => {
    assert.deepStrictEqual(new Boolean(true), new Boolean(true));
  });

  // Boolean object vs primitive should throw
  assert.throws(
    () => {
      assert.deepStrictEqual(new Boolean(true), true);
    },
    assert.AssertionError,
  );
});
