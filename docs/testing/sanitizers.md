# Test Sanitizers

The test runner offers several sanitizers to ensure that the test behaves in a
reasonable and expected way.

### Resource sanitizer

Certain actions in Deno create resources in the resource table
([learn more here](./contributing/architecture.md)).

These resources should be closed after you are done using them.

For each test definition, the test runner checks that all resources created in
this test have been closed. This is to prevent resource 'leaks'. This is enabled
by default for all tests, but can be disabled by setting the `sanitizeResources`
boolean to false in the test definition.

```ts
Deno.test({
  name: "leaky resource test",
  async fn() {
    await Deno.open("hello.txt");
  },
  sanitizeResources: false,
});
```

### Op sanitizer

The same is true for async operation like interacting with the filesystem. The
test runner checks that each operation you start in the test is completed before
the end of the test. This is enabled by default for all tests, but can be
disabled by setting the `sanitizeOps` boolean to false in the test definition.

```ts
Deno.test({
  name: "leaky operation test",
  fn() {
    setTimeout(function () {}, 1000);
  },
  sanitizeOps: false,
});
```

### Exit sanitizer

There's also the exit sanitizer which ensures that tested code doesn't call
`Deno.exit()` signaling a false test success.

This is enabled by default for all tests, but can be disabled by setting the
`sanitizeExit` boolean to false in the test definition.

```ts
Deno.test({
  name: "false success",
  fn() {
    Deno.exit(0);
  },
  sanitizeExit: false,
});

// This test never runs, because the process exits during "false success" test
Deno.test({
  name: "failing test",
  fn() {
    throw new Error("this test fails");
  },
});
```
