# P1 — attacking D3's mechanism on a real dependency graph

Throwaway prototype for [wayfinder P1](https://github.com/bartlomieju/wayfinder/issues/19).
Delete this directory and the branch once the findings are folded into the spec.

**Question.** Does D3's mechanism — caller-attributed global accessors reading
`Isolate::GetCurrentHostDefinedOptions()` — survive contact with a real npm
dependency graph? This cut falsifies **Channel 1 only** (global accessors).
Channel 2 (resolver-mediated imports) is deliberately not built: everything
rests on whether attribution is reliable, so that is what gets attacked.

**Verdict.** Attribution is reliable — far more so than D3 dared claim. The
mechanism does not die here. But three things D3 asserts are wrong, and one of
them is a hole a no-grant package can walk through today.

## How to run

```
cd prototype-p1
CARGO_TARGET_DIR=<warm-target> cargo build --bin deno --features hmr   # from repo root
cp <target>/debug/deno ./deno-permcap
cd app
PERMCAP=1 ../deno-permcap run -A --unstable-worker-options e1_attribution.mjs   # attribution matrix
PERMCAP=1 ../deno-permcap run -A e2_forgery.mjs                                # CJS stamp forgery
PERMCAP=1 ../deno-permcap run -A e2b_no_import.mjs                             # forgery without any import
PERMCAP=1 PERMCAP_DENY=npm:express ../deno-permcap run -A e3_denial.mjs        # denial on express
PERMCAP=1 [PERMCAP_NOOP=1] ../deno-permcap run -A e4_cost.mjs                  # cost
PERMCAP=1 PERMCAP_DENY=npm:probe-esm ../deno-permcap run -A --unstable-worker-options e5_escape.mjs
```

`--features hmr` disables the startup snapshot — required, see finding 5. The
app is a real graph: express 4.22.2, lodash, chalk and their transitive trees,
plus four hand-written probe packages under `node_modules/`.

## What the patch does

- `runtime/permcap_proto.rs` — named property handler on the global object
  template, installed via deno_core's (previously unused)
  `global_template_middleware` hook. On a guarded read it attributes the caller
  via `GetCurrentHostDefinedOptions()` and returns `undefined` for a denied
  package. `globalThis.__permcapWhoami` is a probe returning the attribution.
- `cli/module_loader.rs` — stamps the package id into the host-defined-options
  `PrimitiveArray` at index 2 (0 and 1 are taken by `ext/node` and `node:vm`).
- `ext/node/polyfills/01_require.js` — same stamp on the CJS compile path.

Package identity is scraped from paths. D1 settled that real identity must be
resolver-derived; the heuristic stands in so the *attribution* path can be
exercised, and it is not part of what is being claimed here.

## Findings

### 1. Attribution holds everywhere the mechanism needs it to — CONFIRMED

`GetCurrentHostDefinedOptions()` returned the correct package at accessor time
in every context D3 listed as a risk, and several it didn't:

| context | attributed to |
|---|---|
| ESM package, module top level | the package |
| sync call from app into package | the package |
| after `await` / `Promise.then` / `queueMicrotask` | the package |
| inside a `setTimeout` callback | the package |
| after a real op round-trip (`Deno.readTextFile`) | the package |
| getter defined in package, invoked by `ext:` JS (`Headers`, `URL`) | the package |
| getter invoked by a V8 builtin (`JSON.stringify`) | the package |
| app callback invoked from deep inside lodash's stack | the app |
| CJS package, top level / after await / in a timer / required child | the package |
| worker top level, package code | the package |

The last two rows matter most. Attribution follows the **running script**, not
the caller, so no stack walk is needed and the confused-deputy direction that
killed `--deny-module` does not reappear: a granted package calling a no-grant
package's callback does not lend it anything, and a no-grant package calling a
granted one does not steal anything.

**Correction to D3.** D3 says generated code "carries no host-defined options,
so it is unattributable, and D1's fail-closed rule denies it." Measured: direct
`eval`, indirect `eval`, and `new Function` all **inherit the calling script's
stamp** — they were attributed to the package that called them. This is better
than D3 assumed (legitimate `eval` users are not denied, and eval is not an
escape) but the spec's stated reasoning is wrong and must be rewritten. Only
separately-compiled code — a `data:` module, a worker — comes through unstamped.

### 2. The CJS stamp is forgeable by any package, with no import — REFUTED

D3's open worry was that CJS modules might carry no stamp at all. They do; the
plumbing already exists (`core.compileFunction`'s third argument). The real
problem is the opposite: **the stamp is attacker-controlled**.

A no-grant package patches the CJS compile machinery — as pirates, ts-node,
babel-register and require-in-the-middle all do — so that its own source is
compiled under a *granted* package's filename. The injected code then reads the
victim's identity and the victim's authority:

```
attacker's own attribution: npm:probe-attacker
--- variant A: Module.wrap            (flips deno's internal `patched` flag)
  injected code attributed: npm:probe-victim     injected code saw fetch: function
--- variant B: Module.prototype._compile         (require-in-the-middle shape)
  injected code attributed: npm:probe-victim     injected code saw fetch: function
```

**Channel 2 cannot close this.** The obvious fix is to treat `node:module` as
capability-bearing and deny the import. But `module` is a *parameter of every
CJS wrapper function*, so `module.constructor` is `Module` — reachable with no
import at all. `e2b_no_import.mjs` runs the same attack that way and succeeds.

Closing it needs one of: freezing `Module.prototype._compile` / `Module.wrap` /
`Module.wrapper` before user code runs and denying the `patched` path entirely;
or attributing compiled code to the package that *called the compiler* rather
than to the filename it claims. Both are spec decisions, neither is in D3.

Note this is **not** the conceded conduit hole. Conduit laundering needs a
granted package to cooperate; this needs nothing from the victim and works
against packages that never hand out a reference.

### 3. Denial works, and the error is unusable — CONFIRMED (both halves)

Denying `npm:express` on the real graph produced:

```
TypeError: Cannot read properties of undefined (reading 'env')
    at app.defaultConfiguration (.../express/lib/application.js:78:21)
```

D3's denial shape is "absence is silent, use is loud". The second half only
holds when the missing capability is *called* into a gated op. `process` is
**dereferenced** (`process.env`), so the failure is a `TypeError` on `undefined`
deep inside a package, naming no package, no capability, and no permission
system. Every fallback-idiom benefit of silent absence is real, and so is this.

A trace of the whole express tree shows how narrow the guarded surface is —
5 packages in the resolved graph ever read a guarded global:

```
1069 read process from npm:mime          37 npm:depd     7 npm:debug
   3 read process from npm:iconv-lite     1 npm:express
```

Consistent with R2's minority-holds-authority finding. Also a cost signal:
`mime` reads `process` a thousand times during startup alone.

### 4. Cost is the mechanism's real problem — CONFIRMED, worse than expected

Debug build, 5M iterations, same binary throughout (with `PERMCAP` unset no
interceptor is installed at all, so this is a true off-baseline):

| read | off | interceptor, callback returns immediately | full callback |
|---|---|---|---|
| `globalThis.fetch` (guarded) | 1.01 ns | 163.87 ns | 921.62 ns |
| `globalThis.Math` (**unguarded**) | 0.60 ns | 163.67 ns | 635.48 ns |
| bare `Math` (free name) | 0.55 ns | 82.70 ns | 303.45 ns |
| absent property | 4.38 ns | 183.11 ns | 671.20 ns |
| local variable baseline | 1.04 ns | 1.09 ns | 1.11 ns |

The middle column is the finding: a **no-op** interceptor still costs ~150-270×
on every named global read. Installing a named property handler on the global
object makes V8 abandon its global-load inline caches, so the tax lands on
`Math`, `Object`, `console` — every global any code reads — not just on the
handful of authority-bearing names. The unchanged local-variable baseline
confirms it is global-access-specific and not build noise.

Caveats, stated plainly: this is an **unoptimized debug build**, and the
right-hand column includes a deliberately naive callback (a Rust string
conversion per access). A release build with cached `v8::Global<v8::String>`
comparisons would shrink both columns substantially. What it cannot shrink is
the IC loss, which is structural. **A release-build measurement is the obvious
follow-up and this prototype does not provide it.**

### 5. The mechanism cannot be installed under a snapshot — NEW CONSTRAINT

`create_context` (`libs/core/runtime/jsruntime.rs:2813`) only builds an
`ObjectTemplate` and runs `global_template_middleware` when there is **no**
snapshot; with one it calls `Context::from_snapshot` and the middlewares are
never consulted. The CLI always boots from the startup snapshot, so this
prototype only runs under `--features hmr`.

A shipped implementation must either install the interceptor into the
snapshotted context at snapshot-build time (which drags the callback into the
external-references table), or abandon the whole-object interceptor for
per-property accessors installed via `global_object_middleware` on the live
global — a different mechanism with a different (probably much better) cost
profile, since it would not deoptimize unrelated global reads.

### 6. "Unattributed" is two different things, and a package escapes through it

First-party app code and separately-compiled generated code are **both**
unstamped, and the interceptor cannot tell them apart. D1 says unattributable
code is denied and first-party code is unconfined — those are contradictory
rules over the same observation.

A denied package walks straight out through the gap:

```
package's own view:  {"whoami":"npm:probe-esm","fetch":"undefined","Worker":"function"}
via spawned worker:  {"whoami":"<unattributed>","fetch":"function"}
app (first-party):   {"whoami":"<unattributed>","fetch":"function"}
```

`Worker` is not authority-bearing by itself, which is exactly why it wasn't on
the guarded list — but it manufactures an execution context that is. The fix is
two spec changes: first-party code must be **positively stamped** rather than
identified by absence of a stamp, and context-spawning (`Worker`, `node:vm`,
`blob:`/`data:` modules) must be part of the capability enumeration.

## What this prototype does not answer

- Object identity, `instanceof`, and singletons — untouched by construction,
  since this mechanism adds no realm. Nothing observed suggested otherwise, but
  nothing tested it either.
- Native addons and FFI.
- Release-build cost (see finding 4).
- Channel 2 in any form.
