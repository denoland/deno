# deno_core runtime optimization opportunities

Deep analysis of `libs/core/` focused on the Rust↔V8 boundary: the `op2`
dispatch layer (`libs/ops/op2/`), the async op completion pipeline, and the
per-tick cost of `poll_event_loop_inner`. Goal: lower the cost of crossing the
boundary and lower CPU when the runtime is idle or near-idle.

All line numbers are against the current `feat/musl-builds` checkout
(2026-07-18). Analysis only; nothing here has been implemented or benchmarked.

## How to read the cost model

A typical "one async op completed" event-loop tick today performs:

- 1 isolate scope + 1 context scope + 3-4 `TryCatch` scopes (FFI pairs each)
- **2 Rust→JS `Function::call`s**: `__eventLoopTick` and
  `__drainNextTickAndMacrotasks` (`jsruntime.rs:3554`, `jsruntime.rs:3654`)
- **1 JS→Rust op call**: `drainTicks()` calls `op_run_microtasks()`
  (`01_core.js:426`, `ops_builtin_v8.rs:233`)
- **3-4 `perform_microtask_checkpoint()` FFI calls**, at least two of which are
  guaranteed no-ops (`jsruntime.rs:2445`, `2492`, `2515`, `2586`)
- per completed op: 1 `v8::Integer::new` + 1 `v8::Boolean::new` FFI, a `RefCell`
  borrow + `HashSet::remove` on `unrefed_ops`, and a `RefCell` borrow +
  `BTreeMap::remove` on `activity_traces` (`jsruntime.rs:3533-3539`)
- 1 `Mutex` lock on `foreground_tasks` (`jsruntime.rs:2437`)
- `EventLoopPendingState::new`: ~8 `RefCell` borrows + 1
  `has_pending_background_tasks()` FFI (`jsruntime.rs:3010-3063`)

The essential work in that tick is one JS call and one microtask checkpoint.
Most items below attack the difference.

Validation targets: `cargo bench` in `libs/core` (`benches/ops/async.rs`,
`benches/ops/sync.rs`), plus a macro benchmark like a hello-world `Deno.serve`
under `wrk`/`oha` (per-request cost is dominated by op dispatch + tick overhead)
and `deno run` of a `setTimeout`-heavy script.

---

## 1. Collapse the per-tick JS entries and skip provably-empty microtask checkpoints

**Where:** `jsruntime.rs:2420-2627` (`poll_event_loop_inner`),
`jsruntime.rs:3644-3661` (`drain_next_tick_and_macrotasks`),
`01_core.js:424-432` (`drainTicks`).

**Problem.** `drain_next_tick_and_macrotasks` unconditionally calls into JS
(`__drainNextTickAndMacrotasks` → `drainTicks`). `drainTicks`' first action is
to check `hasTickScheduled()`/`hasRejectionToWarn()` — flags Rust _already has
in shared memory_ (`tick_info` buffer, `ContextState::has_tick_scheduled()`,
`jsrealm.rs:175`; `tick_info[1]` is `hasRejectionToWarn`, written by Rust in
`bindings.rs:1685-1693`). In the common case (no `process.nextTick` users — i.e.
all pure-Deno code) `drainTicks` then just calls `op_run_microtasks()`, which is
a JS→Rust op that calls `perform_microtask_checkpoint`, and returns. So the
common path is: Rust→JS call → JS→Rust op call → V8 checkpoint → return →
return, when it could be a single `perform_microtask_checkpoint()` call from
Rust.

Additionally, `poll_event_loop_inner` performs 3 unconditional
`perform_microtask_checkpoint()` calls per tick (lines 2515, 2586, plus the
pre-phase one at 2445 guarded only by `has_tick_scheduled`). A tick that
dispatched no JS at all (e.g. spurious wake, or the extra self-woken tick after
`uv_did_io`) still pays all of them. If no JS ran since the previous checkpoint,
the microtask queue is necessarily empty — V8 only enqueues microtasks while JS
executes or when the embedder explicitly enqueues.

**Proposed change.**

1. Reimplement `drain_next_tick_and_macrotasks` in Rust mirroring `drainTicks`
   exactly:

   ```rust
   fn drain_next_tick_and_macrotasks(scope, context_state) -> Result<...> {
     if !context_state.has_tick_scheduled()
        && !context_state.has_rejection_to_warn() {
       scope.perform_microtask_checkpoint();
       if !context_state.has_tick_scheduled()
          && !context_state.has_rejection_to_warn() {
         return Ok(());   // zero JS calls on the common path
       }
     }
     // slow path: call the JS processTicksAndRejections directly
     call_js_process_ticks_and_rejections(scope, context_state)
   }
   ```

   The JS callback then only needs to be `processTicksAndRejections` itself (its
   internal `op_run_microtasks()` loop stays as-is for the nextTick case). This
   preserves the nextTick-before-`.then` ordering invariant documented at
   `01_core.js:410-423` because the flag checks and checkpoint ordering are
   byte-for-byte the same, just performed on the Rust side.

2. Track a `ran_js_since_checkpoint: Cell<bool>` on `ContextState`, set by every
   helper that enters JS (`dispatch_event_loop_tick`, `dispatch_user_timers`,
   task spawner, immediates, uv callbacks) and cleared by each checkpoint. Guard
   the checkpoints at lines 2445, 2515, 2527, 2549, 2586, 2626 with it. An empty
   tick then performs **zero** checkpoint FFI calls instead of 3-4.

**Expected impact.** Removes one Rust→JS `Function::call`, one JS→Rust op call,
and 2-3 checkpoint FFI calls per tick on op-driven workloads. On an async-op
microbench (`benches/ops/async.rs`, `op_async_void_deferred`-style round trips)
tick overhead is a large fraction of total time; expect >10% there and
measurable wins (a few %) on `Deno.serve` req/s. Also directly reduces
per-wakeup CPU when near-idle (each I/O event currently triggers a full-fat tick
plus one self-woken extra tick).

**Risks.** The ordering invariants (nextTick before `.then`, `rejectionhandled`
before `unhandledrejection`) are covered by `tests/specs` node-compat tests and
`libs/core` integration tests; run `unhandled_rejection`/`nextTick` spec tests.
`tick_info[1]` is written from Rust and read from JS via the shared buffer —
reads on the Rust side need the same "single-threaded, no sync needed"
justification already documented in `bindings.rs:1687`.

---

## 2. Delete the middleman task in `FuturesUnorderedDriver` — poll completions inline

**Where:** `runtime/op_driver/futures_unordered_driver.rs:33-45` (`poll_task`),
`:100-125`, `:212-226` (`poll_ready`).

**Problem.** Async op completions take two scheduler hops to reach JS. The
driver spawns a dedicated `deno_unsync` task (`poll_task`) whose only job is to
poll the `FuturesUnordered` and shovel results into a `VecDeque<PendingOp>`
(`completed_ops`), then wake the event-loop waker:

- I/O readiness fires an op future's waker → wakes `poll_task`
- tokio schedules and polls `poll_task` → drains `FuturesUnordered`,
  `borrow_mut` + `push_back` per op, `wake_by_ref` per op (line 43-44)
- main event-loop task is woken → tokio schedules and polls it → `poll_ready`
  pops from the `VecDeque`

Every completion batch pays an extra task wakeup, an extra tokio scheduling
round, and a `VecDeque` handoff with per-item `RefCell` traffic. The event-loop
future and `poll_task` run on the same thread, so the indirection buys nothing —
it is a leftover of the earlier `JoinSet`-based driver.

**Proposed change.** Keep the `SubmissionQueue` (needed so `submit_op` can push
while unpolled), but store the `SubmissionQueueResults` in the driver and poll
it directly from `poll_ready`:

```rust
fn poll_ready(&self, cx) -> Poll<(PromiseId, OpId, OpResult)> {
  let ready = std::task::ready!(self.results.borrow_mut().poll_next_unpin(cx));
  let PendingOp(PendingOpInfo(promise_id, op_id), resp) = ready;
  self.len.set(self.len.get() - 1);
  Poll::Ready((promise_id, op_id, resp))
}
```

Delete `poll_task`, `MaybeTask`, `task`/`task_set`, `completed_ops`,
`completed_waker`, and the shutdown dance for them. Op futures' wakers then wrap
the event-loop waker directly (registered via `cx` in `poll_ready`), so an I/O
event wakes the event loop in one hop.

**Expected impact.** Removes one spurious task poll + wake per completion batch
and per-op `VecDeque`/`RefCell` traffic. Improves latency (one fewer scheduling
round between I/O readiness and JS resolution) and CPU per request. This is on
the critical path of every async op in the system; expect mid-single-digit % on
op-bound servers, more on the async op microbench.

**Risks.** Reentrancy: `poll_ready` holds the `FuturesUnordered` borrow while
polling op futures. Op futures are pure Rust (they cannot call JS), but a future
that synchronously calls `driver.submit_op` during its own poll would now hit a
`RefCell` double-borrow. Note the _current_ code has the same constraint
(`poll_task` also holds the borrow while polling), so no new hazard — but verify
with `cargo test -p deno_core`. Also verify `stats()`/`shutdown()` interplay and
that `len()` accounting stays correct for the sanitizer.

---

## 3. Drop the `.catch` wrapper promise allocated for every pending async op

**Where:** `00_infra.js:145-172` (`setPromise`), `:174-195`
(`__resolvePromise`).

**Problem.** Every async op that doesn't complete eagerly allocates:

1. `new Promise(executor)` + executor closure + the `[resolve, reject, id]` ring
   entry array;
2. **a second, derived promise** via
   `PromisePrototypeCatch(promise,
   __opRejectHandler)` whose only purpose is
   to re-capture the stack trace on the (rare) rejection path;
3. a symbol-keyed property store on the wrapped promise.

The catch registration also enqueues promise-reaction machinery on every op, and
doubles the number of promise objects the GC has to sweep in op-heavy workloads.
The rejection-path stack fix runs `ErrorCaptureStackTrace` inside a microtask
with an empty JS stack — the identical result can be produced at reject time.

**Proposed change.** Return the raw promise from `setPromise` (keep the
`promiseIdSymbol` store on it) and move the stack recapture into
`__resolvePromise`'s reject path:

```js
function __resolvePromise(promiseId, res, isOk) {
  ...
  if (isOk) {
    promise[0](res);
  } else {
    // recreate the stacktrace and strip internal event loop frames
    if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, res)) {
      ErrorCaptureStackTrace(res, __resolvePromise);
    }
    promise[1](res);
  }
}
```

(The current handler also runs `ErrorCaptureStackTrace` unconditionally on
whatever the rejection value is; keep exact semantics — non-Error rejection
values throw in `ErrorCaptureStackTrace`? No: it defines `.stack` on any object
— mirror the existing behavior precisely, including for primitives where the
current code would throw; check the existing tests for `op_async_throw` cases.)

**Expected impact.** One fewer promise allocation + one fewer reaction
registration per pending async op — on the async op microbench this is a large
fraction of the per-op JS-side cost; expect ~10%+ there, and reduced GC pressure
on servers (2 promises → 1 per in-flight op).

**Risks.** Stack-trace shape of rejected op promises (there are `.out` tests
asserting error stacks — do not modify them; the strip target changes from
`__opRejectHandler` to `__resolvePromise`, which is equivalent because both run
with only internal frames on the stack). Unhandled-rejection semantics are
unchanged: the rejection now surfaces on the single promise directly instead of
on the derived one.

---

## 4. Shrink the `__eventLoopTick` argument protocol (sign-encode `isOk`, drop `v8::Boolean`/`v8::Integer` creation)

**Where:** `jsruntime.rs:3496-3564` (`dispatch_event_loop_tick`),
`01_core.js:458-465` (`__eventLoopTick`).

**Problem.** Each completed op pushes a `(promiseId, isOk, res)` triplet as V8
call arguments: one `v8::Integer::new` FFI, one `v8::Boolean::new` FFI, plus a
wider `arguments` object for the JS side to walk.

**Proposed change (simple).** Promise ids are non-negative i32s
(`00_infra.js:33-34`). Encode failure in the sign: push `promiseId` on success
and `~promiseId` (bitwise not, handles 0) on error — two args per op instead of
three, no `Boolean` creation:

```js
function __eventLoopTick() {
  for (let i = 0; i < arguments.length; i += 2) {
    let id = arguments[i];
    const isOk = id >= 0;
    if (!isOk) id = ~id;
    __resolvePromise(id, arguments[i + 1], isOk);
  }
}
```

**Proposed change (fuller, optional).** Move ids out of call args entirely: Rust
writes `(±promiseId)` into a shared growable `Int32Array` (same pattern as
`tick_info`/`immediate_info`, `jsrealm.rs:123-132`) with a count in slot 0, and
passes only the `res` values as call arguments. That removes _all_ per-op
`Integer::new` FFI calls; JS pairs `buffer[i+1]` with `arguments[i]`.

**Expected impact.** Cuts per-op boundary values from 3 to 2 (or 1), and per-op
FFI handle creation from 2 to 1 (or 0). Micro-level win on op-heavy ticks;
combined with #1 this substantially thins `dispatch_event_loop_tick`. Low
effort, low risk — good to batch with #1.

**Risks.** None significant; the protocol is fully internal
(`js_event_loop_tick_cb` is set up in `store_js_callbacks`). Keep
`MAX_VEC_SIZE_FOR_OPS` semantics (re-wake when the batch cap is hit).

---

## 5. Stop busy-spinning the event loop while V8 background tasks are pending

**Where:** `jsruntime.rs:2662-2669` (re-wake logic), `jsruntime.rs:3055`
(`has_pending_background_tasks`), `setup.rs:79-118`.

**Problem.** When the only pending state is
`scope.has_pending_background_tasks()` (V8 concurrent GC marking/sweeping, Wasm
background compilation), the loop does `self.inner.state.waker.wake()` and
returns `Pending` — i.e. it _immediately reschedules itself_. Until the
background work drains, the main thread spins through full event-loop ticks
(scopes, checkpoints, pending-state collection) as fast as tokio can poll,
burning a core alongside the V8 worker threads. This is exactly "high CPU while
idle" during/after GC or during Wasm compile of an otherwise-idle process.

**Key observation.** The custom platform already wakes the event loop when
background work needs the main thread: every foreground task posted by V8
(including completion callbacks for Wasm async compile and GC finalization
steps) goes through `queue_task`/`spawn_delayed_task`, which push to the shared
queue and call `entry.waker.wake()` (`setup.rs:83-84`, `:115-116`). The
busy-wake is a belt-and-braces poll, not the actual wake mechanism.

**Proposed change.** When `has_pending_background_tasks` is the _only_ reason to
re-wake, don't self-wake immediately. Instead arm a cheap fallback timer (e.g.
reuse the tokio handle to `sleep(1ms)` and wake, or a capped exponential backoff
100µs → 5ms) so the loop still re-checks even if some V8 background task class
never posts a foreground follow-up. All other re-wake reasons
(`has_tick_scheduled`, immediates, promise events, `uv_did_io`) keep the
immediate wake.

**Expected impact.** Converts a hot spin (thousands of full ticks per GC cycle)
into a handful of polls. Large reduction in main-thread CPU during concurrent GC
and Wasm compilation; directly visible in "idle process shows X% CPU" reports
and in throughput benchmarks (the spin competes with GC threads for cores
exactly when memory pressure is highest).

**Risks.** A background task class whose completion requires main-thread polling
without posting a foreground task would see up to the backoff cap of added
latency — the fallback timer bounds this. Test: Wasm streaming-compile tests
(`tests/misc.rs test_pump_message_loop`), GC-heavy tests, and `--expose-gc`
stress runs.

---

## 6. Trim the fixed per-tick and per-op bookkeeping

**Where:** `jsruntime.rs:2433-2440`, `jsruntime.rs:3533-3536`, `stats.rs:63-69`,
`tasks.rs` (as prior art).

Several small unconditional costs run on every tick or every op completion; each
is individually minor but they are all on the hottest loop in the runtime and
are trivially gateable:

1. **`foreground_tasks` mutex** (`jsruntime.rs:2437`): the tick unconditionally
   locks a `std::sync::Mutex` and `mem::take`s a (almost always empty) `Vec`.
   Add an `AtomicBool has_tasks` fast flag exactly like `V8TaskSpawnerFactory`
   already does (`tasks.rs:33`, `:67-74`): check with `Acquire` load, only lock
   when set.
2. **`activity_traces.complete()` per op** (`jsruntime.rs:3534-3536`,
   `stats.rs:63-69`): unconditional `RefCell::borrow_mut` + `BTreeMap::remove`
   per completed op even though leak tracing is almost always disabled. Gate on
   `activity_traces.is_enabled()` (a `Cell<bool>` read).
3. **`unrefed_ops.borrow_mut().remove()` per op** (`jsruntime.rs:3533`): skip
   when the set is empty (`borrow().is_empty()` first — reads a len); ops are
   rarely unrefed outside of `Deno.serve` accept loops.
4. **`EventLoopPendingState::new`** (`jsruntime.rs:3010-3063`): collects ~12
   booleans through `RefCell` borrows and one FFI call on every tick, including
   ticks that did nothing. After #1's `ran_js_since_checkpoint` flag exists,
   most fields can be skipped on no-work ticks by short-circuiting: check the
   cheap `Cell`/atomic fields first and only fall through to the borrow-heavy
   ones when needed (`is_pending()` is a disjunction — evaluate lazily instead
   of materializing the whole struct).

**Expected impact.** Individually sub-1%; together they shave a meaningful slice
off the fixed tick cost, which multiplies with #1. Zero-risk, purely mechanical
changes — good first PR to establish the benchmark baseline.

---

## 7. Make per-timer op calls conditional (`op_timer_track` / `op_timer_untrack`)

**Where:** `02_timers.js:339-372` (`createTimer`), `:270-335` (`listOnTimeout`),
`ops_builtin.rs`/`ops_builtin_v8.rs` (`op_timer_track`, `op_timer_untrack`),
`jsrealm.rs:133-137` (`active_timers`), `jsruntime.rs:3042`
(`has_pending_timers`).

**Problem.** Every `setTimeout`/`setInterval` creation crosses the boundary 2-3
times: `op_timer_now()` + `op_timer_track(id, repeat, system)` (+
`op_timer_schedule` when the earliest expiry changes), and every completion or
cancellation crosses again for `op_timer_untrack`. The `active_timers` map these
ops maintain is consumed only by (a) the test-sanitizer / `RuntimeActivityStats`
and (b) `has_pending_timers`, which is used solely in the TLA-stall retry
heuristics (`jsruntime.rs:2687`, `:2745`) — not for event-loop liveness (that's
`timer_info[0]` + `user_timer.is_refed()`).

Timer-heavy workloads are common (per-connection timeouts in Node-compat servers
create and cancel a timer per request).

**Proposed change.**

1. Maintain a plain _count_ of live timers in the existing shared `timer_info`
   buffer (widen it by one slot; JS increments on create, decrements on
   fire/cancel — zero boundary cost). Use that count for `has_pending_timers`.
2. Only call `op_timer_track`/`op_timer_untrack` when tracking is actually on:
   gate in JS behind a flag set by the sanitizer/leak-tracing activation (the
   same pattern as `__isLeakTracingEnabled()` which already gates the stack
   capture at `02_timers.js:366`). `deno test` turns it on; `deno run` never
   pays it.
3. `op_timer_now()` in `insert()` can reuse the `now` already passed in when
   called from the repeat path (already done) — additionally consider passing
   `now` from `processTimers` into fresh `createTimer` calls made inside timer
   callbacks via a scoped "current now" to skip another op call; minor.

**Expected impact.** Removes 2 op crossings per timer lifecycle in `deno
run`.
For request-timeout-per-connection servers this is 2 crossings per request. A
few % on such workloads; nothing on timer-free code.

**Risks.** The sanitizer must see timers created _before_ it enables tracking:
on enable, JS can replay the currently-live timer table (it has the linked
lists) into `op_timer_track` — or the sanitizer flag can simply be set at
startup under `deno test`, which is how leak tracing behaves today. Verify
`deno test` timer-leak sanitizer specs.

---

## 8. Fast-call path for _eager_ async ops (design-level, highest ceiling)

**Where:** `libs/ops/op2/dispatch_fast.rs:440-446` (fast dispatch bails for
eager async), `dispatch_async.rs:117-138`, `00_infra.js:210-442`
(`setUpAsyncStub`).

**Problem.** The default (eager) async op — which includes the hottest ops in
the system: `op_read`, `op_write`, HTTP ops — can never use the V8 fast-call
path. Every invocation goes through the slow path: `FunctionCallbackInfo`
unpacking, `CallbackScope` creation, `Local<Value>` arg checks, plus the JS stub
logic. Yet the _common_ outcome for many of these (socket has buffered data,
write fits the kernel buffer) is eager completion with a small numeric result:
the future resolves on the first poll and the value is returned synchronously
(`submit_op_* eager poll`, `futures_unordered_driver.rs:152-166`).

Only `lazy`/`deferred` async ops get fast calls today because they always return
"pending" and need no eager result channel.

**Proposed change (sketch).** Give eager async ops a fast-call overload with an
out-of-band eager-result channel:

- Fast fn signature returns the numeric result type (e.g. `i32`/`u32`) and takes
  `FastApiCallbackOptions` (already supported for opctx access).
- On eager completion: write a "completed" tag into a per-`OpCtx` shared status
  slot (a `Cell<u8>` exposed to JS via a shared `Uint8Array`, same pattern as
  `tick_info`) and return the value directly.
- On pending: write "pending" into the slot, return 0.
- The JS async stub reads the status byte to decide between
  `PromiseResolve(value)` and `setPromise(id)`. The status read is a typed array
  load — no boundary crossing.
- Errors: fall back through the existing native-throw path
  (`generate_fast_result_early_exit` already handles Result shedding in fast
  calls without re-entering JS, `dispatch_fast.rs:387-413`).

Restrict the first iteration to ops whose success type maps to a fast-call
return type (`void`, `bool`, i32/u32/f64) and whose args are already
fast-compatible — that covers `op_read`/`op_write`/`op_write_all` and most of
`ext/http`'s per-request ops.

**Expected impact.** This is the largest single lever for op-bound servers: it
moves the most-frequent boundary crossing in the system from the V8 slow-call
ABI to the fast-call ABI (no `FunctionCallbackInfo`, no handle scope in the
common case, JIT-inlined argument marshaling). Prior art from when sync ops
gained fast calls suggests 2-5x on the crossing itself; end-to-end, expect >10%
on I/O-heavy benchmarks.

**Risks.** Highest-effort item; the fast-call contract forbids JS re-entry and
allocation-triggering V8 API use — the eager poll runs arbitrary op future code,
which today may touch scopes via `rv_map` only on the _resolution_ path (safe:
mapping happens later in `__eventLoopTick` for the pending case; for the eager
case the numeric result needs no scope). `promise_id` is already available as a
fast arg (`FastArg::PromiseId`, `dispatch_fast.rs:179-182`). Needs careful
review of the `deno_core` op metrics variants and the reentrancy checker.
Prototype on `op_void_async` + `op_read` first and measure with
`benches/ops/async.rs`.

---

## 9. ASCII fast path for string return values

**Where:** `runtime/ops_rust_to_v8.rs:323-331`.

**Problem.** `String`/`&str`/`Cow<str>` returns go through `v8::String::new` →
`NewFromUtf8`, which runs V8's UTF-8 scan/decode even when the payload is pure
ASCII (the overwhelmingly common case for op results: paths, headers, JSON).
V8's one-byte constructor (`new_from_one_byte`) skips the decode entirely;
Rust-side `is_ascii()` is SIMD-vectorized and much cheaper than V8's UTF-8 walk.

**Proposed change.** In the `to_v8_fallible!` impl for strings:

```rust
if value.is_ascii() {
  v8::String::new_from_one_byte(scope, value.as_bytes(), NewStringType::Normal)
} else {
  v8::String::new(scope, &value)
}
```

(ASCII ⊂ Latin-1, so the bytes are valid one-byte string content as-is.) This
mirrors the argument-direction optimization already done in `to_str_ptr`
(`runtime/ops.rs:169-203`), which comments that the ASCII check beats the copy —
the same trade holds in the return direction.

**Expected impact.** Sizeable on string-returning ops (`op_encode`-adjacent
paths, path/URL ops, header handling). Measure with a sync-op string bench; low
effort, low risk. Note: consider the same split in `serde_v8`'s string
serialization for `#[serde]` ops.

**Risks.** None semantic — both constructors produce identical JS strings for
ASCII input. Just make sure the length-limit error path stays identical.

---

## Suggested implementation order

1. **#6** (mechanical trims) + benchmark baseline setup
2. **#4** then **#1** (tick protocol + consolidation — same code region, biggest
   tick-path win)
3. **#3** (promise wrapper removal — isolated to `00_infra.js`)
4. **#2** (driver de-indirection — isolated to `op_driver/`)
5. **#5** (background-task spin — needs GC/Wasm testing)
6. **#7** (timer tracking gate)
7. **#9** (string retvals)
8. **#8** (eager-async fast calls — prototype last, largest payoff)

Items 1-4 compose: after all four, a one-op tick should be roughly _one_ Rust→JS
call, _one_ microtask checkpoint, and _one_ scope setup — versus today's 2 JS
calls, 1 op call, 4 checkpoints, and 4 TryCatch scopes.
