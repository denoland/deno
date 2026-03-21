---
description: Instructions for Copilot code review on pull requests
applyTo: '**'
---

# Code Review Guidelines

## Before commenting, verify your claims

- If you claim something is missing (a stub, a test, error handling), search the
  full diff AND the existing codebase before commenting. Do not flag missing
  code that already exists elsewhere in the PR or the repository.
- If you suggest a code change, verify it compiles and does not break the
  intended behavior. Do not suggest fixes that contradict the PR's stated goal.
- Do not duplicate your own comments. If you already flagged an issue, do not
  post a second comment about the same thing.

## Focus on high-value issues

Prioritize these (in order):

1. **Correctness bugs** -- logic errors, race conditions (e.g. spurious wakeups
   on `Condvar`), use-after-free, null derefs
2. **Public API leaks** -- internal fields accidentally exposed in public return
   types
3. **Security** -- unsafe blocks with incorrect safety invariants, unsanitized
   inputs at system boundaries
4. **Missing error handling** -- errors silently swallowed where they should
   propagate, or propagated where they should be caught

Do NOT comment on:

- Style preferences already enforced by the project's formatter (dprint) and
  linter (clippy + dlint)
- Suggesting longer timeouts or shorter delays in tests without evidence of
  flakiness
- Minor documentation wording unless it is actively misleading
- Hypothetical edge cases that cannot realistically occur (e.g. a process having
  > 256 direct children)

## Deno-specific conventions

- Ops use the `#[op2]` macro. Fast ops use `#[op2(fast)]`.
- Internal symbols in JS (`ext/process/40_process.js`, `ext/node/polyfills/`)
  use the `k` prefix convention: `kIpc`, `kSerialization`, `kInputOption`. Flag
  deviations.
- Node.js compatibility code is in `ext/node/polyfills/`. When reviewing these
  files, check behavior against Node.js docs, not just Deno conventions. Some
  patterns (like synchronous `throw` in `spawn()` for EPERM but async error
  event for ENOENT) are intentional Node.js compatibility.

## When suggesting error handling changes

- Check whether the function's callers expect the error to propagate or be
  swallowed. Do not suggest propagating errors in paths that are intentionally
  best-effort (e.g. killing already-exited processes).
- When suggesting a specific error variant or code change, verify the types
  actually match. For example, `Option::or_else` takes `FnOnce() -> Option<T>`,
  not `FnOnce(E) -> ...`.
- If a Rust function returns `Result` and an `Option` is needed, use `.ok()`
  before `.or_else()` or `.unwrap_or()`.

## Platform-specific code

- `#[cfg(unix)]` code that calls `find_descendant_pids` or similar must have
  stubs for non-Linux/non-macOS Unix targets. Check that
  `#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]` stubs
  exist before flagging missing platform support.
- Windows `unsafe` blocks using `windows_sys` are common for process management.
  Verify handle cleanup (`CloseHandle`) but do not flag the `unsafe` usage
  itself.
