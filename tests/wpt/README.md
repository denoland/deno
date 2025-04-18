# Web Platform Test (WPT)

Deno uses a custom test runner for Web Platform Tests. It can be found at
`./tests/wpt/wpt.ts`, relative to the root of this codebase.

## Running tests

> If you are on Windows, or your system does not support hashbangs, prefix all
> `./tests/wpt/wpt.ts` commands with
> `deno run --unstable --allow-write --allow-read --allow-net --allow-env --allow-run`.

Before attempting to run WPT tests for the first time, please run the WPT setup.
You must also run this command every time the `./test_util/wpt` submodule is
updated:

```shell
./tests/wpt/wpt.ts setup
```

To run all available web platform tests, run the following command:

```shell
./tests/wpt/wpt.ts run

# You can also filter which test files to run by specifying filters:
./tests/wpt/wpt.ts run -- streams/piping/general hr-time
```

The test runner will run each web platform test and record its status (failed or
ok). It will then compare this output to the expected output of each test as
specified in the `./tests/wpt/runner/expectation.json` file. This file is a
nested JSON structure that mirrors the `./tests/wpt/suite` directory. It
describes for each test file, if it should pass as a whole (all tests pass,
`true`), if it should fail as a whole (test runner encounters an exception
outside of a test or all tests fail, `false`), or which tests it expects to fail
(a string array of test case names).

## Updating enabled tests or expectations

You can update the `./tests/wpt/runner/expectation.json` file manually by
changing the value of each of the test file entries in the JSON structure. The
alternative and preferred option is to have the WPT runner run all, or a
filtered subset of tests, and then automatically update the `expectation.json`
file to match the current reality. You can do this with the `./wpt.ts update`
command. Example:

```shell
./tests/wpt/wpt.ts update -- hr-time
```

After running this command the `expectation.json` file will match the current
output of all the tests that were run. This means that running `wpt.ts run`
right after a `wpt.ts update` should always pass.

## Subcommands

### `setup`

Validate that your environment is configured correctly, or help you configure
it.

This will check that the python3 (or `python.exe` on Windows) is actually
Python 3.

You can specify the following flags to customize behaviour:

```console
--rebuild
    Rebuild the manifest instead of downloading. This can take up to 3 minutes.

--auto-config
    Automatically configure /etc/hosts if it is not configured (no prompt will be shown).
```

### `run`

Run all tests like specified in `expectation.json`.

You can specify the following flags to customize behaviour:

```console
--release
    Use the ./target/release/deno binary instead of ./target/debug/deno

--quiet
    Disable printing of `ok` test cases.

--json=<file>
    Output the test results as JSON to the file specified.
```

You can also specify exactly which tests to run by specifying one of more
filters after a `--`:

```console
./tests/wpt/wpt.ts run -- hr-time streams/piping/general
```

### `update`

Update the `expectation.json` to match the current reality.

You can specify the following flags to customize behaviour:

```console
--release
    Use the ./target/release/deno binary instead of ./target/debug/deno

--quiet
    Disable printing of `ok` test cases.

--json=<file>
    Output the test results as JSON to the file specified.
```

You can also specify exactly which tests to run by specifying one of more
filters after a `--`:

```console
./tests/wpt/wpt.ts update -- hr-time streams/piping/general
```

## FAQ

### Upgrading the wpt submodule:

```shell
cd tests/wpt/suite
git fetch origin
git checkout origin/epochs/daily
cd ../../../
git add ./tests/wpt/suite
```

All contributors will need to rerun `./tests/wpt/wpt.ts setup` after this.

Since upgrading WPT usually requires updating the expectations to cover all
sorts of upstream changes, it's best to do that as a separate PR, rather than as
part of a PR that implements a fix or feature.
