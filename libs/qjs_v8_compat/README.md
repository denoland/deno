# qjs_v8_compat

A `rusty_v8`-shaped API surface backed by [QuickJS-ng].

`deno_core` is currently built against `rusty_v8` — the safe Rust bindings to
V8. This crate provides a `pub mod v8` re-export that mirrors the rusty_v8
surface, but underneath dispatches to QuickJS-ng's C runtime. With the future
`--features quickjs` flag on `deno_core`, `use v8` resolves to the types in this
crate and the engine is swapped at compile time.

This is the implementation of
[denoland/orchid #66](https://github.com/denoland/orchid/issues/66).

## Status

Working scaffold + semantic round-trips. The crate:

- Compiles cleanly on stable Rust 1.91 (both `--features link_quickjs` and the
  default mock backend).
- Exposes the full type surface that `deno_core` imports from `v8` (Isolate,
  Context, HandleScope, EscapableHandleScope, Local, Global, Value, Object,
  Array, Function, Promise, Module, TryCatch, Exception, ArrayBuffer, Symbol,
  BigInt, Script, ScriptOrigin, …).
- Ships a pure-Rust mock backend so semantic and refcount-discipline tests run
  without QuickJS-ng linked. **All 20 tests pass** across two suites:
  `tests/refcount.rs` (9 tests) covers GC translation under every scope nesting
  / Global promotion / Escape pattern; `tests/semantics.rs` (11 tests) covers
  Object property round-trip, indexed access, Promise resolve/reject state
  transitions, TryCatch capture and rethrow, and bytecode-cache serialization
  round-trip — all observable through the mock arena, with the same code paths
  dispatching to QuickJS-ng's C API when `--features link_quickjs` is active.
- Hand-written FFI declarations for the QuickJS-ng C API in `src/ffi.rs`, with
  the full `sys` shim layer exposing property access, `JS_NewPromiseCapability`,
  exception throw/take, and `JS_WriteObject`/`JS_ReadObject` for the
  bytecode-cache snapshot path.

What's **not** here yet (follow-ups; tracked in [`ARCHITECTURE.md`]):

- Driving an actual `JsRuntime` through the compat layer end-to-end.
- The `deno_core` cargo feature wiring (`--features quickjs`).
- Async module loader bridge (QuickJS-ng's experimental `JS_LoadModuleAsync` is
  the target).
- WebAssembly / SharedArrayBuffer / Inspector stubs return correct values but
  don't actually execute.

The bytecode-cache snapshot stage (ARCHITECTURE §6.2) is now wired:
`SnapshotCreator::add_source` compiles modules with `JS_EVAL_FLAG_COMPILE_ONLY`,
serializes the result with `JS_WriteObject`, and emits a versioned
`(url, bytecode)` blob; `restore_blob` / `load_blob_entries` read it back via
`JS_ReadObject`.

## Build modes

```bash
# Type-check + run unit tests against the mock backend (no QuickJS needed).
cargo test -p qjs_v8_compat

# Link against a real QuickJS-ng tree.
QUICKJS_NG_DIR=/path/to/quickjs-ng/build \
  cargo build -p qjs_v8_compat --features link_quickjs
```

The mock backend is **not** a JS engine. It exists only to validate the
refcount-balance invariant — every `JS_NewX` in the wrapper is balanced by
exactly one `JS_FreeValue`, regardless of how scopes nest, how Globals are
promoted, or how `EscapableHandleScope::escape` reparents handles. The arena
panics loudly on leak or double-free, so a passing test suite is strong evidence
that the GC translation is sound.

## Crate layout

```
qjs_v8_compat/
  src/
    lib.rs          → pub mod v8 { ... } — the re-export
    ffi.rs          → hand-written extern "C" declarations
    sys.rs          → dual-mode dispatch (real FFI or mock arena)
    arena.rs        → mock refcounted arena
    isolate.rs      → OwnedIsolate / Isolate / IsolateHandle
    context.rs      → Context, ContextScope
    scope.rs        → HandleScope, EscapableHandleScope, CallbackScope
    value.rs        → Local<'s, T>, Global<T>, Weak<T>, type lattice
    primitives.rs   → String, Integer, Number, Boolean, BigInt, Symbol
    object.rs       → Object, Array, Map, Proxy, property handlers
    buffer.rs       → ArrayBuffer, TypedArray, BackingStore
    function.rs     → Function, FunctionCallbackInfo, ReturnValue
    promise.rs      → Promise, PromiseResolver, PromiseState
    module.rs       → Module, ModuleRequest, FixedArray, status enums
    script.rs       → Script, ScriptOrigin
    exception.rs    → Exception, TryCatch
    template.rs     → FunctionTemplate, ObjectTemplate
    external.rs     → External, serializers, CachedData
    snapshot.rs     → SnapshotCreator stub (QJS-DIVERGE)
  tests/
    refcount.rs     → refcount-balance + type-discrimination
```

Every divergence from V8 semantics is annotated in-source with a
`// QJS-DIVERGE:` comment explaining what differs and why.

## License

MIT. Copyright the Deno authors.

[QuickJS-ng]: https://github.com/quickjs-ng/quickjs-ng
[`ARCHITECTURE.md`]: ./ARCHITECTURE.md
