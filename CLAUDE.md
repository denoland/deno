# Deno Development Guide

## Table of Contents

- [High Level Overview](#high-level-overview)
- [Quick Start](#quick-start)
- [Commands](#commands)
- [Testing](#testing)
- [Development Workflows](#development-workflows)
- [Debugging](#debugging)
- [Codebase Navigation](#codebase-navigation)
- [Troubleshooting](#troubleshooting)

## High Level Overview

The user visible interface and high level integration is in the `deno` crate
(located in `./cli`).

This includes flag parsing, subcommands, package management tooling, etc. Flag
parsing is in `cli/args/flags.rs`. Tools are in `cli/tools/<tool>`.

The `deno_runtime` crate (`./runtime`) assembles the javascript runtime,
including all of the "extensions" (native functionality exposed to javascript).
The extensions themselves are in the `ext/` directory, and provide system access
to javascript – for instance filesystem operations and networking.

### Key Directories

- `cli/` - User-facing CLI implementation, subcommands, and tools
- `runtime/` - JavaScript runtime assembly and integration
- `ext/` - Extensions providing native functionality to JS (fs, net, etc.)
- `tests/specs/` - Integration tests (spec tests)
- `tests/unit/` - Unit tests
- `tests/testdata/` - Test fixtures and data files

## Quick Start

### Building Deno

To compile after making changes:

```bash
cargo build
```

For faster iteration during development (less optimization):

```bash
cargo build --bin deno
```

Execute your development build:

```bash
./target/debug/deno eval 'console.log("Hello from dev build")'
```

### Running with your changes

```bash
# Run a local file
./target/debug/deno run path/to/file.ts

# Run with permissions
./target/debug/deno run --allow-net --allow-read script.ts

# Run the REPL
./target/debug/deno
```

## Commands

### Compilation and Checks

```bash
# Check for compilation errors (fast, no binary output)
cargo check

# Check specific package
cargo check -p deno_runtime

# Build release version (slow, optimized)
cargo build --release
```

### Code Quality

```bash
# Lint the code
./tools/lint.js

# Format the code
./tools/format.js

# Both lint and format
./tools/format.js && ./tools/lint.js
```

## Testing

### Running Tests

```bash
# Run all tests (this takes a while)
cargo test

# Filter tests by name
cargo test <nameOfTest>

# Run tests in a specific package
cargo test -p deno_core

# Run just the CLI integration tests
cargo test --bin deno

# Run spec tests only
cargo test specs

# Run a specific spec test
cargo test spec::test_name
```

### Test Organization

- **Spec tests** (`tests/specs/`) - Main integration tests, CLI command
  execution and output validation
- **Unit tests** - Inline with source code in each module
- **Integration tests** (`cli/tests/`) - Additional integration tests
- **WPT** (`tests/wpt/`) - Web Platform Tests for web standards compliance

## "spec" tests

The main form of integration test in deno is the "spec" test. These tests can be
found in `tests/specs`. The idea is that you have a `__test__.jsonc` file that
lays out one or more tests, where a test is a CLI command to execute and the
output is captured and asserted against.

The name of the test comes from the directory the `__test__.jsonc` appears in.

### Creating a New Spec Test

1. Create a directory in `tests/specs/` with a descriptive name
2. Add a `__test__.jsonc` file describing your test steps
3. Add any input files needed for the test
4. Add `.out` files for expected output (or inline in `__test__.jsonc`)

Example:

```
tests/specs/my_feature/
  __test__.jsonc
  main.ts
  expected.out
```

### `__test__.jsonc` schema

The schema for `__test__.jsonc` can be found in `tests/specs/schema.json`.

Example test structure:

```jsonc
{
  "tests": {
    "basic_case": {
      "args": "run main.ts",
      "output": "expected.out"
    },
    "with_flag": {
      "steps": [
        {
          "args": "run --allow-net main.ts",
          "output": "[WILDCARD]success[WILDCARD]"
        }
      ]
    }
  }
}
```

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

Example `.out` file:

```
Check file://[WILDCARD]/main.ts
[WILDCARD]
Successfully compiled [WILDLINE]
```

## Development Workflows

### Adding a New CLI Subcommand

1. Define the command structure in `cli/args/flags.rs`
2. Add the command handler in `cli/tools/<command_name>.rs` or
   `cli/tools/<command_name>/mod.rs`
3. Wire it up in `cli/main.rs`
4. Add spec tests in `tests/specs/<command_name>/`

Example files to reference:

- Simple command: `cli/tools/fmt.rs`
- Complex command: `cli/tools/test/`

### Modifying or Adding an Extension

1. Navigate to `ext/<extension_name>/` (e.g., `ext/fs/`, `ext/net/`)
2. Rust code provides the ops (operations) exposed to JavaScript
3. JavaScript code in the extension provides the higher-level APIs
4. Update `runtime/worker.rs` to register the extension if new
5. Add tests in the extension's directory

### Updating Dependencies

```bash
# Update Cargo dependencies
cargo update

# Update to latest compatible versions
cargo upgrade  # Requires cargo-edit: cargo install cargo-edit

# Check for outdated dependencies
cargo outdated  # Requires cargo-outdated
```

## Debugging

### Debugging Rust Code

Use your IDE's debugger (VS Code with rust-analyzer, IntelliJ IDEA, etc.):

1. Set breakpoints in Rust code
2. Run tests in debug mode through your IDE
3. Or use `lldb` directly:

```bash
lldb ./target/debug/deno
(lldb) run eval 'console.log("test")'
```

### Debugging JavaScript Runtime Issues

```bash
# Enable V8 inspector
./target/debug/deno run --inspect-brk script.ts

# Then connect Chrome DevTools to chrome://inspect
```

Or use println debugging.

### Verbose Logging

```bash
# Set Rust log level
DENO_LOG=debug ./target/debug/deno run script.ts

# Specific module logging
DENO_LOG=deno_core=debug ./target/debug/deno run script.ts
```

### Debug Prints

In Rust code:

```rust
eprintln!("Debug: {:?}", some_variable);
dbg!(some_variable);
```

In the JavaScript runtime:

```javascript
console.log("Debug:", value);
```

## Codebase Navigation

### Key Files to Understand First

1. `cli/main.rs` - Entry point, command routing
2. `cli/args/flags.rs` - CLI flag parsing and structure
3. `runtime/worker.rs` - Worker/runtime initialization
4. `runtime/permissions.rs` - Permission system
5. `cli/module_loader.rs` - Module loading and resolution

### Common Patterns

- **Ops** - Rust functions exposed to JavaScript (in `ext/` directories)
- **Extensions** - Collections of ops and JS code providing functionality
- **Workers** - JavaScript execution contexts (main worker, web workers)
- **Resources** - Managed objects passed between Rust and JS (files, sockets,
  etc.)

### Finding Examples

- Need to add a CLI flag? Look at similar commands in `cli/args/flags.rs`
- Need to add an op? Look at ops in relevant `ext/` directory (e.g.,
  `ext/fs/lib.rs`)
- Need to add a tool? Reference existing tools in `cli/tools/`

## Troubleshooting

### Build Failures

**Error: linking with `cc` failed**

- Make sure you have the required system dependencies
- On macOS: `xcode-select --install`
- On Linux: Install `build-essential` or equivalent

**Error: failed to download dependencies**

- Check internet connection
- Try `cargo clean` then rebuild
- Check if behind a proxy, configure cargo accordingly

### Test Failures

**Spec test failures**

- Check the test output carefully for differences
- Update `.out` files if output format changed intentionally
- Use `[WILDCARD]` for non-deterministic parts of output

**Flaky tests**

- Add `[UNORDERED_START]`/`[UNORDERED_END]` for order-independent output
- Check for race conditions in test code
- May need to increase timeouts or add retries

### Permission Issues

**Tests failing with permission errors**

- Ensure test files have correct permissions
- Check that test setup properly grants necessary permissions

### Performance Issues

**Slow compile times**

- Use `cargo check` instead of `cargo build` when possible
- Use `--bin deno` to build only the main binary
- Use `sccache` or `mold` linker for faster builds
- Consider using `cargo-watch` for incremental builds

### Runtime Debugging

**Crashes or panics**

- Run with `RUST_BACKTRACE=1` for full backtrace
- Use `RUST_BACKTRACE=full` for even more detail
- Check for unwrap() calls that might panic

**Unexpected behavior**

- Add debug prints liberally
- Use the inspector for JS-side debugging
- Check permission grants - many features require explicit permissions

### Getting Help

- Check existing issues on GitHub
- Look at recent PRs for similar changes
- Review the Discord community for discussions
- When in doubt, ask! The maintainers are helpful
