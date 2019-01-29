# Testing

This module provides a few basic utilities to make testing easier and
consistent in Deno.

## Usage

The module exports a `test` function which is the test harness in Deno. It
accepts either a function (including async functions) or an object which
contains a `name` property and a `fn` property. When running tests and
outputting the results, the name of the past function is used, or if the
object is passed, the `name` property is used to identify the test.

The module also exports `assert`, `assertEqual`, and `equal`.

`equal` is a deep comparision function, where `actual` and `expected` are
compared deeply, and if they vary, `equal` returns `false`.

The export `assert` is a function, but it is also decorated with other useful
functions:

- `assert()` - Expects a boolean value, throws if the value is `false`.
- `assert.equal()` - Uses the `equal` comparison and throws if the `actual` and
  `expected` are not equal.
- `assert.strictEqual()` - Compares `actual` and `expected` strictly, therefore
  for non-primitives the values must reference the same instance.
- `assert.throws()` - Expects the passed `fn` to throw. If `fn` does not throw,
  this function does. Also compares any errors thrown to an optional expected
  `Error` class and checks that the error `.message` includes an optional
  string.
- `assert.throwsAsync()` - Expects the passed `fn` to be async and throw (or
  return a `Promise` that rejects). If the `fn` does not throw or reject, this
  function will throw asynchronously. Also compares any errors thrown to an
  optional expected `Error` class and checks that the error `.message` includes
  an optional string.

`assertEqual()` is the same as `assert.equal()` but maintained for backwards
compatibility.

`runTests()` executes the declared tests.

Basic usage:

```ts
import {
  runTests,
  test,
  assert,
  equal
} from "https://deno.land/x/testing/mod.ts";

test({
  name: "testing example",
  fn() {
    assert(equal("world", "world"));
    assert(!equal("hello", "world"));
    assert(equal({ hello: "world" }, { hello: "world" }));
    assert(!equal({ world: "hello" }, { hello: "world" }));
    assert.equal("world", "world");
    assert.equal({ hello: "world" }, { hello: "world" });
  }
});

runTests();
```

Short syntax (named function instead of object):

```ts
test(function example() {
  assert(equal("world", "world"));
  assert(!equal("hello", "world"));
  assert(equal({ hello: "world" }, { hello: "world" }));
  assert(!equal({ world: "hello" }, { hello: "world" }));
  assert.equal("world", "world");
  assert.equal({ hello: "world" }, { hello: "world" });
});
```

Using `assert.strictEqual()`:

```ts
test(function isStrictlyEqual() {
  const a = {};
  const b = a;
  assert.strictEqual(a, b);
});

// This test fails
test(function isNotStrictlyEqual() {
  const a = {};
  const b = {};
  assert.strictEqual(a, b);
});
```

Using `assert.throws()`:

```ts
test(function doesThrow() {
  assert.throws(() => {
    throw new TypeError("hello world!");
  });
  assert.throws(() => {
    throw new TypeError("hello world!");
  }, TypeError);
  assert.throws(
    () => {
      throw new TypeError("hello world!");
    },
    TypeError,
    "hello"
  );
});

// This test will not pass
test(function fails() {
  assert.throws(() => {
    console.log("Hello world");
  });
});
```

Using `assert.throwsAsync()`:

```ts
test(async function doesThrow() {
  assert.throwsAsync(async () => {
    throw new TypeError("hello world!");
  });
  assert.throwsAsync(async () => {
    throw new TypeError("hello world!");
  }, TypeError);
  assert.throwsAsync(
    async () => {
      throw new TypeError("hello world!");
    },
    TypeError,
    "hello"
  );
  assert.throwsAsync(async () => {
    return Promise.reject(new Error());
  });
});

// This test will not pass
test(async function fails() {
  assert.throwsAsync(async () => {
    console.log("Hello world");
  });
});
```
