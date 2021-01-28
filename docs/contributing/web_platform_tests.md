## Web Platform Test

Deno uses a custom test runner for Web Platform Tests. It can be found at
`./tools/wpt.ts`.

### Running tests

> If you are on Windows, or your system does not support shebangs, prefix all
> `./tools/wpt.ts` commands with
> `deno run --unstable --allow-write --allow-read --allow-net --allow-env --allow-run`.

Before attempting to run WPT tests for the first time, please run the WPT setup.
You must also run this command every time the `./test_util/wpt` submodule is
updated:

```shell
./tools/wpt.ts setup
```

To run all available web platform tests, run the following command:

```shell
./tools/wpt.ts run

# You can also filter which test files to run by specifying filters:
./tools/wpt.ts run -- streams/piping/general hr-time
```

The test runner will run each web platform test and record its status (failed or
ok). It will then compare this output to the expected output of each test as
specified in the `./tools/wpt/expectation.json` file. This file is a nested JSON
structure that mirrors the `./test_utils/wpt` directory. It describes for each
test file, if it should pass as a whole (all tests pass, `true`), if it should
fail as a whole (test runner encounters an exception outside of a test or all
tests fail, `false`), or which tests it expects to fail (a string array of test
case names).

### Updating enabled tests or expectations

You can update the `./tools/wpt/expectation.json` file manually by changing the
value of each of the test file entries in the JSON structure. The alternative
and preferred option is to have the WPT runner run all, or a filtered subset of
tests, and then automatically update the `expectation.json` file to match the
current reality. You can do this with the `./wpt.ts update` command. Example:

```shell
./tools/wpt.ts update -- hr-time
```

After running this command the `expectation.json` file will match the current
output of all the tests that were run. This means that running `wpt.ts run`
right after a `wpt.ts update` should always pass.

### Subcommands

#### `setup`

Validate that your environment is configured correctly, or help you configure it.

This will check that the python3 (or `python.exe` on Windows) is actually
Python 3.

#### `run`

Run all tests like specified in `expectation.json`.

You can specify the following flags to customize behaviour:

```
--release
    Use the ./target/release/deno binary instead of ./target/debug/deno

--quiet
    Disable printing of `ok` test cases.

--json=<file>
    Output the test results as JSON to the file specified.
```

You can also specify exactly which tests to run by specifying one of more
filters after a `--`:

```
./tools/wpt.ts run -- hr-time streams/piping/general
```

### `update`

Update the `expectation.json` to match the current reality.

You can specify the following flags to customize behaviour:

```
--release
    Use the ./target/release/deno binary instead of ./target/debug/deno

--quiet
    Disable printing of `ok` test cases.

--json=<file>
    Output the test results as JSON to the file specified.
```

You can also specify exactly which tests to run by specifying one of more
filters after a `--`:

```
./tools/wpt.ts update -- hr-time streams/piping/general
```

### FAQ

#### Upgrading the wpt submodule:

```shell
cd test_util/wpt/
# Rebase to retain our modifications
git rebase origin/master
git push denoland
```

All contributors will need to rerun `./tools/wpt.ts setup` after this.
