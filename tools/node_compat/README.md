# Tools for Node.js compatibility work

We run
[native Node.js test cases](https://github.com/nodejs/node/tree/main/test)
against our Node.js compatibility feature.

This directory includes the tools for downloading, setting up, and updating the
Node.js compat testing in Deno repository.

- `//tools/node_compat/setup.ts`
  - This script sets up the Node.js compat tests.
- `//tools/node_comapt/versions/`
  - Node.js source tarballs and extracted test cases are stored here.
- `//cli/tests/node_compat/config.json`
  - This json file stores the settings about which Node.js compat test to run
    with Deno.
- `//cli/tests/node_compat/test`
  - The actual test cases are stored here.

## Steps to add new test cases from Node.js test cases

1. Update `tests` property of `//cli/tests/node_compat/config.json`. For
   example, if you want to add `test/paralles/test-foo.js` from Node.js test
   cases, then add `test-foo.js` entry in `tests.parallel` array property in
   `config.json`
1. Run `./tools/node_compat/setup.ts`

The above command copies the updated items from Node.js tarball to the Deno
source tree.

Ideally Deno should pass the Node.js compat tests without modification, but if
you need to modify it, then add that item in `ignore` perperty of `config.json`.
Then `setup.ts` doesn't overwrite the modified Node.js test cases anymore.

If the test needs to be ignored in particular platform, then add them in
`${platform}Ignore` property of `config.json`

## Run Node.js test cases

Node.js compat tests are run as part of `cargo test` command. If you want to run
only the Node.js compat test cases you can use the command
`cargo test node_compat`.
