# Web Platform Tests (WPT)

Deno uses a custom test runner for Web Platform Tests. It can be found at
`./tests/wpt/wpt.ts`, relative to the root of this codebase.

## Setup

Before attempting to run WPT tests for the first time, run the setup command.
You must also run this command every time the `./tests/wpt/suite` submodule is
updated:

```shell
./tests/wpt/wpt.ts setup
```

This will:

- Check that Python 3.11 is available (required by the WPT test server)
- Update the WPT manifest (`./tests/wpt/runner/manifest.json`)
- Configure `/etc/hosts` with entries required by the WPT test server

You can specify the following flags:

- `--rebuild` — Rebuild the manifest from scratch instead of incrementally
  updating. This can take up to 3 minutes.
- `--auto-config` — Automatically configure `/etc/hosts` without prompting.

## Running tests

To run all web platform tests, use the `--all` flag:

```shell
./tests/wpt/wpt.ts run --all
```

To run a specific subset, specify filters after `--`:

```shell
# Run all tests in a suite
./tests/wpt/wpt.ts run -- fetch

# Run tests in a subdirectory
./tests/wpt/wpt.ts run -- streams/piping/general

# Run a single test file
./tests/wpt/wpt.ts run -- /WebCryptoAPI/getRandomValues.any.html

# Run multiple filters
./tests/wpt/wpt.ts run -- hr-time fetch/api/basic
```

Running `wpt.ts run` with neither `--all` nor filters will print usage help.

Filters can start with `/` (absolute path match) or without (prefix match
without the leading `/`).

Tests are run in parallel across CPU cores, partitioned by top-level directory.

### Flags

- `--all` — Run all tests (required if no filters are specified)
- `--release` — Use `./target/release/deno` instead of `./target/debug/deno`
- `--binary=<path>` — Use a specific Deno binary (skips `cargo build`)
- `--quiet` — Only print failing test cases
- `--json=<file>` — Write test results as JSON
- `--wptreport=<file>` — Write results in the
  [wptreport](https://github.com/nicedoc/wpt-report) format
- `--inspect-brk` — Attach the V8 inspector to each test
- `--no-ignore` — Run tests marked with `{"ignore": true}` in expectations
- `--exit-zero` — Exit with code 0 even if there are failures

## Updating expectations

The `update` command runs tests and overwrites the expectation files to match
current results:

```shell
# Update all expectations
./tests/wpt/wpt.ts update --all

# Update expectations for specific suites
./tests/wpt/wpt.ts update -- hr-time fetch
```

Running `wpt.ts run` immediately after `wpt.ts update` should always pass.

The `update` command accepts the same flags as `run` (`--release`, `--binary`,
`--quiet`, `--json`, `--no-ignore`, `--inspect-brk`).

## Expectation file format

The expectations directory (`./tests/wpt/runner/expectations/`) contains one
JSON file per test suite (e.g., `fetch.json`, `dom.json`, `WebCryptoAPI.json`).
Each file is a nested JSON object that mirrors the WPT directory structure,
following the directory tree down to individual test files.

Leaf values describe what is expected for each test file:

| Value                                      | Meaning                                                               |
| ------------------------------------------ | --------------------------------------------------------------------- |
| `true`                                     | All subtests are expected to pass                                     |
| `false`                                    | The entire test file is expected to fail (crash, harness error, etc.) |
| `{"expectedFailures": ["name1", "name2"]}` | These specific subtests are expected to fail; all others should pass  |
| `{"ignore": true}`                         | Skip this test entirely (override with `--no-ignore`)                 |

Example:

```jsonc
{
  "fetch": {
    "api": {
      "basic": {
        "accept-header.any.html": true, // all subtests pass
        "stream-response.any.html": false, // entire file fails
        "request-headers.any.html": { // these 2 subtests fail
          "expectedFailures": [
            "Fetch with PUT with body",
            "Fetch with POST with body"
          ]
        },
        "mode-no-cors.sub.any.html": { // skipped
          "ignore": true
        }
      }
    }
  }
}
```

When the `run` command finishes, it shows a git diff between the current
expectation files and what the actual results would produce. This makes it easy
to see regressions and improvements.

## FAQ

### Upgrading the WPT submodule

```shell
cd tests/wpt/suite
git fetch origin
git checkout origin/epochs/daily
cd ../../../
git add ./tests/wpt/suite
```

All contributors will need to rerun `./tests/wpt/wpt.ts setup` after this.

Since upgrading WPT usually requires updating the expectations to cover upstream
changes, it's best to do that as a separate PR rather than as part of a PR that
implements a fix or feature.
