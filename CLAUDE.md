# High level overview

The user visible interface and high level integration is in the `deno` crate
(located in `./cli`).

This includes flag parsing, subcommands, package management tooling, etc. Flag
parsing is in `cli/args/flags.rs`. Tools are in `cli/tools/<tool>`.

The `deno_runtime` crate (`./runtime`) assembles the javascript runtime,
including all of the "extensions" (native functionality exposed to javascript).
The extensions themselves are in the `ext/` directory, and provide system access
to javascript – for instance filesystem operations and networking.

## Running a development deno build

To compile after making changes, run `cargo build`. Then, you can execute the
development build from `./target/debug/deno`, for instance
`./target/debug/deno eval 'console.log("reproducing...")'`.

## Commands

To check for compilation errors, run `cargo check`.

To run all the tests, use `cargo test`. To filter tests to run, use
`cargo test <nameOfTest>`.

To lint the code, run `./tools/lint.js`. To format the code, run
`./tools/format.js`.

## "spec" tests

The main form of integration test in deno is the "spec" test. These tests can be
found in `tests/specs`. The idea is that you have a `__test__.jsonc` file that
lays out one or more tests, where a test is a CLI command to execute and the
output is captured and asserted against.

The the name of the test comes from the directory the `__test__.jsonc` appears
in.

### `__test__.jsonc` **schema**

The schema for `__test__.jsonc` can be found in `tests/specs/schema.json`.

### Output assertions

The expected output can be inline in a `__test__.jsonc` file or in a file ending
with `.out`. For a given test step, the `output` field tells you either the
inline expectation or the name of the file containing the **expectation**. The
expectation uses a small matching language to support wildcards and things like
that. A literal character means you expect that exact character, so `Foo bar`
would expect the output to be "Foo bar". Then there are things with special
meanings:

- `[WILDCARD]` : matches 0 or more of any character, like `.*` in regex. this
  can cross newlines
- `[WILDLINE]` : matches 0 or more of any character, ending at the end of a line
- `[WILDCHAR]` - match the next character
- `[WILDCHARS(5)]` - match any of the next 5 characters
- `[UNORDERED_START]` followed by many lines then `[UNORDERED_END]` will match
  the lines in any order (useful for non-deterministic output)
- `[# example]` - line comments start with `[#` and end with `]`
