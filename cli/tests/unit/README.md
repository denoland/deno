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
    ignore: Deno.build.os === "windows",
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

Runner discovers required permissions combinations by loading
`cli/tests/unit/unit_tests.ts` and going through all registered instances of
`unitTest`.

There are three ways to run `unit_test_runner.ts`:

```
# Run all tests. Spawns worker processes for each discovered permission
# combination:
target/debug/deno run -A cli/tests/unit/unit_test_runner.ts --master

# By default all output of worker processes is discarded; for debug purposes
# the --verbose flag preserves output from the worker
target/debug/deno run -A cli/tests/unit/unit_test_runner.ts --master --verbose

# Run subset of tests that don't require any permissions
target/debug/deno run --unstable cli/tests/unit/unit_test_runner.ts

# Run subset tests that require "net" and "read" permissions
target/debug/deno run --unstable --allow-net --allow-read cli/tests/unit/unit_test_runner.ts

# "worker" mode communicates with parent using TCP socket on provided address;
# after initial setup drops permissions to specified set. It shouldn't be used
# directly, only be "master" process.
target/debug/deno run -A cli/tests/unit/unit_test_runner.ts --worker --addr=127.0.0.1:4500 --perms=net,write,run

# Run specific tests
target/debug/deno run --unstable --allow-net cli/tests/unit/unit_test_runner.ts -- netTcpListenClose

RUST_BACKTRACE=1 cargo run -- run --unstable --allow-read --allow-write cli/tests/unit/unit_test_runner.ts -- netUnixDialListen
```

### Http server

`tools/http_server.py` is required to run when one's running unit tests. During
CI it's spawned automatically, but if you want to run tests manually make sure
that server is spawned otherwise there'll be cascade of test failures.
