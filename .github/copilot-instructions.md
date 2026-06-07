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
- `libs/` — Shared Rust crates (core, resolver, npm, node_resolver, serde_v8,
  etc.)
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

## Pull Request Reviews

### Before commenting, verify your claims

- If you claim something is missing (a stub, a test, error handling), search the
  full diff AND the existing codebase before commenting. Do not flag missing
  code that already exists elsewhere in the PR or the repository.
- If you suggest a code change, verify it compiles and does not break the
  intended behavior. Do not suggest fixes that contradict the PR's stated goal.
- Do not duplicate your own comments. If you already flagged an issue, do not
  post a second comment about the same thing.

### Focus on high-value issues

Prioritize these (in order):

1. **Correctness bugs** — logic errors, race conditions (e.g. spurious wakeups
   on `Condvar`), use-after-free, null derefs
2. **Public API leaks** — internal fields accidentally exposed in public return
   types
3. **Security** — unsafe blocks with incorrect safety invariants, unsanitized
   inputs at system boundaries
4. **Missing error handling** — errors silently swallowed where they should
   propagate, or propagated where they should be caught

Do NOT comment on:

- Style preferences already enforced by the project's formatter (dprint) and
  linter (clippy + dlint)
- Suggesting longer timeouts or shorter delays in tests without evidence of
  flakiness
- Minor documentation wording unless it is actively misleading
- Hypothetical edge cases that cannot realistically occur (e.g. a process having
  > 256 direct children)

### Understand the runtime model before suggesting fixes

Deno embeds V8 and uses a single-threaded async event loop (tokio). Code that
looks like a busy-spin may actually be required because:

- `poll_sessions(None)` in the inspector drives async I/O — parking the thread
  prevents WebSocket close frames from being processed, causing deadlocks
- The event loop must keep running for futures to make progress; blocking the
  main thread stops the I/O reactor
- Some loops intentionally spin to allow the waker/poller to process events each
  iteration

Before suggesting `sleep`, `park`, or backoff in a polling loop, check whether
the loop body drives async I/O that would stall if the thread were blocked.

### Trace through the actual code path, not just the function signature

A common review mistake is looking at a function's local behavior without
tracing its callers and the runtime context. For example:

- A function that "doesn't check field X" may not need to because callers
  guarantee X is consumed before the check runs (e.g., `handshake` is always
  `take()`n at the top of `poll_sessions` before any session-count checks)
- A heuristic that "doesn't handle case Y" may already handle it through a
  different code path (e.g., dotenv comment detection via `#` prefix check
  happens before the inner-quote fallback is reached)

### Don't suggest fixes that introduce circular dependencies

In parsers and state machines, be careful not to suggest fixes that require
knowing the answer to the question being solved. For example, suggesting "detect
where the comment starts before matching quotes" is circular when the quote
matching is what determines where the value (and thus the comment) begins.

### Verify suggestions against existing tests

Before suggesting a change, check whether the codebase already has test coverage
for the case in question. The `test_valid_env` test in `libs/dotenv/lib.rs`
covers many edge cases including inline comments with quotes
(`EDGE_CASE_INLINE_COMMENTS`). Running existing tests before and after a
suggested change catches regressions.

### Match existing patterns in the codebase

When suggesting changes to a function, look for similar functions nearby that
solve analogous problems. If the existing pattern (e.g., `wait_for_session`)
uses a specific flag/mechanism, understand why before suggesting a different
approach for a similar function (e.g., `wait_for_sessions_disconnect`). The
difference may be intentional.

### Deno-specific conventions

- Ops use the `#[op2]` macro. Fast ops use `#[op2(fast)]`.
- Internal symbols in JS (`ext/process/40_process.js`, `ext/node/polyfills/`)
  use the `k` prefix convention: `kIpc`, `kSerialization`, `kInputOption`. Flag
  deviations.
- Node.js compatibility code is in `ext/node/polyfills/`. When reviewing these
  files, check behavior against Node.js docs, not just Deno conventions. Some
  patterns (like synchronous `throw` in `spawn()` for EPERM but async error
  event for ENOENT) are intentional Node.js compatibility.

### When suggesting error handling changes

- Check whether the function's callers expect the error to propagate or be
  swallowed. Do not suggest propagating errors in paths that are intentionally
  best-effort (e.g. killing already-exited processes).
- When suggesting a specific error variant or code change, verify the types
  actually match. For example, `Option::or_else` takes `FnOnce() -> Option<T>`,
  not `FnOnce(E) -> ...`.
- If a Rust function returns `Result` and an `Option` is needed, use `.ok()`
  before `.or_else()` or `.unwrap_or()`.

### Platform-specific code

- `#[cfg(unix)]` code that calls `find_descendant_pids` or similar must have
  stubs for non-Linux/non-macOS Unix targets. Check that
  `#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]` stubs
  exist before flagging missing platform support.
- Windows `unsafe` blocks using `windows_sys` are common for process management.
  Verify handle cleanup (`CloseHandle`) but do not flag the `unsafe` usage
  itself.

## Troubleshooting

- **Slow compile times**: Use `cargo check`, `--bin deno`, or `sccache`
- **Build failures on macOS**: Run `xcode-select --install`
- **Build failures on Linux**: Install `build-essential`
- **Spec test failures**: Check output diffs, use `[WILDCARD]` for
  non-deterministic parts
- **Permission errors**: Ensure test files have correct permissions
