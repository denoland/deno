# Design: a shared fake-timers + mock core for Deno's testing stack (W6)

Status: proposal (design only, no implementation) Author: W6 Depends on: PR
#35297 (W2, branch `fix/node-test-correctness`) landing first Audience: an
implementer who will execute the phased rollout below

## Problem

Deno currently ships three independent virtual-clock / mocking implementations
that overlap heavily but share zero code:

1. **`node:test` `mock.timers`** -- the `MockTimers` class plus `mock.fn` /
   `mock.method` / `mock.property` call tracking, all inside
   `ext/node/polyfills/testing.ts`. (Just merged on the
   `fix/node-test-correctness` branch; not yet on `main`.)
2. **`@std/testing`** -- `FakeTime` in `tests/util/std/testing/time.ts` plus
   `spy` / `stub` / `assertSpyCall*` in `tests/util/std/testing/mock.ts`. The
   vendored copy under `tests/util/std/` is a read-only mirror; the canonical
   source lives in the separate `denoland/std` repo (see
   `tests/util/std/README.md`).
3. **Real runtime timers** -- `ext/web/02_timers.js` (built on
   `core.createTimer`, a Rust event-loop primitive) and
   `ext/node/polyfills/timers.ts` (the `node:timers` / `node:timers/promises`
   wrappers around the same core timers).

The two virtual-clock implementations have diverged in subtle, test-visible ways
(different data structures, different `runAll` termination semantics, different
`Date` mocking strategy, `setImmediate` support in one and not the other).
Maintaining two clocks is wasteful and the divergence is a latent compatibility
hazard. This document proposes a single shared core, decides where it should
live, answers whether Deno should expose a first-class `Deno.test`-friendly mock
API, and lays out a migration plan that does not break the existing node:test
spec suite.

This is an opinionated proposal, not a survey. The recommendations are in
**bold** at the relevant decision points and summarized at the end.

---

## 1. Comparison matrix of the three implementations

Line numbers below refer to the files as they exist on the
`fix/node-test-correctness` worktree
(`/Users/ib/dev/deno2-worktrees/node-test-correctness`) for node:test, and on
`main` for std and ext/web.

### 1.1 Virtual-clock model

| Aspect               | node:test `MockTimers`                                                                                                                                 | `@std/testing` `FakeTime`                                                                                                                                            | Real timers (ext/web)                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Clock storage        | `_now` integer ms field; `MockTimers` at `testing.ts:2117`                                                                                             | module-level `now` (`time.ts:244`)                                                                                                                                   | wall clock, Rust event loop via `core.createTimer` (`02_timers.js:75`) |
| Timer queue          | `SafeMap` id -> timer, **linear scan** to find next (`#findNextTimer`, `testing.ts:2348`) and longest (`#findLongestTimer`, `testing.ts:2363`)         | `RedBlackTree<DueNode>` keyed by due time + `Map<id, DueNode>` (`time.ts:250-251`, `time.ts:185-198`)                                                                | Rust BinaryHeap inside the op layer                                    |
| Advance primitive    | `tick(ms=1)` walks each due timer, setting `_now = next.fireAt` before firing (`testing.ts:2261-2272`)                                                 | `set now(value)` setter drains the tree, setting `now = timer.due` before each callback (`time.ts:501-531`); `tick(ms=0)` just does `this.now += ms` (`time.ts:657`) | n/a (driven by real time)                                              |
| Ordering tie-break   | `fireAt`, then `immediate` first, then lower `id` (`testing.ts:2353-2355`)                                                                             | FIFO within a `DueNode.timers` array at equal `due` (`time.ts:509` `shift()`)                                                                                        | insertion / heap order                                                 |
| Interval re-arm      | `timer.fireAt += interval` in `#fireTimer` (`testing.ts:2379-2380`)                                                                                    | pushes a fresh `DueNode` at `due + delay` (`time.ts:512-520`)                                                                                                        | core re-schedules                                                      |
| `runAll` termination | bounded: ticks up to `#findLongestTimer().fireAt` (`testing.ts:2277-2282`); a bare interval fires a **finite** number of times and re-arms past `_now` | unbounded: `while (!dueTree.isEmpty()) this.next()` (`time.ts:773-777`); a bare interval **never terminates**                                                        | n/a                                                                    |

The `runAll` divergence is the single most important behavioral mismatch and is
called out again in section 5.

### 1.2 Supported APIs

| API                                                        | node:test                                                                   | std FakeTime                                                                  |
| ---------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| `setTimeout` / `clearTimeout`                              | yes (`testing.ts:2186-2195`)                                                | yes (`time.ts:141-156`)                                                       |
| `setInterval` / `clearInterval`                            | yes (`testing.ts:2196-2205`)                                                | yes (`time.ts:158-173`)                                                       |
| `setImmediate` / `clearImmediate`                          | **yes** (`testing.ts:2206-2215`), ordered before zero-delay `setTimeout`    | **no**                                                                        |
| `Date`                                                     | yes (`createMockDate`, `testing.ts:2390`)                                   | yes (`FakeDate` Proxy, `time.ts:77-92`)                                       |
| `AbortSignal.timeout`                                      | no                                                                          | **yes** (`fakeAbortSignalTimeout`, `time.ts:202-208`)                         |
| `queueMicrotask` driving                                   | no                                                                          | indirectly, via `runMicrotasks`/`delay` against real time (`time.ts:616-618`) |
| per-API selection (`enable({ apis })`)                     | yes (`SUPPORTED_APIS`, `testing.ts:2065`; `_apiEnabled`, `testing.ts:2135`) | no (all-or-nothing in `overrideGlobals`, `time.ts:210-217`)                   |
| `advanceRate` auto-tick                                    | no                                                                          | yes (`time.ts:341-345`)                                                       |
| `node:timers` / `node:timers/promises` module interception | **yes** (`kInstallMockTimers`, see 1.4)                                     | **no** (globals only)                                                         |

### 1.3 Date mocking

| Aspect                          | node:test                                                                                                                                                  | std FakeTime                                                 |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Mechanism                       | a plain function `MockDate` using `ReflectConstruct(originalDate, ...)` (`testing.ts:2391-2401`); shares `originalDate.prototype` (`testing.ts:2402-2406`) | a `Proxy(Date, { construct, apply, get })` (`time.ts:77-92`) |
| `Date.now()`                    | `() => mockTimers._now` (`testing.ts:2412`)                                                                                                                | `fakeTimeNow()` reads `time?.now` (`time.ts:73-75`)          |
| Marker                          | `Date.isMock = true`, `Date.toString()` returns native-code string (`testing.ts:2415-2416`)                                                                | none                                                         |
| `Date.parse` / `Date.UTC`       | forwarded to original (`testing.ts:2413-2414`)                                                                                                             | forwarded via Proxy `get`                                    |
| `now` accepts a `Date` instance | yes, normalized via `DatePrototypeGetTime` (`testing.ts:2163-2164`)                                                                                        | yes, `start instanceof Date` (`time.ts:310-311`)             |

### 1.4 Interception mechanism

| Aspect          | node:test                                                                                                                                                                                                                                                                        | std FakeTime                                                                                                                    |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Globals         | `#mockGlobal(name, value)` saves originals into a `SafeMap` and overwrites `globalThis[name]` (`testing.ts:2125-2130`)                                                                                                                                                           | `overrideGlobals()` assigns `globalThis.*` directly; `restoreGlobals()` reads from `_internals` (`time.ts:210-226`, `_time.ts`) |
| node modules    | `core.loadExtScript("ext:deno_node/timers.ts")[kInstallMockTimers](this)` installs the live instance into `timers.ts` (`testing.ts:2220`); each wrapper checks `mockTimers !== null && mockTimers._apiEnabled(api)` per call (`timers.ts:61, 111, 154, 167, 188, 221, 267, 312`) | not intercepted -- `import { setTimeout } from "node:timers"` keeps the real timer                                              |
| Restore         | iterate saved originals, restore, clear maps, set `_enabled=false` (`reset`, `testing.ts:2223-2237`); also `[kInstallMockTimers](null)`                                                                                                                                          | `time = undefined; restoreGlobals()` (`time.ts:830-837`)                                                                        |
| Singleton guard | `_enabled` flag; throws `ERR_INVALID_STATE` on double-enable (`testing.ts:2140-2143`)                                                                                                                                                                                            | module-level `time` var; throws `TimeError` if already faked (`time.ts:306-308`)                                                |

### 1.5 Public API surface

| node:test (`test.mock`)                                                                                                            | std (`@std/testing`)                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `mock.timers.enable({ apis, now })`                                                                                                | `new FakeTime(start?, { advanceRate, advanceFrequency })`                               |
| `mock.timers.tick(ms=1)`                                                                                                           | `time.tick(ms=0)`, `time.tickAsync(ms)`                                                 |
| `mock.timers.runAll()`                                                                                                             | `time.runAll()`, `time.runAllAsync()`, `time.next()`, `time.nextAsync()`                |
| `mock.timers.setTime(ms)`                                                                                                          | `time.now = ms` setter, `get now`, `get start`                                          |
| `mock.timers.reset()` / `[Symbol.dispose]`                                                                                         | `time.restore()` / `FakeTime.restore()` / `[Symbol.dispose]`                            |
| timer handles: `ref/unref/hasRef/refresh` (`MockTimersHandle`, `testing.ts:2072`)                                                  | numeric ids only                                                                        |
| `mock.fn/method/getter/setter/property` (`testing.ts:2422-2531`)                                                                   | `spy()` / `stub()` (`mock.ts:818, 1055`)                                                |
| call record shape: `{ arguments, error, result, stack, target, this }` (`MockFunctionContext._recordCall`, `testing.ts:1785-1794`) | `SpyCall { args, returned?, error?, self? }` (`mock.ts:359-375`)                        |
| `mock.reset()` (reset call records) / `mock.restoreAll()` (`testing.ts:2497-2508`)                                                 | `restore(id?)` + `mockSession` / `sessions` registry (`_mock_utils.ts:24-41`)           |
| n/a                                                                                                                                | `restoreFor`, `delay`, `runMicrotasks`, rich `assertSpyCall*` helpers (`mock.ts:1174+`) |

### 1.6 Where they diverge in behavior/semantics

1. **`runAll` on a bare interval**: node:test bounds and terminates; std loops
   forever (relies on the test clearing the interval). A unified core must pick
   one and gate the other behind an option (section 5).
2. **`setImmediate`**: present and ordered in node:test, absent in std.
3. **`tick` default arg**: `tick(1)` (node) vs `tick(0)` (std).
4. **Date overflow / delay clamping**: node clamps `delay > 2147483647` to `1`
   and `delay < 0`/non-finite to `1` (`testing.ts:2299-2300`); std uses
   `Math.max(repeat ? 1 : 0, Math.floor(delay))` (`time.ts:183`), so a
   zero-delay `setTimeout` is allowed `due=now` while an interval floors to 1.
5. **Error types**: `ERR_INVALID_STATE` / `ERR_INVALID_ARG_VALUE` (node) vs
   `TimeError` / `RangeError` (std).
6. **Module interception**: node intercepts `node:timers*`; std does not.
7. **Async tick / microtask draining**: std-only (`tickAsync`, `runMicrotasks`,
   `delay`, `runAllAsync`).
8. **Call-record shape**: `MockFunctionContext` vs `SpyCall` are structurally
   different and intentionally Node-shaped vs std-shaped.

---

## 2. Proposed shared core

### 2.1 Scope of "core"

Split the shared surface into two cleanly separable primitives:

- **VirtualClock** -- the clock + timer queue + the deterministic fire/advance
  algorithm. This is where node:test and std overlap almost entirely.
- **CallTracker** -- the spy/stub/call-recording primitive. node:test's
  `MockFunctionContext` and std's `spy`/`stub` overlap on the idea of "wrap a
  callable, record `{args, returned, error, self/this}` per call, support
  restore". This is a smaller, lower-risk shared surface.

**These are independent.** Land VirtualClock first; CallTracker is optional and
lower priority (section 7).

### 2.2 VirtualClock primitive

A single class with this shape (names illustrative):

```
class VirtualClock {
  now: number
  // schedule, returns an opaque numeric id
  schedule(callback, delay, args, { repeat, immediate }): number
  cancel(id): void
  refresh(id): void                 // re-arm a one-shot from current now
  ref(id) / unref(id) / hasRef(id)  // bookkeeping only; no real timer
  advanceTo(target): void           // fire all timers with fireAt <= target,
                                    // setting now to each fireAt before firing
  next(): boolean                   // advance to the soonest timer, fire it
  peekLongest(): number | null      // for bounded runAll
  reset(): void
}
```

Design decisions baked into the core:

- **Firing model = node:test's "advance to each `fireAt`"**
  (`testing.ts:2261-2272`). std's `now` setter already does the same thing
  (`time.ts:501-531`); both set the clock to the scheduled time before invoking
  the callback, so a callback reading `Date.now()` sees its scheduled time. This
  is the behavior the node:test spec asserts (`test.js:262-287`) and is the more
  correct of the two. Keep it.
- **Queue = a sorted structure, not a linear scan.** Adopt std's ordered tree
  idea over node's `SafeMap` linear scan (`testing.ts:2348-2371`, O(n) per
  advance step). The core must remain primordial-safe inside ext/node (section
  6), so use a small internal binary-heap / sorted-insert keyed by
  `(fireAt, immediate desc, id)` rather than pulling in `@std/data-structures`
  `RedBlackTree`. The tie-break ordering must reproduce node:test's
  `(fireAt, immediate, id)` rule (`testing.ts:2353-2355`) so
  `setImmediate`-before-zero-`setTimeout` keeps working (`test.js:113-125`).
- **`runAll` is parameterized**, not hardcoded: `runAll({ bounded: true })`
  ticks up to `peekLongest()` (node behavior, the default for node:test);
  `runAll({ bounded: false })` loops `next()` until empty (std behavior). This
  is how the core reconciles the divergence in 1.6.1 without changing either
  consumer's observable behavior.
- **`setImmediate` is a first-class scheduling flag** (`immediate: true`), so
  std can opt in later for free.
- **Delay clamping is a policy injected by the caller**, not hardcoded: node
  passes its clamp (`< 0`/non-finite/`> 2^31-1` -> 1, `testing.ts:2295-2300`);
  std passes `Math.max(repeat?1:0, floor(delay))` (`time.ts:183`). The core
  takes an already-normalized integer delay and does not re-interpret it.
- **Date mocking stays a thin adapter on top of the core**, reading `clock.now`.
  node keeps `createMockDate` (`ReflectConstruct`, primordial-friendly,
  `testing.ts:2390`); std keeps its `Proxy` flavor. The core does not own
  `Date`; it only owns the clock value. This avoids forcing one Date strategy on
  both and keeps the primordials surface small.

### 2.3 Where does the core live? JS, Rust op, or pure JS?

**Recommendation: pure JS, shipped as an internal ext module
(`ext:deno_web/timers_virtual.js` or a new `ext:deno_node`-adjacent module),
authored against primordials. No Rust op.**

Justification:

- **A Rust op for the clock buys nothing.** The whole point of a virtual clock
  is that it is _not_ the real `core.createTimer` Rust clock
  (`02_timers.js:75`). There is no shared state with the event loop to protect,
  no syscall to make, and no cross-thread concern: a fake clock is a
  deterministic in-memory integer plus a sorted list, mutated only from the
  single isolate that owns the test. Putting it in Rust would add an op-call per
  `schedule`/`tick` and a serialization boundary for zero benefit, and would
  make it _harder_ for std (which cannot call internal ops from JSR) to ever
  consume it.
- **Pure JS keeps std able to vendor it.** std lives in a separate repo and is
  distributed via JSR; it can only depend on public JS/Web APIs. A pure-JS core
  can be published (e.g. as a tiny `@std/internal`-style module or copied) and
  consumed identically by both ext/node and std. An op cannot.
- **Primordials are the only constraint.** ext/node code must be written against
  `primordials` (the layer was converted in commit `9935b92a86`, "use
  primordials across the node:* polyfill layer"). std code must _not_ use
  primordials (it is user-facing library code). This is the one real tension and
  it is addressed in section 6: ship the canonical core as primordial-clean
  internal JS for the runtime, and let std carry a primordials-free
  transcription (they are the same algorithm; std already has the algorithm in
  `time.ts`).

So: **one canonical algorithm, expressed as primordial-safe internal JS in the
Deno runtime; std adopts the same algorithm (not a literal byte copy) in its own
repo.** The "shared core" is shared _by design and by tests_, not necessarily by
a single linked artifact, because the runtime/JSR boundary forbids that. See
section 5 for how the two stay in lockstep.

---

## 3. The native-API question

**Should Deno expose a first-class `Deno.test`-friendly fake-timers / mock API
on top of this core, or keep `@std/testing/mock` as the blessed path?**

**Recommendation: keep `@std/testing` as the blessed path for `Deno.test` users.
Do NOT add a `Deno.*` native mock/fake-timers API.**

Tradeoffs considered:

| Factor                  | Native `Deno.*` API                                                                                                                                                                                                                                                                             | Keep `@std/testing`                                                                                |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Discoverability         | better (no import)                                                                                                                                                                                                                                                                              | weaker, but `@std/testing` is already the documented answer and is widely used                     |
| std-vs-runtime boundary | bad: mocking is a _testing-library_ concern, not a runtime capability. The runtime boundary should expose system access (fs/net/timers), not test ergonomics                                                                                                                                    | good: keeps the runtime surface minimal and stable; testing helpers evolve in std at std's cadence |
| Startup / bundle cost   | bad: every `deno` process would carry spy/stub/assert code in the snapshot even though almost no program mocks. The team has spent real effort lazy-loading polyfills (commits `d60510ad72`, `6100ec3a87`, `e84fc15522`) precisely to keep startup lean; a native mock API pushes the other way | good: zero cost for non-test code; std is imported only by tests that need it                      |
| Node parity             | node:test _already_ provides `mock.*` for users who want the Node API; a third Deno-branded API would be a _third_ spelling of the same thing and worsen the "which one do I use" problem                                                                                                       | good: two clear lanes -- `node:test` for Node-style, `@std/testing` for Deno-style                 |
| API churn risk          | high: a `Deno.fakeTimers` is forever; std can rev majors                                                                                                                                                                                                                                        | low                                                                                                |

The strongest argument _for_ a native API is discoverability, and it is real but
weak: the fix is documentation (point `Deno.test` users at `@std/testing`), not
a new runtime surface. The startup-cost and boundary arguments are decisive
against. **Verdict: no native mock API; std stays blessed; node:test stays the
Node-compatible path.** The shared core makes both of those better without
adding a third public surface.

---

## 4. Migration plan -- node:test onto the shared core, without behavior change

The node:test spec suite at `tests/specs/node/node_test_mock_timers/test.js`
(369 lines, 30+ cases) is the contract. The refactor must keep `test.out`
byte-identical. Key invariants the plan must preserve, with the asserting test:

- `tick` advances clock to each `fireAt` (`test.js:262-274`).
- interval callbacks see their scheduled time under `runAll`
  (`test.js:276-287`).
- `setImmediate` before zero-delay `setTimeout` (`test.js:113-125`).
- bounded `runAll`: interval fires a finite count up to longest timer
  (`test.js:157-184`).
- overflow delay clamps to 1ms (`test.js:127-136`).
- throwing callback propagates synchronously out of `tick` (`test.js:186-194`).
- `refresh()` re-arms a fired one-shot (`test.js:86-100`).
- handle coerces to numeric id; `clearTimeout(+id)` works (`test.js:73-84`).
- Date mock: `isMock`, native-code `toString`, `parse`/`UTC`, `Date()` call
  form, `now` as a `Date` instance (`test.js:196-219`).
- `node:timers` and `node:timers/promises` interception, including the per-API
  "untouched when not enabled" case (`test.js:289-348`).
- `ERR_INVALID_STATE` / `ERR_INVALID_ARG_VALUE` error codes (`test.js:350-369`).

Refactor steps (each is a no-op behaviorally and can be reviewed in isolation):

1. **Extract `VirtualClock`** from `MockTimers` by moving `_setTimeout`,
   `_setInterval`, `_clearTimer`, `#findNextTimer`, `#findLongestTimer`,
   `#fireTimer`, `tick`, `runAll`, and the `_now`/`_timers`/`_nextId` state
   (`testing.ts:2295-2384`, `2257-2289`) into the new internal module. Replace
   the linear scans with the sorted heap, asserting the
   `(fireAt, immediate, id)` tie-break is identical (covered by
   `test.js:113-125`).
2. **`MockTimers` becomes a thin adapter** over `VirtualClock`: it keeps
   `enable`/`reset`/`setTime`/`_apiEnabled`, the `#mockGlobal` save/restore
   (`testing.ts:2125-2130`), `createMockDate` wiring, and the
   `kInstallMockTimers` install/uninstall (`testing.ts:2220, 2225`). `tick`
   delegates to `clock.advanceTo(now + ms)`; `runAll` calls
   `clock.runAll({ bounded: true })`.
3. **`MockTimersHandle`** (`testing.ts:2072-2110`) keeps its identity but
   delegates `refresh`/`ref`/`unref` to the clock; `[SymbolToPrimitive]` and
   `_id` semantics are unchanged so `+id` / `clearTimeout(+id)` keep working
   (`test.js:73-84`).
4. **`timers.ts` is untouched.** The `kInstallMockTimers` symbol
   (`timers.ts:25, 388-390`) and the per-call `_apiEnabled` checks
   (`timers.ts:61` etc.) keep talking to the `MockTimers` adapter, whose
   `_setTimeout`/`_setInterval`/`_clearTimer`/`_apiEnabled` signatures are
   preserved verbatim. This is critical: changing those signatures would break
   module interception (`test.js:289-348`). Keep them as the stable contract
   between `testing.ts` and `timers.ts`.
5. **Run the spec** (`cargo test specs::node::node_test_mock_timers` or the
   project's `./x test-spec` filter) and confirm `test.out` is identical.

No `test.out` edits. If a step changes output, the step is wrong (per the
project rule: fix implementation, never the expectation).

### 4.1 How `@std/testing` FakeTime adopts the core

std lives in `denoland/std` (separate repo); the vendored
`tests/util/std/testing/` here is a mirror and must not be hand-edited as the
source of truth. Adoption is therefore a **coordination task**, not an in-repo
refactor:

1. Land the canonical primordial-clean `VirtualClock` algorithm in the Deno
   runtime first (it backs node:test).
2. Open a companion `denoland/std` PR that re-expresses `FakeTime` on top of the
   _same algorithm_ (primordials stripped, `RedBlackTree` kept or swapped for
   the heap -- std may keep its tree since it has no primordials constraint).
   The behavioral target is: `runAll({ bounded: false })` (std's current
   unbounded semantics, `time.ts:773-777`), `tick(0)` default, no `setImmediate`
   unless std opts in, `AbortSignal.timeout` and `advanceRate` remain std-only
   adapters.
3. Add a **shared conformance test vector** (a JSON list of
   `schedule`/`advance`/`expect-fired` steps) checked into both repos so the two
   implementations are provably the same algorithm. This is the mechanism that
   keeps a single-artifact-less "shared core" honest across the repo boundary.
4. Once std adopts it, re-vendor into `tests/util/std/` via the normal std
   vendoring process (do not hand-edit the mirror).

Because std is versioned and shipped on JSR independently, **the std side is
best-effort and decoupled**: the runtime refactor (steps in section 4) must not
block on it. If std never adopts, node:test still gets the better core, and the
two implementations simply continue to coexist with a documented conformance
vector aspiration.

---

## 5. Risks

1. **Primordials / prototype-pollution (ext/node).** The canonical core runs
   inside ext/node, which was converted to primordials in `9935b92a86` and is
   linted with `prefer-primordials`. The core must use `SafeMap`,
   `ArrayPrototype*`, `MapPrototype*`, `ReflectApply`, etc. -- never bare
   `Array#sort` or object-literal iteration that a malicious test could poison.
   The existing `MockTimers` already does this (`SafeMapIterator` at
   `testing.ts:2350`, `ReflectApply` at `testing.ts:2378`); the heap must keep
   the same discipline. Conversely, the std transcription must _not_ use
   primordials (it is user library code), which is exactly why the "single
   linked artifact" approach is rejected in section 2.3.

2. **node:timers-module interception subtlety.** The interception is _per-call
   and per-API_, not a one-time swap: every wrapper in `timers.ts` re-checks
   `mockTimers !== null && mockTimers._apiEnabled(api)` on each invocation
   (`timers.ts:61, 111, 154, 167, 188, 221, 267, 312`). This is what makes
   `test.js:335-348` ("module setTimeout untouched when its api is not enabled")
   pass. The refactor must keep `_apiEnabled` and the
   `_setTimeout/_setInterval/_clearTimer` entry points on the _adapter_
   (`MockTimers`), not bury them in `VirtualClock`, or interception breaks. Also
   note `setTimeoutPromise`/`setIntervalP` route `resolve` through the mock
   (`timers.ts:111-113, 312-313`), and the promises-API tests
   (`test.js:289-321`) depend on that; the adapter's entry points feed those
   exactly as today.

3. **`runAll` termination semantics (the std/Node reconciliation).** node:test
   `runAll` is _bounded_ by the longest pending timer (`testing.ts:2277-2282`),
   so a bare interval fires finitely (`test.js:157-166`). std `runAll` is
   _unbounded_ (`while !dueTree.isEmpty()`, `time.ts:773-777`) and would spin
   forever on a bare interval. **The unified core must not pick one silently.**
   It exposes `runAll({ bounded })` and each consumer passes its own value
   (node: `true`, std: `false`). Getting this wrong would either hang node:test
   or change std's documented behavior.

4. **Ordering / tie-break divergence.** node orders equal-`fireAt` timers by
   `(immediate, id)` (`testing.ts:2353-2355`); std uses FIFO array order within
   a `DueNode` (`time.ts:509`). Since std has no `setImmediate`, the only
   observable case is equal-delay `setTimeout`s, where both happen to be
   insertion order. The core's `(fireAt, immediate, id)` comparator subsumes
   both, but the std transcription must confirm equal-delay FIFO is preserved
   (covered by the conformance vector in 4.1.3).

5. **Date strategy mismatch is intentional, keep it.** Forcing node's
   `ReflectConstruct` Date onto std (or std's `Proxy` Date onto node) is a trap:
   node's spec asserts `Date.isMock`, native-code `toString`, and the `Date()`
   (no-`new`) call form (`test.js:200-202`), which the `Proxy` apply-trap
   returns differently. The core deliberately does _not_ own `Date`; each
   consumer keeps its adapter reading `clock.now`. Do not unify Date.

6. **Singleton/global-restore correctness.** Both implementations save and
   restore `globalThis` timer functions; node also restores via
   `kInstallMockTimers(null)` and clears its maps (`testing.ts:2223-2237`), and
   supports `using` via `[SymbolDispose]` (`testing.ts:2291-2293`, asserted at
   `test.js:252-260`). The adapter, not the core, owns global/module restore;
   the core only owns the clock + queue, so a refactor bug there cannot leave
   globals patched after `reset()`.

7. **CallTracker unification is lower-value and riskier.** The `SpyCall`
   (`mock.ts:359-375`) and `MockFunctionContext` (`testing.ts:1734-1815`) record
   shapes are intentionally different (Node's
   `{arguments,result,error,stack,target,this}` vs std's
   `{args,returned,error,self}`). A shared CallTracker would have to keep both
   public shapes as thin views over one internal record. **Defer this** -- the
   clock is where the real duplication and divergence risk live; the spy layer
   is mostly parallel-but-independent and not currently diverging in ways that
   bite users.

---

## 6. Primordials strategy (the one cross-cutting concern)

Concretely:

- The runtime-side canonical core (`VirtualClock`) is authored in a single
  internal `.js` under an ext (sibling to `ext/node/polyfills/timers.ts`), fully
  primordial-clean, and `loadExtScript`-loaded by `testing.ts`. This keeps it
  inside the existing `prefer-primordials` lint gate.
- The std-side copy is plain library JS (no primordials), living in
  `denoland/std`.
- A checked-in **conformance vector** (section 4.1.3) is the contract that makes
  them "the same core" despite being two source files in two repos. This is the
  pragmatic resolution of the runtime/JSR boundary: we get shared _behavior_ and
  shared _tests_ even though a single linked module is impossible across that
  boundary.

---

## 7. Phased rollout (sequenced after PR #35297 / W2)

PR #35297 (`fix/node-test-correctness`) is in flight and is actively editing
`ext/node/polyfills/testing.ts` (it adds `runWithTestGuards`, the failure-sink
stack, and timeout/abort handling, and it relies on the
`realSetTimeout`/`realClearTimeout` captured at `testing.ts:59-60` so the mock
clock cannot suppress a real test timeout). Touching `testing.ts` before it
lands would collide. Sequence:

- **Phase 0 -- block until W2 lands.** Do nothing in `testing.ts` until #35297
  is merged to `main` and this branch is rebased onto it. (This design doc is
  the only artifact produced before then.)

- **Phase 1 -- extract `VirtualClock` (runtime only, no behavior change).**
  Steps 1-3 of section 4. New internal ext JS module + `MockTimers` adapter.
  Gate: `node_test_mock_timers` spec `test.out` unchanged; full node:test spec
  suite green. No std involvement.

- **Phase 2 -- harden + document the core.** Add the conformance vector (section
  4.1.3) on the runtime side, exercised against `VirtualClock`. Document the
  `runAll({ bounded })` contract and the adapter-owns-globals/Date boundary
  inline. Still runtime-only.

- **Phase 3 -- std adoption (separate `denoland/std` PR, decoupled).**
  Re-express `FakeTime` on the shared algorithm; ship the same conformance
  vector in std; re-vendor into `tests/util/std/`. Best-effort, does not block
  Phases 1-2. `runAll` stays `bounded:false`, `tick(0)` default, std-only
  adapters (`AbortSignal.timeout`, `advanceRate`, `tickAsync`/`runMicrotasks`)
  preserved.

- **Phase 4 (optional, deferred) -- CallTracker.** Only if the spy/stub
  duplication starts to actually diverge. Unify `MockFunctionContext` and
  `spy`/`stub` over one internal record with two public views. Lowest priority;
  explicitly out of scope for the initial effort.

No native `Deno.*` mock API is introduced at any phase (section 3).

---

## Summary of recommendations

- **Shared core = `VirtualClock`** (clock + sorted timer queue +
  advance-to-each-`fireAt` firing), authored as **pure, primordial-clean
  internal JS in the Deno runtime** -- _not_ a Rust op (no event-loop state to
  protect, and an op would lock std out) and _not_ a single cross-repo artifact
  (the runtime/JSR primordials boundary forbids it). std consumes the _same
  algorithm_ via a primordials-free transcription kept honest by a checked-in
  conformance vector.
- **`runAll` is parameterized** (`bounded` true for node, false for std) to
  reconcile the one hard behavioral divergence; **Date and global/module
  interception stay consumer-owned adapters**, deliberately not unified.
- **No native `Deno.*` mock/fake-timers API.** Keep `@std/testing` as the
  blessed Deno-style path and `node:test`'s `mock.*` as the Node-compatible
  path; a third surface would cost startup/snapshot size and muddy the
  runtime/library boundary for only a marginal discoverability gain.
- **node:test migration is a behavior-preserving extract** validated against the
  existing `node_test_mock_timers` spec (`test.out` must stay identical); **std
  adoption is a decoupled `denoland/std` coordination task.**
- **Everything is sequenced after PR #35297 (W2)** to avoid colliding with
  in-flight `testing.ts` work; CallTracker unification is explicitly deferred.
