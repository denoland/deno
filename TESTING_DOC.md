# Testing: `node:test` + mocking — state and plan

Status of Deno's `node:test` module and mocking support, and a plan to improve
both. Living document. Last updated 2026-06-17.

## Current state

The polyfill in `ext/node/polyfills/testing.ts` (~1961 lines) is substantially
complete. What works today:

- `test` / `it` / `describe` / `suite`, plus `.skip` / `.only` / `.todo`
- All four hooks: `before` / `after` / `beforeEach` / `afterEach` (issue #29360
  is stale; already implemented)
- Subtests, `t.plan` / `t.diagnostic` / `t.assert`
- Mock surface: `mock.fn` / `mock.method` / `mock.getter` / `mock.setter` /
  `mock.property` / `mock.reset` / `mock.restoreAll`, with call tracking
- Reporters: tap, spec, dot, junit (lcov is a stub)
- TAP mode and a watch event stream

### Verified gaps

| Gap                                          | Status                                                  | Issue / PR                  |
| -------------------------------------------- | ------------------------------------------------------- | --------------------------- |
| `mock.timers.*`                              | `notImplemented` stub (testing.ts:1924-1937)            | #32987, PR #33946 in flight |
| Unhandled rejection does not fail test       | broken                                                  | #34818                      |
| `test.run()` file discovery/execution        | watch-event stub only                                   | #32929                      |
| `mock.module` (ESM/CJS module mocking)       | absent from mock object                                 | none yet                    |
| `timeout` / `signal` / `concurrency` options | parsed but not enforced                                 | none                        |
| `t.assert.snapshot`                          | absent (Deno-native in PR #35139)                       | none                        |
| Coverage via `node:test`                     | absent; compat tests commented (config.jsonc:3179-3194) | none                        |
| Native `Deno.*` mock / fake-timers           | none; only `@std/testing/mock`                          | none                        |

## Architectural note

Three independent timer-interception implementations are about to coexist: PR
#33946's `MockTimers`, `@std/testing`'s `FakeTime`, and a potential Deno-native
one. Build one shared virtual-clock + mock core (a JS primitive, possibly with a
thin op for the clock) that `node:test` mock, `@std/testing/mock`, and a future
`Deno.test` mock all consume. This is the spine; the rest hangs off it.

## Plan of action (6 workstreams)

Ordered by priority. W1 and the W6 decision go first; the rest can run in
parallel.

### W1 — Land what is in flight (quick wins)

- Finish PR #33946 `mock.timers` (address review) -> closes #32987
- Verify hooks and close stale issue #29360
- Re-enable commented node-compat test-runner entries that now pass
  (config.jsonc:3179-3194: coverage-thresholds, coverage-source-map, inspect)

### W2 — Correctness fixes

- #34818: fail a test on unhandled promise rejection (Node parity)
- Enforce `timeout`: a hung test must fail; wire `t.signal` to fire on timeout /
  `signal` option
- Then `concurrency`

### W3 — Real `test.run()` programmatic runner (#32929)

Replace the watch stub with real file discovery + execution emitting structured
`test:start` / `test:pass` / `test:fail` / `test:plan` / `test:diagnostic` /
`test:coverage` events. Foundation for IDE test explorers and CI integrations.

### W4 — `mock.module`

Implement ESM + CJS module mocking. Aligns with the registerHooks module
customization work in flight (#35026 / #35027 / #35028); build on those hooks
rather than a parallel mechanism. Do not start implementation until those
settle.

### W5 — Snapshot + coverage bridges

Bridge `t.assert.snapshot` / `--test-update-snapshots` onto the Deno-native
snapshot work in PR #35139. Map node:test coverage flags onto Deno's existing
coverage engine.

### W6 — Shared mocking core (the spine; decide first)

Build the shared mock + fake-timers core (a pure, primordial-clean
`VirtualClock`: clock + sorted timer queue, in internal runtime JS — not a Rust
op). `runAll({ bounded })` is parameterized to reconcile node:test (bounded) vs
std `FakeTime` (unbounded) `runAll`. Date mocking and module interception stay
consumer-owned adapters. Prevents the three-implementations problem.

Design doc: `docs/designs/testing_mock_core.md`.

DECISION (2026-06-18): we DO want proper, first-class mocking inside `node:test`
(the Node-compat path stays and grows: mock.fn/method/timers + mock.module in
W4). The design doc's "no native API, keep @std as blessed" lean is overruled on
the mocking-completeness point. The hard constraint is **zero-cost when not
used**: the entire mock subsystem (the shared `VirtualClock` core, mock.fn,
mock.module, mock.timers) must be lazy-loaded on first `mock.*` use and stay out
of the startup snapshot, so importing `node:test` without mocking pays nothing.
node:test is already lazy on import; split the mock machinery into a separate
`loadExtScript`-loaded sub-module gated behind first access. The doc must be
revised to (a) build node:test mocking rather than defer to @std, and (b) add a
lazy-loading / zero-startup-cost section specifying the exact mechanism.

### Suggested sequencing

Make the W6 core decision -> land W1 -> fan W2 / W3 / W4 out in parallel -> W5
last.
