# Web Platform Test documentation

Deno uses a custom test runner for Web Platform Tests.

To run all web platform tests, run the following command:

```shell
deno run --unstable --allow-write --allow-read --allow-net --allow-run ./tools/wpt.ts run
```

You can specify the following flags to customize behaviour:

```
--release
    Use the ./target/release/deno binary instead of ./target/debug/deno

--quiet
    Disable printing of `ok` test cases.

--json=<file>
    Output the test results to the JSON file specified.
```

You can also filter which test files to run by specifying filters:

```shell
./tools/wpt.ts run -- streams/piping/general hr-time
```

## FAQ

### How to update WPT repo

```shell
cd test_util/wpt/
# Update the repo
git checkout origin/master
```
