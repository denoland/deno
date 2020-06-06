# Testing

Deno has a built-in test runner that you can use for testing JavaScript or
TypeScript code.

## Writing tests

To define a test you need to call `Deno.test` with a name and function to be
tested:

```ts
Deno.test("hello world", () => {
  const x = 1 + 2;
  if (x !== 3) {
    throw Error("x should be equal to 3");
  }
});
```

There are some useful assertion utilities at https://deno.land/std/testing to
make testing easier:

```ts
import { assertEquals } from "https://deno.land/std/testing/asserts.ts";

Deno.test("hello world", () => {
  const x = 1 + 2;
  assertEquals(x, 3);
});
```

### Async functions

You can also test asynchronous code by passing a test function that returns a
promise. For this you can use the `async` keyword when defining a function:

```ts
import { delay } from "https://deno.land/std/async/delay.ts";

Deno.test("async hello world", async () => {
  const x = 1 + 2;

  // await some async task
  await delay(100);

  if (x !== 3) {
    throw Error("x should be equal to 3");
  }
});
```

### Resource and async op sanitizers

Certain actions in Deno create resources in the resource table
([learn more here](./contributing/architecture.md)). These resources should be
closed after you are done using them.

For each test definition the test runner checks that all resources created in
this test have been closed. This is to prevent resource 'leaks'. This is enabled
by default for all tests, but can be disabled by setting the `sanitizeResources`
boolean to false in the test definition.

The same is true for async operation like interacting with the filesystem. The
test runner checks that each operation you start in the test is completed before
the end of the test. This is enabled by default for all tests, but can be
disabled by setting the `sanitizeOps` boolean to false in the test definition.

```ts
Deno.test({
  name: "leaky test",
  fn() {
    Deno.open("hello.txt");
  },
  sanitizeResources: false,
  sanitizeOps: false,
});
```

### Ignoring tests

Sometimes you want to ignore tests based on some sort of condition (for example
you only want a test to run on Windows). For this you can use the `ignore`
boolean in the test definition. If it is set to true the test will be skipped.

```ts
Deno.test({
  name: "do macOS feature",
  ignore: Deno.build.os !== "darwin",
  fn() {
    doMacOSFeature();
  },
});
```

## Running tests

To run the test, call `deno test` with the file that contains your test
function:

```shell
deno test my_test.ts
```

You can also omit the file name, in which case all tests in the current
directory (recursively) that match the glob `{*_,*.,}test.{js,mjs,ts,jsx,tsx}`
will be run. If you pass a directory, all files in the directory that match this
glob will be run.
