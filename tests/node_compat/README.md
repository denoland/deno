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

## Add test case entry to CI check

If you fixed some Node.js compabitility and some test cases started passing,
then add those cases to `config.toml`. The items listed in there are checked in
CI check.

## Daily test viewer

To see the latest test results of all test cases, visit this site
https://node-test-viewer.deno.dev/results/latest
