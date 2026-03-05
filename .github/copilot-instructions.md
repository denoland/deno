# Deno Development Guide for GitHub Copilot

## Network Access

The development tools in this repository need network access to function. When
running `tools/format.js` or `tools/lint.js`, ensure the following domains are
reachable:

- `https://jsr.io` — Deno's package registry, used for `@std/*` imports
- `https://dprint.dev` and `https://plugins.dprint.dev` — the formatter
  (`tools/format.js`) runs `npm:dprint` which downloads WASM plugins from
  `plugins.dprint.dev`
- `https://raw.githubusercontent.com` — `tools/lint.js` downloads prebuilt
  `dlint` binaries from GitHub

If you are running in a sandboxed environment, you must allowlist these domains
or the tools will fail with network errors.

## High Level Overview

The user-visible interface and high-level integration is in the `deno` crate
(located in `./cli`).

This includes flag parsing, subcommands, package management tooling, etc. Flag
parsing is in `cli/args/flags.rs`. Tools are in `cli/tools/<tool>`.

The `deno_runtime` crate (`./runtime`) assembles the JavaScript runtime,
including all of the "extensions" (native functionality exposed to JavaScript).
The extensions themselves are in the `ext/` directory, and provide system access
to JavaScript — for instance filesystem operations and networking.

### Key Directories

- `cli/` — User-facing CLI implementation, subcommands, and tools
- `runtime/` — JavaScript runtime assembly and integration
- `ext/` — Extensions providing native functionality to JS (fs, net, etc.)
- `tests/specs/` — Integration tests (spec tests)
- `tests/unit/` — Unit tests
- `tests/testdata/` — Test fixtures and data files

### Key Files to Understand First

1. `cli/main.rs` — Entry point, command routing
2. `cli/args/flags.rs` — CLI flag parsing and structure
3. `runtime/worker.rs` — Worker/runtime initialization
4. `runtime/permissions.rs` — Permission system
5. `cli/module_loader.rs` — Module loading and resolution

### Common Patterns

- **Ops** — Rust functions exposed to JavaScript (in `ext/` directories)
- **Extensions** — Collections of ops and JS code providing functionality
- **Workers** — JavaScript execution contexts (main worker, web workers)
- **Resources** — Managed objects passed between Rust and JS (files, sockets,
  etc.)

## Building

```bash
# Check for compilation errors (fast, no binary output)
cargo check

# Build debug binary
cargo build --bin deno

# Build release version (slow, optimized)
cargo build --release

# Run the dev build
./target/debug/deno eval 'console.log("Hello from dev build")'
```

## Code Quality

Before committing, always run the formatter and linter:

```bash
# Format all code (uses dprint under the hood)
./tools/format.js

# Lint all code (JS + Rust via clippy)
./tools/lint.js

# Lint only JS/TS (faster, skips clippy)
./tools/lint.js --js

# Lint only Rust
./tools/lint.js --rs
```

The formatter (`tools/format.js`) runs `dprint` via `npm:dprint@0.47.2`. It
formats TypeScript, JavaScript, JSON, Markdown, YAML, and Rust (via `rustfmt`).
Configuration is in `.dprint.json`.

The linter (`tools/lint.js`) runs `dlint` for JS/TS and `cargo clippy` for Rust.
`dlint` is a prebuilt binary downloaded from GitHub on first use. Configuration
is in `.dlint.json`.

## Testing

```bash
# Run all tests
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

- **Spec tests** (`tests/specs/`) — Main integration tests
- **Unit tests** — Inline with source code in each module
- **Integration tests** (`cli/tests/`) — Additional integration tests
- **WPT** (`tests/wpt/`) — Web Platform Tests for web standards compliance

### Spec Tests

The main form of integration test is the "spec" test in `tests/specs/`. Each
test has a `__test__.jsonc` file describing CLI commands to run and expected
output. The schema is in `tests/specs/schema.json`.

Example `__test__.jsonc`:

```jsonc
{
  "tests": {
    "basic_case": {
      "args": "run main.ts",
      "output": "expected.out"
    }
  }
}
```

Output assertions support wildcards:

- `[WILDCARD]` — matches 0 or more characters (crosses newlines)
- `[WILDLINE]` — matches to end of line
- `[WILDCHAR]` — matches one character
- `[WILDCHARS(N)]` — matches N characters
- `[UNORDERED_START]` / `[UNORDERED_END]` — matches lines in any order

## Git Workflow and Pull Requests

### PR Title Linting

PR titles are validated by CI (see `.github/workflows/pr.yml`). The title must
follow [Conventional Commits](https://www.conventionalcommits.org) and start
with one of these prefixes:

- `feat:` — new features
- `fix:` — bug fixes
- `chore:` — maintenance tasks
- `perf:` — performance improvements
- `ci:` — CI changes
- `cleanup:` — code cleanup
- `docs:` — documentation
- `bench:` — benchmarks
- `build:` — build system changes
- `refactor:` — refactoring
- `test:` — test changes
- `Revert` — reverting a previous commit
- `Reland` — relanding a reverted commit
- `BREAKING` — breaking changes

Additionally, deno_core/v8 upgrades must NOT use `chore:` — use `feat:`, `fix:`,
or `refactor:` instead, with a title describing the actual change.

Release PRs (titles matching `X.Y.Z`) are also valid.

The validation script is at `tools/verify_pr_title.js`.

### Workflow Rules

- Create feature branches with descriptive names
- Commit with clear, descriptive messages
- Never force push — all commits are squashed on merge
- Keep changes minimal and focused; avoid drive-by changes
- Before committing, run `tools/format.js` and `tools/lint.js`

## Development Workflows

### Adding a New CLI Subcommand

1. Define the command structure in `cli/args/flags.rs`
2. Add the command handler in `cli/tools/<command_name>.rs` or
   `cli/tools/<command_name>/mod.rs`
3. Wire it up in `cli/main.rs`
4. Add spec tests in `tests/specs/<command_name>/`

### Modifying or Adding an Extension

1. Navigate to `ext/<extension_name>/`
2. Rust code provides the ops exposed to JavaScript
3. JavaScript code in the extension provides higher-level APIs
4. Update `runtime/worker.rs` to register a new extension
5. Add tests in the extension's directory

## Debugging

```bash
# Verbose logging
DENO_LOG=debug ./target/debug/deno run script.ts

# Module-specific logging
DENO_LOG=deno_core=debug ./target/debug/deno run script.ts

# Full backtrace on panic
RUST_BACKTRACE=1 ./target/debug/deno run script.ts

# V8 inspector
./target/debug/deno run --inspect-brk script.ts
```

In Rust code: `eprintln!("Debug: {:?}", var);` or `dbg!(var);`

## Troubleshooting

- **Slow compile times**: Use `cargo check`, `--bin deno`, or `sccache`
- **Build failures on macOS**: Run `xcode-select --install`
- **Build failures on Linux**: Install `build-essential`
- **Spec test failures**: Check output diffs, use `[WILDCARD]` for
  non-deterministic parts
- **Permission errors**: Ensure test files have correct permissions
