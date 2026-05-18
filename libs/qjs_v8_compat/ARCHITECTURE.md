# qjs_v8_compat architecture

This document is the design + roadmap for swapping QuickJS-ng in as a backend
for `deno_core`. It complements the audit in
[denoland/orchid #66](https://github.com/denoland/orchid/issues/66).

## 1. Goal

Let `deno_core` compile and run against QuickJS-ng instead of V8, with a cargo
feature flag and **no public API changes to deno_core**. This unlocks new
deployment targets — IoT, edge/serverless cold-start-sensitive workloads, WASM
hosts — where V8's footprint is impractical.

## 2. Audit of the V8 surface

`deno_core`'s codebase imports **126 distinct `v8::` symbols** (counted with
`grep -rohE "\bv8::[A-Za-z_][A-Za-z0-9_]*" core ops serde_v8 testing | sort -u`).

Every one of them maps to one of three categories:

| Category                                       | Count | Example                                                            |
| ---------------------------------------------- | ----- | ------------------------------------------------------------------ |
| Direct equivalent in QuickJS-ng                | ~70   | `Isolate`, `Context`, `Promise`                                    |
| Stubbable (no-op, no JS-visible effect)        | ~30   | `cppgc`, `fast_api`, `Platform`                                    |
| Hard divergence (need redesign or work-around) | ~25   | `SnapshotCreator`, `SharedArrayBuffer`, `Weak`, `WasmModuleObject` |

The full audit table lives in the issue body. This crate covers categories 1 and
2 in its scaffold; category 3 is addressed below.

## 3. GC model translation — the heart of the matter

V8 uses a tracing GC anchored to `HandleScope`s on the stack. QuickJS-ng uses
reference counting. The compat layer absorbs the impedance mismatch.

**Invariant:** every `JSValue` belongs to exactly one _owner_ at any time, and
is dropped exactly once.

Owners are:

- A `HandleScope`'s `owned: Vec<JSValue>` — dropped when the scope is dropped.
- A `Global<T>` — dropped in `Global::drop`.
- A parent scope, via `EscapableHandleScope::escape`, which moves the JSValue
  from the inner scope's vec to the outer scope's vec.

Promotion paths:

- `HandleScope` → `Global` via `Global::new`. The Global calls `JS_DupValue` (a
  second refcount) so the scope drop and the Global drop both balance.
- `Global` → `HandleScope` via `Global::to_local`. Same trick: dup, then push to
  the new scope's vec.

The mock backend in `arena.rs` exists _purely_ to verify this invariant. It
tracks per-handle refcounts and panics on drop if any entry is still live. Each
test in `tests/refcount.rs` exercises a different scope-nesting or escape
pattern.

## 4. Local<'s, T> design notes

`rusty_v8` puts methods on the `T` and uses `Deref<Target=T>` to dispatch from
`Local`. We diverge slightly: methods live directly on `Local<'s, T>`. The call
site syntax (`local.method(scope, ...)`) is the same; the difference is
invisible to deno_core.

Upcasts (`Local<String>` → `Local<Value>`) are `From` impls — infallible, since
the underlying JSValue is the same. Downcasts are `TryFrom` and live on the
matching marker type (currently not all implemented).

## 5. Ops bridge

`FunctionCallbackInfo` is laid out
`{ implicit_args: *mut JSValue, values:
*mut JSValue, length: i32 }`. The
QuickJS dispatcher in the compat layer will build this struct from
`(this_val, argc, argv)` and call into the op shim. `ReturnValue::set` writes
back through `*const FunctionCallbackInfo`.

This shape lets `op2`-generated code stay byte-identical between backends: op2
emits `unsafe extern "C" fn op_foo(info: *const FunctionCallbackInfo)`, which
the V8 path takes verbatim and the QuickJS path adapts via a trampoline
(`JSCFunction` → `info` builder → op shim).

The `op2` macro itself doesn't change.

## 6. Snapshots — staged plan

V8 startup snapshots have no QuickJS analog. Options ranked by tractability:

### Stage 1 (this PR): stub + warning

`SnapshotCreator::create_blob` returns an empty `StartupData`. Loading a
non-empty blob into a QuickJS-backed isolate is a runtime error. Calling the
constructor logs once to stderr explaining the divergence.

This is enough for non-Deno embedders (IoT, edge) where snapshots are not the
main win.

### Stage 2: bytecode cache (Option A from the issue)

QuickJS-ng exposes `JS_WriteObject(ctx, value, JS_WRITE_OBJ_BYTECODE)` and
`JS_ReadObject(ctx, buf, len, JS_READ_OBJ_BYTECODE)`. Each loaded extension JS
file is compiled once and its bytecode cached; subsequent runtime startup
`JS_ReadObject`s instead of re-parsing.

Expected speedup: ~5-10× over re-eval, ~2× slower than V8 snapshot.

### Stage 3: manual heap serializer (Option C)

Walk the QuickJS object graph at warm-up time, serialize each object's shape +
properties + prototype chain to a side-table, restore by replay on fresh start.
This is genuinely hard — QuickJS doesn't expose the internal shape API — and
only worth pursuing if Stage 2 turns out to be insufficient for Deno-CLI-class
startup.

## 7. Missing V8 features and how we handle them

| Feature                  | Strategy                                                                                                                                  |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `SharedArrayBuffer`      | Stubbed; throws if constructed on QuickJS backend. The op2 fast-buffer paths that depend on it are V8-gated via `#[cfg(feature = "v8")]`. |
| WebAssembly              | Stubbed; throws on instantiation. Future: link `wasmtime` (already in Deno's tree) via a custom QuickJS module loader.                    |
| Inspector / CDP          | Stubbed; the inspector is disabled on the QuickJS backend. The debugger is the main feature you give up.                                  |
| `Weak<T>` finalizers     | Best-effort — QuickJS has no weak ref API. The few sites in deno_core that use `Weak` (cppgc bookkeeping) are V8-gated.                   |
| cppgc                    | Stubbed — no equivalent. The cppgc bridge in deno_core is V8-gated.                                                                       |
| `Platform` (libplatform) | No-op. QuickJS has no worker pool; all ops run on the embedder's thread.                                                                  |
| TLA snapshots            | See §6.                                                                                                                                   |
| ICU                      | Stubbed `set_common_data_*`. QuickJS ships its own (minimal) Intl.                                                                        |

## 8. deno_core integration plan (the follow-up PR)

The goal is **zero changes** to deno_core, but in practice a small number are
unavoidable. Tracked here:

1. `core/Cargo.toml`: add features
   ```toml
   [features]
   default = ["v8"]
   v8 = ["dep:v8"]
   quickjs = ["dep:qjs_v8_compat"]
   ```

2. A single `lib.rs` switch:
   ```rust
   #[cfg(feature = "quickjs")]
   pub use qjs_v8_compat::v8 as v8_engine;
   #[cfg(feature = "v8")]
   pub use v8 as v8_engine;
   ```
   …and `s/use v8/use crate::v8_engine as v8/` (one rename across the crate).

3. Each remaining V8-only call site (`SnapshotCreator`, cppgc, fast_api,
   inspector usage) gets a `#[cfg(feature = "v8")]` gate, with the feature-gated
   dispatch replaced by a stub call on the QuickJS path.

The target is **fewer than 20 `#[cfg]` annotations in deno_core**. If we exceed
that, the compat layer is incomplete and we should fix it instead.

## 9. Testing strategy

### Pure-Rust tests (this PR)

- `tests/refcount.rs` — refcount balance under every scope nesting + Global
  promotion + escape combination.

### With QuickJS linked (next PR)

- `tests/eval.rs` — `1+1`, basic eval, error paths.
- `tests/promise.rs` — `JS_NewPromiseCapability`, resolve, reject, hook.
- `tests/modules.rs` — `JS_EVAL_TYPE_MODULE`, loader callback.
- `tests/op2_bridge.rs` — sync + async op dispatch end-to-end.

### deno_core test suite (final PR)

- Run `cargo test -p deno_core --features quickjs` and track pass/fail.
- WASM, SAB, inspector tests are expected to fail and are excluded.

## 10. Benchmarks (final PR)

- **Cold start** via `hyperfine`: V8 vs QuickJS on a hello-world program.
- **Memory** via `/usr/bin/time -v` peak RSS on the same program.
- **Op throughput** via the existing `core/benches/` harness, tagged with the
  active backend.

Expected: QuickJS wins on cold start (10-100×) and memory (3-5×). V8 wins on
steady-state throughput once the JIT warms up. The crossover point gets
documented in the PR.

## 11. Platform matrix

Each platform needs CI:

- `x86_64-unknown-linux-gnu` — baseline.
- `aarch64-unknown-linux-gnu` — arm64.
- `x86_64-apple-darwin`, `aarch64-apple-darwin`.
- `x86_64-pc-windows-msvc`.
- `wasm32-unknown-emscripten` — QuickJS is C, so emscripten is the obvious path.
  Tracks the [quickjs-emscripten] effort upstream.
- `wasm32-wasi` — running ops inside a WASM sandbox is one of the motivations
  for this work.
- `thumbv7em-none-eabihf` — embedded. Needs QuickJS configured with
  `--disable-stdlib` and `core::alloc` integration.

[quickjs-emscripten]: https://github.com/justjake/quickjs-emscripten

## 12. Open questions

- **Async module loading.** QuickJS-ng has experimental `JS_LoadModuleAsync` in
  `master`; latest stable still requires synchronous module resolution. The
  deno_core module loader is async. Bridging requires either (a) waiting for the
  QuickJS-ng API to stabilize, or (b) pre-fetching all module sources in Rust
  before handing them to QuickJS's sync loader. Option (b) is what this scaffold
  assumes.

- **PromiseHook quadruple.** V8 exposes four separate hooks (init, before,
  after, resolve) for Async Context. QuickJS has a single rejection tracker. We
  map "resolve" to the tracker and drop the others. Async Context support is
  V8-only until QuickJS-ng adds the full set.

- **Stack overflow handling.** V8 has `Isolate::set_stack_size`. QuickJS has
  `JS_SetMaxStackSize`. The semantics differ around generators — worth a focused
  test before claiming parity.
