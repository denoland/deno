# Testing

Deno has several test suites, each aimed at a different layer. This page is a
map of what each one is for and how to run it. For the operational command
reference, `CLAUDE.md` at the repository root is the source of truth; this page
adds the "when do I reach for which suite" context.

## The suites at a glance

| Suite       | Location             | Tests…                                 |
| ----------- | -------------------- | -------------------------------------- |
| Spec tests  | `tests/specs/`       | CLI commands end-to-end (the main one) |
| Unit tests  | `tests/unit/`        | Runtime/web APIs from JS/TS            |
| unit_node   | `tests/unit_node/`   | The `node:*` compatibility layer       |
| Node compat | `tests/node_compat/` | Node's own test suite, run on Deno     |
| WPT         | `tests/wpt/`         | Web Platform Tests (web standards)     |
| Rust tests  | throughout           | Crate-level Rust logic                 |

## Spec tests — the main integration tests

Spec tests live in `tests/specs/`. Each test directory contains a
`__test__.jsonc` file describing one or more steps; a step is a `deno`
invocation whose output is captured and matched against an expectation. The test
name is the directory name. The expectation language supports wildcards
(`[WILDCARD]`, `[WILDLINE]`), unordered blocks, and inline comments; the schema
is `tests/specs/schema.json`.

Reach for a spec test whenever a change is observable from the command line: a
new flag, a subcommand behavior, an error message, a resolution outcome.

To create one:

1. Make a directory under `tests/specs/` with a descriptive name.
2. Add `__test__.jsonc` with the step(s).
3. Add any input files the test needs.
4. Put expected output inline or in a `.out` file.

## Unit tests (`tests/unit/`)

JavaScript/TypeScript tests for runtime and web APIs, named `*_test.ts`. Use
these for behavior that is best asserted from inside the runtime (a Web API's
semantics, a `Deno.*` method). They run under the cargo test harness, not under
a bare `deno test`, because they rely on harness setup.

## Node compatibility tests

Two distinct things, often confused:

- `tests/unit_node/` — Deno-authored unit tests for the `node:*` builtins.
- `tests/node_compat/` — Node.js's **own** test files, executed against Deno to
  measure compatibility. The set that runs is controlled by
  `tests/node_compat/config.jsonc`. OS-specific skips use per-OS flags (for
  example `"windows": false`), not a blanket `"ignore"`.

## Web Platform Tests (`tests/wpt/`)

The upstream Web Platform Tests, used to check standards conformance for web
APIs. Run on Linux in CI.

## Rust tests

Standard `cargo test`, plus `cargo nextest` for the `deno_core` / `libs/*`
crates in CI. Lower-layer crates under `libs/` are designed to be testable in
isolation; some need a sibling crate co-selected to compile (see `CLAUDE.md` and
the project memory for the specific gotchas).

## Running the suites

`CLAUDE.md` lists the exact commands. In short, the `./x` helper wraps the
common ones:

- `./x test-spec <name>` — spec tests.
- `./x test-unit <name>` — unit tests.
- `./x test-compat <name>` — node compat tests.
- `./x test-napi` — NAPI tests.
- `cargo test unit_node::<module>` — `unit_node` tests.

Run `./x --help` to see everything. Before pushing, run `tools/format.js` and
the appropriate `tools/lint.js` invocation (`--js` when only JS/TS changed).

## What CI runs

On a pull request, CI builds Deno across the supported platforms and runs the
spec, unit, node-compat and WPT suites, plus the `deno_core` crate tests and
lint. The exception is a PR that only edits `doc/`: it runs the `lint` job alone
and skips the rest, since Markdown changes cannot affect the binary. See
[`ci.md`](./ci.md) for how that decision is made.
