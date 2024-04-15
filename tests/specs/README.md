# specs

These are integration tests that execute the `deno` binary. They supersede the
`itest` macro found in the `tests/integration` folder and are the preferred way
of writing tests that use the `deno` binary.

## Structure

Tests must have the following directory structure:

```
tests/specs/<category_name>/<test_name>/__test__.json
```

## Test filtering

To run a specific test, run:

```
cargo test specs::category_name::test_name
```

Or just the following, though it might run other tests:

```
cargo test test_name
```

## `__test__.json` file

This file describes the test to execute and the steps to execute. A basic
example looks like:

```json
{
  "args": "run main.js",
  "output": "main.out"
}
```

This will run `deno run main.js` then assert that the output matches the text in
`main.out`.

Or another example that runs multiple steps:

```json
{
  "tempDir": true,
  "steps": [{
    "args": "cache main.ts",
    "output": "cache.out"
  }, {
    "args": "run main.ts",
    "output": "error.out",
    "exitCode": 1
  }]
}
```

### Top level properties

- `base` - The base config to use for the test. Options:
  - `jsr` - Uses env vars for jsr.
  - `npm` - Uses env vars for npm.
- `tempDir` (boolean) - Copy all the non-test files to a temporary directory and
  execute the command in that temporary directory.
  - By default, tests are executed with a current working directory of the test,
    but this may not be desirable for tests such as ones that create a
    node_modules directory.

### Step properties

When writing a single step, these may be at the top level rather than nested in
a "steps" array.

- `args` - A string (that will be spilt on whitespace into an args array) or an
  array of arguments.
- `output` - Path to use to assert the output.
- `clean` (boolean) - Whether to empty the deno_dir before running the step.
- `exitCode` (number) - Expected exit code.

## `.out` files

`.out` files are used to assert the output when running a test or test step.

Within the file, you can use the following for matching:

- `[WILDCARD]` - match any text at the wildcard
- `[WILDLINE]` - match any text on the current line
- `[WILDCHAR]` - match the next character
- `[WILDCHARS(5)]` - match any of the next 5 characters
- `[UNORDERED_START]` followed by many lines then `[UNORDERED_END]` will match
  the lines in any order (useful for non-deterministic output)
- `[# example]` - line comments start with `[#` and end with `]`
