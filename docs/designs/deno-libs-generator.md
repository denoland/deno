# De-forking Deno's TypeScript libs: a generator for `lib.deno.*.d.ts`

Status: draft / RFC Context: Deno 3 "un-fork TypeScript" — making `deno check`
run stock tsgo, and publishing a Node-consumable `@types/deno`.

## Problem

Deno ships ~200 KB of forked web-platform lib declarations (`lib.deno_web.d.ts`
~100 KB, `lib.deno_fetch.d.ts`, `lib.deno_url.d.ts`,
`lib.deno.shared_globals.d.ts`, ...). Each fully _redefines_ types that stock
TypeScript already ships in `lib.dom.d.ts` / `lib.webworker.d.ts` /
`lib.esnext.*`. `lib.deno_web.d.ts` alone has 176 declarations, almost all of
them shadowing a stock type.

This fork exists for a good reason (see "Why Deno forked" below) but it has two
costs:

1. The declarations do **not co-load cleanly with `dom`**. When a project uses
   both Deno's libs and `lib.dom`, duplicate `interface`/`declare var`
   declarations collide, which is why the generated tsconfig currently sets
   `skipLibCheck: true` as a crutch.
2. It blocks a clean, Node-publishable `@types/deno`.

## Why Deno forked (the constraint we must preserve)

Deno is **not** a browser and **not** a web worker. Stock `lib.webworker.d.ts`
declares **284 global bindings**; Deno's real global surface is **~43**.
`lib.webworker` asserts globals Deno does not implement — CSS Typed OM
(`CSSMathClamp`, `CSSNumericValue`, ...), WebCodecs (`AudioData`,
`AudioDecoder`, `AudioEncoder`), ServiceWorker (`Client`, `Clients`),
`createImageBitmap`, `CookieStore`, and more.

If we naively adopted a stock lib as the base, TypeScript would green-light
`new AudioDecoder(...)` that throws `ReferenceError` at runtime. That is the
**false-positive class** the fork was created to avoid. Any de-fork MUST NOT
reintroduce it.

## The two-axis model

The key realization: "collides with dom" and "false positive" are two
_independent_ axes, and they need different mechanisms.

### Axis 1 — global bindings (`declare var X`, `declare function f`)

These assert "the runtime has this." This is the **only** axis where false
positives live. The binding _list_ must stay curated to exactly what Deno
implements. But the _type_ each binding resolves to can safely defer to stock
when stock is present.

Mechanism (the `@types/node` ↔ `lib.dom` coexistence trick):

```ts
declare var Request: typeof globalThis extends { document: any; Request: infer T }
  ? T
  : <DenoRequest>;
```

`document` is a DOM-exclusive marker (neither Deno nor `@types/node` declare
it). When real `lib.dom` is loaded, the conditional yields dom's `Request` type;
the two `declare var Request` declarations now have _identical_ types, so they
merge (no duplicate-identifier error). When dom is absent, Deno keeps its own
type and `@types/node` defers to it. The binding list is untouched, so no new
false positives.

### Axis 2 — interface shapes (`interface Request { ... }`)

These are pure type descriptions. An `interface` does not assert a global exists
— you only obtain a `Request` value by constructing one. So sharing shapes with
stock creates **no** false positive. Conflicts here are _duplicate-member_
conflicts under declaration merging: two global `interface Request` merge
silently **iff** their members are compatible.

Interfaces cannot use conditional types, so the Axis-1 deferral trick does not
apply. Shapes must instead be generated to be **structurally identical** to
stock, with Deno-only members split into a separate augmentation.

## Correctness invariant

> The curated global-binding list is inviolable. De-forking aligns the _types_
> and _shapes_, never the _membership_ of the global surface. We never inherit a
> stock lib's global list.

## What already exists

- `tools/generate_types_deno.ts` — generates the `@types/deno` namespace package
  for DefinitelyTyped from `deno types` output (ts-morph). The namespace half is
  self-contained and Node-safe (Deno fully owns `Deno.*`).
- `tools/apply_web_globals_deferral.ts` (landed in #35639) — the **Axis-1
  generator**. Already run; its output is committed in
  `cli/tsc/dts/lib.deno*.d.ts` (see the
  `typeof globalThis extends { document: any; ... }` conditionals). The overlap
  set is derived from `TYPES_NODE_IGNORABLE_NAMES` in `cli/tsc/mod.rs` so it
  stays in sync on `@types/node` bumps.

Axis 1 is therefore **done and proven in-tree**. It validates the whole
approach.

## Axis-2 progress (#36125)

Two mechanisms have landed, both validated with the ts-globals harness
(`deno+node` stays at 0 collisions throughout; `deno+dom` 373 → 253):

**Axis-2a — interface member reconciliation** (`tools/generate_deno_libs.ts`).
For each Deno interface member that has _accidentally_ drifted from stock (a
literal narrowed to `number`, a property that lost its `?`, `any` where stock
says `null`, ...), copy the stock member text back so declaration merging is a
silent no-op. Members where Deno _intentionally_ differs are recorded in the
tool's `KEEP` list and left alone. (16 reconciled, 19 kept.)

**Axis-2b — `declare class` → `interface` + `declare var`**
(`tools/defork_classes.ts`). The largest `TS2300` source: Deno declared 32 web
types (all the WebGPU types; the DOM event classes) as `declare class`, which
_cannot_ declaration-merge with `lib.dom`'s `interface`. Split each into its
two-axis shape — instance members become `interface X` (mergeable, no false
positive), the constructor/statics become `declare var X` (the binding, fed into
the Axis-1 deferral). Membership is unchanged: the class already _was_ the value
binding. (−84 collisions.)

Result: co-loading `dom` is much closer to a clean merge; once the tail below is
closed, `skipLibCheck` can flip to `false` and the `deno.ns`-only defer becomes
an optimization rather than a requirement.

### What remains

- **~90 identical `type X = …` aliases** (`URLPatternInput`, `BufferSource`,
  `ReferrerPolicy`, ...). Both Deno and stock declare them _identically_, but
  two identical type aliases still can't redeclare (`TS2300`) — and a type alias
  can't use the conditional-defer trick (that only works for `var`) or merge
  (that only works for `interface`). This is the genuinely-novel piece and needs
  a different mechanism (e.g. don't emit the alias when it's stock-identical and
  dom is present, or express the family as interfaces).
- **`WebAssembly` namespace inner classes** (`Memory`, `Table`, `Module`, ...) —
  the same class-vs-interface fix as Axis-2b, but one level down inside
  `declare namespace WebAssembly`; `defork_classes.ts` only handles top-level
  classes, and a namespace-scoped `var` can't use the `globalThis`-marker
  deferral directly.
- **Axis-3** — reconcile the `@types/node` deep web-stream shapes
  (`ReadableStreamBYOBReader` vs `NonSharedArrayBufferView`).

## Relationship to the `deno.ns`-only defer (interim)

Today, when a project's effective `lib` includes `dom`, the generated
`.deno/types/deno/index.d.ts` emits only the `Deno` namespace (`deno.ns` +
`deno.net`) and lets `dom` own the web globals (see
`get_deno_ns_declaration_file_text` in `cli/tsc/mod.rs` and `has_dom` in
`cli/tsc/tsconfig_gen.rs`). This is the crude-but-correct interim: it sidesteps
the Axis-2 interface collisions by not shipping web shapes at all when dom is
present. Once the Axis-2 generator lands, this defer becomes optional.

## Layering, end state

1. `@types/deno` — the `Deno` namespace + Deno-only globals; nothing stock
   already owns. Node-publishable. (`generate_types_deno.ts`.)
2. Web-global bindings — curated list, Axis-1 deferral applied. (Done.)
3. Web interface shapes — generated stock-identical + Deno augmentation overlay.
   (Axis-2a member reconciliation + Axis-2b `class`→`interface`+`var` conversion
   done in #36125; identical type aliases + `WebAssembly` namespace classes
   remain.)

## Open questions

- Is full dom co-load (`deno.window` + `dom`, shapes merging) worth the Axis-2
  generator, or is the `deno.ns`-only defer sufficient for dom users? The Axis-2
  win is `skipLibCheck: false` for Deno's _own_ default config; the dom co-load
  is a narrower benefit.
- Temporal (`esnext.temporal`) and `QuotaExceededError` are currently stripped
  at generation time. Under the Axis-2 model these become opt-in stock libs
  (once TS ships Temporal) or Deno augmentations — not strips.
- `@types/node` overlaps (`Buffer`, `NodeJS` namespace) are a third overlap
  source, orthogonal to dom; handled by `TYPES_NODE_IGNORABLE_NAMES` today.
