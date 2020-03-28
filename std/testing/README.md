# Testing

This module provides a few basic utilities to make testing easier and consistent
in Deno.

## Usage

`testing/asserts.ts` module provides range of assertion helpers. If the
assertion is false an `AssertionError` will be thrown which will result in
pretty-printed diff of failing assertion.

- `equal()` - Deep comparison function, where `actual` and `expected` are
  compared deeply, and if they vary, `equal` returns `false`.
- `assert()` - Expects a boolean value, throws if the value is `false`.
- `assertEquals()` - Uses the `equal` comparison and throws if the `actual` and
  `expected` are not equal.
- `assertNotEquals()` - Uses the `equal` comparison and throws if the `actual`
  and `expected` are equal.
- `assertStrictEq()` - Compares `actual` and `expected` strictly, therefore for
  non-primitives the values must reference the same instance.
- `assertStrContains()` - Make an assertion that `actual` contains `expected`.
- `assertMatch()` - Make an assertion that `actual` match RegExp `expected`.
- `assertArrayContains()` - Make an assertion that `actual` array contains the
  `expected` values.
- `assertThrows()` - Expects the passed `fn` to throw. If `fn` does not throw,
  this function does. Also compares any errors thrown to an optional expected
  `Error` class and checks that the error `.message` includes an optional
  string.
- `assertThrowsAsync()` - Expects the passed `fn` to be async and throw (or
  return a `Promise` that rejects). If the `fn` does not throw or reject, this
  function will throw asynchronously. Also compares any errors thrown to an
  optional expected `Error` class and checks that the error `.message` includes
  an optional string.
- `unimplemented()` - Use this to stub out methods that will throw when invoked
- `unreachable()` - Used to assert unreachable code

Basic usage:

```ts
import { assertEquals } from "https://deno.land/std/testing/asserts.ts";

Deno.test({
  name: "testing example",
  fn(): void {
    assertEquals("world", "world");
    assertEquals({ hello: "world" }, { hello: "world" });
  },
});

await Deno.runTests();
```

Short syntax (named function instead of object):

```ts
Deno.test(function example(): void {
  assertEquals("world", "world");
  assertEquals({ hello: "world" }, { hello: "world" });
});
```

Using `assertStrictEq()`:

```ts
Deno.test(function isStrictlyEqual(): void {
  const a = {};
  const b = a;
  assertStrictEq(a, b);
});

// This test fails
Deno.test(function isNotStrictlyEqual(): void {
  const a = {};
  const b = {};
  assertStrictEq(a, b);
});
```

Using `assertThrows()`:

```ts
Deno.test(function doesThrow(): void {
  assertThrows((): void => {
    throw new TypeError("hello world!");
  });
  assertThrows((): void => {
    throw new TypeError("hello world!");
  }, TypeError);
  assertThrows(
    (): void => {
      throw new TypeError("hello world!");
    },
    TypeError,
    "hello"
  );
});

// This test will not pass
Deno.test(function fails(): void {
  assertThrows((): void => {
    console.log("Hello world");
  });
});
```

Using `assertThrowsAsync()`:

```ts
Deno.test(async function doesThrow(): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      throw new TypeError("hello world!");
    }
  );
  await assertThrowsAsync(async (): Promise<void> => {
    throw new TypeError("hello world!");
  }, TypeError);
  await assertThrowsAsync(
    async (): Promise<void> => {
      throw new TypeError("hello world!");
    },
    TypeError,
    "hello"
  );
  await assertThrowsAsync(
    async (): Promise<void> => {
      return Promise.reject(new Error());
    }
  );
});

// This test will not pass
Deno.test(async function fails(): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      console.log("Hello world");
    }
  );
});
```

### Benching Usage

Basic usage:

```ts
import { runBenchmarks, bench } from "https://deno.land/std/testing/bench.ts";

bench(function forIncrementX1e9(b): void {
  b.start();
  for (let i = 0; i < 1e9; i++);
  b.stop();
});

runBenchmarks();
```

Averaging execution time over multiple runs:

```ts
bench({
  name: "runs100ForIncrementX1e6",
  runs: 100,
  func(b): void {
    b.start();
    for (let i = 0; i < 1e6; i++);
    b.stop();
  },
});
```

#### Benching API

##### `bench(benchmark: BenchmarkDefinition | BenchmarkFunction): void`

Registers a benchmark that will be run once `runBenchmarks` is called.

##### `runBenchmarks(opts?: BenchmarkRunOptions): Promise<void>`

Runs all registered benchmarks serially. Filtering can be applied by setting
`BenchmarkRunOptions.only` and/or `BenchmarkRunOptions.skip` to regular
expressions matching benchmark names.

##### `runIfMain(meta: ImportMeta, opts?: BenchmarkRunOptions): Promise<void>`

Runs specified benchmarks if the enclosing script is main.

##### Other exports

```ts
/** Provides methods for starting and stopping a benchmark clock. */
export interface BenchmarkTimer {
  start: () => void;
  stop: () => void;
}

/** Defines a benchmark through a named function. */
export interface BenchmarkFunction {
  (b: BenchmarkTimer): void | Promise<void>;
  name: string;
}

/** Defines a benchmark definition with configurable runs. */
export interface BenchmarkDefinition {
  func: BenchmarkFunction;
  name: string;
  runs?: number;
}

/** Defines runBenchmark's run constraints by matching benchmark names. */
export interface BenchmarkRunOptions {
  only?: RegExp;
  skip?: RegExp;
}
```
