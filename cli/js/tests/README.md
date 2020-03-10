# Deno runtime tests

Files in this directory are unit tests for Deno runtime.

They are run under compiled Deno binary as opposed to files in `cli/js/` which
are bundled and snapshotted using `deno_typescript` crate.

Testing Deno runtime code requires checking API under different runtime
permissions (ie. running with different `--allow-*` flags). To accomplish this
all tests exercised are created using `unitTest()` function.

```
import { unitTest } from "./test_util.ts";

unitTest(function simpleTestFn(): void {
  // test code here
});

unitTest({
    skip: Deno.build.os === "win",
    perms: { read: true, write: true },
  },
  function complexTestFn(): void {
    // test code here
  }
);
```

`unitTest` is is a wrapper function that enhances `Deno.test()` API in several
ways:

- ability to conditionally skip tests using `UnitTestOptions.skip`
- ability to register required set of permissions for given test case using
  `UnitTestOptions.perms`
- sanitization of resources - ensuring that tests close all opened resources
  preventing interference between tests
- sanitization of async ops - ensuring that tests don't leak async ops by
  ensuring that all started async ops are done before test finishes

`unit_test_runner.ts` is main script used to run unit tests.

Runner discoveres required permissions combinations by loading
`cli/js/tests/unit_tests.ts` and going through all registered instances of
`unitTest`. For each discovered permission combination a new Deno process is
created with respective `--allow-*` flags which loads
`cli/js/tests/unit_tests.ts` and executes all `unitTest` that match runtime
permissions.
