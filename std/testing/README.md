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
- `assertStrictEquals()` - Compares `actual` and `expected` strictly, therefore
  for non-primitives the values must reference the same instance.
- `assertStringIncludes()` - Make an assertion that `actual` includes
  `expected`.
- `assertMatch()` - Make an assertion that `actual` match RegExp `expected`.
- `assertNotMatch()` - Make an assertion that `actual` not match RegExp
  `expected`.
- `assertArrayIncludes()` - Make an assertion that `actual` array includes the
  `expected` values.
- `assertObjectMatch()` - Make an assertion that `actual` object match
  `expected` subset object
- `assertThrows()` - Expects the passed `fn` to throw. If `fn` does not throw,
  this function does. Also compares any errors thrown to an optional expected
  `Error` class and checks that the error `.message` includes an optional
  string.
- `assertThrowsAsync()` - Expects the passed `fn` to be async and throw (or
  return a `Promise` that rejects). If the `fn` does not throw or reject, this
  function will throw asynchronously. Also compares any errors thrown to an
  optional expected `Error` class and checks that the error `.message` includes
  an optional string.
- `unimplemented()` - Use this to stub out methods that will throw when invoked.
- `unreachable()` - Used to assert unreachable code.

Basic usage:

```ts
import { assertEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";

Deno.test({
  name: "testing example",
  fn(): void {
    assertEquals("world", "world");
    assertEquals({ hello: "world" }, { hello: "world" });
  },
});
```

Short syntax (named function instead of object):

```ts
Deno.test("example", function (): void {
  assertEquals("world", "world");
  assertEquals({ hello: "world" }, { hello: "world" });
});
```

Using `assertStrictEquals()`:

```ts
Deno.test("isStrictlyEqual", function (): void {
  const a = {};
  const b = a;
  assertStrictEquals(a, b);
});

// This test fails
Deno.test("isNotStrictlyEqual", function (): void {
  const a = {};
  const b = {};
  assertStrictEquals(a, b);
});
```

Using `assertThrows()`:

```ts
Deno.test("doesThrow", function (): void {
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
    "hello",
  );
});

// This test will not pass.
Deno.test("fails", function (): void {
  assertThrows((): void => {
    console.log("Hello world");
  });
});
```

Using `assertThrowsAsync()`:

```ts
Deno.test("doesThrow", async function (): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      throw new TypeError("hello world!");
    },
  );
  await assertThrowsAsync(async (): Promise<void> => {
    throw new TypeError("hello world!");
  }, TypeError);
  await assertThrowsAsync(
    async (): Promise<void> => {
      throw new TypeError("hello world!");
    },
    TypeError,
    "hello",
  );
  await assertThrowsAsync(
    async (): Promise<void> => {
      return Promise.reject(new Error());
    },
  );
});

// This test will not pass.
Deno.test("fails", async function (): Promise<void> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      console.log("Hello world");
    },
  );
});
```

## Benching

With this module you can benchmark your code and get information on how is it
performing.

### Basic usage:

Benchmarks can be registered using the `bench` function, where you can define a
code, that should be benchmarked. `b.start()` has to be called at the start of
the part you want to benchmark and `b.stop()` at the end of it, otherwise an
error will be thrown.

After that simply calling `runBenchmarks()` will benchmark all registered
benchmarks and log the results in the commandline.

```ts
import {
  bench,
  runBenchmarks,
} from "https://deno.land/std@$STD_VERSION/testing/bench.ts";

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

Running specific benchmarks using regular expressions:

```ts
runBenchmarks({ only: /desired/, skip: /exceptions/ });
```

### Processing benchmark results

`runBenchmarks()` returns a `Promise<BenchmarkRunResult>`, so you can process
the benchmarking results yourself. It contains detailed results of each
benchmark's run as `BenchmarkResult` s.

```ts
runBenchmarks()
  .then((results: BenchmarkRunResult) => {
    console.log(results);
  })
  .catch((error: Error) => {
    // ... errors if benchmark was badly constructed.
  });
```

### Processing benchmarking progress

`runBenchmarks()` accepts an optional progress handler callback function, so you
can get information on the progress of the running benchmarking.

Using `{ silent: true }` means you wont see the default progression logs in the
commandline.

```ts
runBenchmarks({ silent: true }, (p: BenchmarkRunProgress) => {
  // initial progress data.
  if (p.state === ProgressState.BenchmarkingStart) {
    console.log(
      `Starting benchmarking. Queued: ${p.queued.length}, filtered: ${p.filtered}`,
    );
  }
  // ...
});
```

#### Benching API

##### `bench(benchmark: BenchmarkDefinition | BenchmarkFunction): void`

Registers a benchmark that will be run once `runBenchmarks` is called.

##### `runBenchmarks(opts?: BenchmarkRunOptions, progressCb?: (p: BenchmarkRunProgress) => void | Promise<void>): Promise<BenchmarkRunResult>`

Runs all registered benchmarks serially. Filtering can be applied by setting
`BenchmarkRunOptions.only` and/or `BenchmarkRunOptions.skip` to regular
expressions matching benchmark names. Default progression logs can be turned off
with the `BenchmarkRunOptions.silent` flag.

##### `clearBenchmarks(opts?: BenchmarkClearOptions): void`

Clears all registered benchmarks, so calling `runBenchmarks()` after it wont run
them. Filtering can be applied by setting `BenchmarkRunOptions.only` and/or
`BenchmarkRunOptions.skip` to regular expressions matching benchmark names.
