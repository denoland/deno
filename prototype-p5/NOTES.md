# P5 — async scheduling and context propagation cannot desynchronise the stamp

Prototype for [wayfinder P5](https://github.com/bartlomieju/wayfinder/issues/34).
Throwaway code. Builds on [P1](https://github.com/bartlomieju/wayfinder/issues/19)'s
`runtime/permcap_proto.rs` unchanged — this cut adds attack surface, not mechanism.

## How to run

```
CARGO_TARGET_DIR=<warm-target> cargo build --bin deno --features hmr
cp <target>/debug/deno ./deno-permcap
cd prototype-p5/app

PERMCAP=1 ../../deno-permcap run -A --v8-flags=--expose-gc a1_propagation.mjs
PERMCAP=1 PERMCAP_DENY=npm:p5-denied ../../deno-permcap run -A --v8-flags=--expose-gc a2_bypass.mjs
PERMCAP=1 PERMCAP_DENY=npm:p5-denied ../../deno-permcap run -A a3_context_steal.mjs
```

`--features hmr` is still required for P1's reason (finding 5 there): the
interceptor installs via `global_template_middleware`, which `create_context`
only consults in the no-snapshot branch. That is [P3](https://github.com/bartlomieju/wayfinder/issues/28)'s
problem, untouched here.

## The question

P1 proved `GetCurrentHostDefinedOptions()` follows the **running script** across
`await`, microtasks, timers, op round-trips and workers. It did not go near the
primitives whose entire purpose is to **detach a continuation from the code that
scheduled it and re-enter it later**.

That matters because Deno has *two* propagation mechanisms in the same engine:

- **host-defined options** ride with the script — this is what the stamp is;
- **`ContinuationPreservedEmbedderData`** rides with the continuation — this is
  what `getAsyncContext`/`setAsyncContext` in `libs/core/01_core.js` move
  around, and what `ext/web/02_timers.js:58-70` saves and restores around every
  timer callback, and what `bindings.rs` captures at a dynamic `import()` call
  site for the module map to restore when it settles.

P1 never tested where those two meet. If any primitive restores a saved
continuation and the stamp follows it, a package executes under someone else's
identity. If any primitive runs a callback with **no script on the stack**, the
stamp comes back empty — and D1 reads an absent stamp as first-party, i.e.
*unconfined*, which is exactly how P1's `data:` worker escaped.

## Result: no bypass, on any of 30 probed contexts

**A1** (nothing denied) — correct attribution to `npm:p5-denied` in **every**
context, 0 unattributed, 0 mis-attributed:

module top level and sync baseline · `setTimeout` · `setInterval` ·
`queueMicrotask` · `Promise.then` · `setImmediate` · `process.nextTick` ·
`AsyncLocalStorage.run` · `run` + nested `setTimeout` · `enterWith` ·
`MessageChannel.onmessage` · `EventTarget` listener · `AbortSignal` abort ·
`ReadableStream` pull · `Atomics.waitAsync` resolution · `FinalizationRegistry`
callback · `Error.prepareStackTrace` · `structuredClone` getter · `Proxy` trap
invoked by `ext:` · dynamic `import(data:)` settle.

**A2** (same probes, `PERMCAP_DENY=npm:p5-denied`) — **0 bypasses**, 0
unattributed. Every context read `fetch` as `undefined`.

**A3** (denied package re-entering a *granted* package's async context, with a
control row proving the granted package holds `fetch`) — **0 bypasses**:

| attempt | stamp | fetch |
|---|---|---|
| granted pkg (control) | `npm:p5-granted` | **function** |
| `AsyncResource.runInAsyncScope`, resource built by granted pkg | `npm:p5-denied` | undefined |
| same, resource built deep in granted pkg's stack | `npm:p5-denied` | undefined |
| `AsyncLocalStorage.bind` into granted pkg's context | `npm:p5-denied` | undefined |
| `AsyncLocalStorage.snapshot()` restored by denied pkg | `npm:p5-denied` | undefined |
| denied callback invoked from deep in granted pkg's stack | `npm:p5-denied` | undefined |
| `AsyncResource` built inside granted stack, re-entered later | `npm:p5-denied` | undefined |
| `async_hooks` `init` lifecycle callback | `npm:p5-denied` | undefined |

## Why it holds, stated as a claim the spec can make

**CPED and host-defined options are independent slots, and only the latter is
the stamp.** Every context-propagation primitive Deno exposes — `AsyncResource`,
`AsyncLocalStorage.run`/`enterWith`/`bind`/`snapshot`, `async_hooks` lifecycle
hooks, the timer bracket, the dynamic-import bracket — moves **CPED**. None of
them touches host-defined options, because host-defined options are a property
of the compiled script, fixed at compile time, and there is no API to swap them
on a live frame.

So re-entering a context restores *data*, never *identity*. A denied package can
faithfully restore a granted package's entire async context and still be denied,
because the code doing the reading is still its own script.

**Two predictions in the ticket were wrong, and it is worth recording which.**
The ticket flagged `FinalizationRegistry`, `Atomics.waitAsync` and
`Error.prepareStackTrace` as the likely escapes — callbacks invoked "from the
engine, not from a call site", where the stamp might come back empty. It does
not. V8 reports the host-defined options of the script that *defined* the
callback function, and that is the denied package in all three cases. The
"no script on the stack" hazard is narrower than assumed: it needs code that was
**separately compiled**, which is P1's finding 5 (`data:` modules, workers), not
merely code that was **asynchronously invoked**.

## What this does not answer

- **TC39 `AsyncContext` is not implemented in Deno.** No global, no flag
  (`grep` over `cli/args/flags.rs` finds nothing); only the internal
  `getAsyncContext`/`setAsyncContext` core ops exist. `AsyncContext.Snapshot.wrap`
  is the sharpest instrument on the ticket's list and it could not be tested.
  **This must be re-run when the proposal lands**, and the spec should say the
  claim is contingent on it, since `wrap()` is precisely a capture-and-re-enter
  primitive exposed directly to user code.
- **`process.nextTick` under denial reported `<never fired>`** — correct
  behaviour, not a gap: `process` is itself a guarded global, so the *scheduling
  call* is denied before a callback exists. A1 confirms it attributes correctly
  when not denied. Worth noting because it is the first observed case of a
  denial changing control flow rather than a value.
- **Channel 2 is still unbuilt** (as in P1), and enforcement here is still
  Channel 1 global-accessor denial, not D2's op-layer `PermissionsContainer`
  with the `OpState` current-package cell. This cut tests *attribution under
  async re-entry*, which is what the bypass question turns on; it does not test
  the op-layer bracket D2 specified.
- **Workers crossed with context propagation** were not probed. P1 covered
  worker top level; a worker plus ALS plus a denied parent is untested.
