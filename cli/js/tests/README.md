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

## Running tests

`unit_test_runner.ts` is the main script used to run unit tests.

Runner discoveres required permissions combinations by loading
`cli/js/tests/unit_tests.ts` and going through all registered instances of
`unitTest`.

There are three ways to run `unit_test_runner.ts`:

- run tests matching current process permissions

```
// run tests that don't require any permissions
target/debug/deno unit_test_runner.ts

// run tests with "net" permission
target/debug/deno --allow-net unit_test_runner.ts

target/debug/deno --allow-net --allow-read unit_test_runner.ts
```

- run all tests - "master" mode, that spawns worker processes for each
  discovered permission combination:

```
target/debug/deno -A unit_test_runner.ts --master
```

By default all output of worker processes is discarded; for debug purposes
`--verbose` flag can be provided to preserve output from worker

```
target/debug/deno -A unit_test_runner.ts --master --verbose
```

- "worker" mode; communicates with parent using TCP socket on provided address;
  after initial setup drops permissions to specified set. It shouldn't be used
  directly, only be "master" process.

```
target/debug/deno -A unit_test_runner.ts --worker --addr=127.0.0.1:4500 --perms=net,write,run
```

### Filtering

Runner supports basic test filtering by name:

```
target/debug/deno unit_test_runner.ts -- netAccept

target/debug/deno -A unit_test_runner.ts --master -- netAccept
```

Filter string must be specified after "--" argument
