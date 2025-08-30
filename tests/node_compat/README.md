# Node compat test directory

This directory includes the tools for running Node.js test cases directly in
Deno.

- ./runner/suite/ - vendored Node.js test cases (git submodule at
  https://github.com/denoland/node_test)
- ./config.toml - has the list of passing Node.js test cases
- ./test.ts - The script entrypoint of node compat test.

If you run single node.js test case, use the command:

```
./tools/node_compat_tests.js --filter <name of test file>
```

## Configuration file

The `config.toml` specifies which tests should pass in Deno and includes
platform-specific and behavioral settings for each test.

### Options

Each test entry can include the following optional configuration properties:

- **`flaky`** (boolean): Marks a test as flaky. It will be run at most 3 times
  before being considered failed.

- **`windows`** (boolean): Controls whether the test should run on Windows.
  Defaults to `true`.

- **`darwin`** (boolean): Controls whether the test should run on macOS.
  Defaults to `true`.

- **`linux`** (boolean): Controls whether the test should run on Linux. Defaults
  to `true`.

- **`reason`** (string): Optional explanation for why a test is marked as
  skipped.

### Examples

```toml
# Should pass on all platforms
"parallel/test-foo.js" = {}

# Test marked as flaky
"parallel/test-bar.js" = { flaky = true }

# Test skipped on all platforms with explanation
"parallel/test-baz.js" = { darwin = false, linux = false, windows = false, reason = "some reason" }

# Test skipped only on Windows
"parallel/test-qux.js" = { windows = false }
```

## Add test case entry to CI check

If you fixed some Node.js compabitility and some test cases started passing,
then add those cases to `config.toml`. The items listed in there are checked in
CI check.

## Daily test viewer

To see the latest test results of all test cases, visit this site
https://node-test-viewer.deno.dev/results/latest
