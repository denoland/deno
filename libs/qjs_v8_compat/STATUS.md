# QuickJS Integration — Overnight Status

**Branch:** `orch/issue-66` (PR #34033). 36 commits this session.

## What works today, runnable

```sh
cargo test -p qjs_v8_compat --features link_quickjs
```

…runs **28 tests** (all passing) against real QuickJS-ng linked from
the vendored `vendor/quickjs-ng` submodule:

* `tests/real_engine.rs` — `console.log("hello, world")` with a
  Rust callback installed as a JS function; arithmetic eval; runtime
  lifecycle leak check; Promise + `queueMicrotask` drain.
* `tests/esm.rs` — ES-module loading via host loader: single import,
  function name resolution across modules, transitive chain
  `a.js → b.js → c.js`, async `import { later } from './async.js'`.
* `tests/http_server.rs` — A real Hyper HTTP/1.1 server bound to a
  random port; every request invokes a JS handler in QuickJS-ng;
  test sends real HTTP requests for `/hello`, `/echo`, `/sum/...`,
  fallback path. All assertions pass on the JS-computed bodies.

## Where deno_core stands

`cargo check -p deno_core --no-default-features --features quickjs,include_icu_data,reactor-tokio`

| stage          | error count |
|----------------|------------:|
| start of session |   1693 |
| current (head=4e526200b) |   16 |

The 16 remaining are all `lifetime may not live long enough` in
`serde_v8`'s deserializer access types (`MapObjectAccess`,
`MapPairsAccess`, `SeqAccess`, `StructAccess`). Root cause:
rusty_v8's `&'a mut PinScope<'s, 'i>` storage pattern leans on
specific subtyping that doesn't translate to `qjs_v8_compat::PinScope`
without one of:

* (a) serde_v8-side refactor that unsafe-extends the scope's
  borrow lifetime at each callsite (about 8 sites in `de.rs`), or
* (b) replacing `PinScope` with a `HandleScope` newtype using
  interior mutability so the scope is borrowed via `&` not `&mut`.

Both are tractable, but each needs a careful pass — last-mile
work that should land cleanly when not solo-debugging at 4 AM.

## Path to `Deno.serve` working

After the 16-error wall:

1. **Finish `deno_core --features quickjs`** — variance fix.
2. **`ext/http`, `ext/web`, `ext/net`, `ext/url` quickjs features** —
   each ext crate currently has `v8.workspace = true`; needs the
   same engine-selector cargo feature + `extern crate ... as v8`
   alias that `deno_core` and `serde_v8` got this session. Likely
   surfaces another batch of compat-surface gaps similar in shape
   to what we already closed (most heavy lifting is in qjs_v8_compat,
   so subsequent crates should converge faster).
3. **Build a Deno binary with `--features quickjs`** — `cli/main.rs`
   plus `cli/lib`, `runtime`, etc.
4. **Functional `Deno.serve(handler)`** — once the binary builds,
   the JS `Deno.serve` API is implemented in `ext/http`'s JS files
   on top of ops; if the ops work, Deno.serve works.

## Architectural decisions made overnight

* `extern crate qjs_v8_compat as v8` aliasing trick: lets every
  `use v8::*` site in `deno_core`/`serde_v8` resolve to the compat
  crate without source edits to those callsites.
* `serde_v8` got the same engine-selector cargo features as
  `deno_core` (`v8-engine` default, `quickjs` opt-in). Without this,
  `serde_v8` was binding rusty_v8 unconditionally and producing the
  bulk of the type-mismatch noise.
* QuickJS-ng vendored as a git submodule under
  `libs/qjs_v8_compat/vendor/quickjs-ng`; CI workflow updated
  (`.github/workflows/ci.ts`) to init it during checkout.
* `PinScope`, `PinCallbackScope` are now structs (not type aliases)
  so they accept the two-lifetime form rusty_v8 callers expect.
* All scope-creation macros mirrored: `scope!`, `tc_scope!`,
  `callback_scope!` (with `unsafe`/`let` variants), `isolate_scope!`,
  `scope_with_context!`, `escapable_handle_scope!`, `context_scope!`.

## How to resume

1. `cargo check -p deno_core --no-default-features --features quickjs,include_icu_data,reactor-tokio`
   — see the 16 lifetime errors.
2. Apply approach (a) or (b) above to silence them.
3. Add the `quickjs` feature to `ext/web/Cargo.toml` (smallest ext)
   and the corresponding `extern crate ... as v8` to its `lib.rs`.
   Use that crate as a microcosm before propagating to the rest.
