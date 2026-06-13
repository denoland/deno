# Research: Adding a native DOM API to Deno (cppgc-backed)

> Status: **Research only.** No implementation is proposed to be merged from this
> document. The goal is to assess what it would take to ship a built-in,
> server-side DOM in the Deno runtime, backed by V8's C++ garbage collection
> (cppgc / Oilpan), drawing on `jsdom` and `deno-dom` for inspiration.

## 1. Goal & scope

Provide a built-in DOM implementation in Deno such that globals like `Document`,
`Element`, `Node`, `Text`, `DOMParser`, `NodeList`, `HTMLCollection`, etc. exist
without a third-party dependency. The motivating constraints from the request:

- **Use the "gppgc" (cppgc) API** — i.e. model DOM nodes as native Rust objects
  managed by V8's garbage collector, rather than a pure-JS object graph or a
  Rust-side arena handed across an FFI boundary.
- **Server-side DOM only** (parsing + tree manipulation + querying). No layout,
  no rendering, no script execution inside parsed documents, no CSSOM in v1 —
  matching the non-goals of `deno-dom`.

This is the architecturally interesting decision: **where do the DOM nodes
live?** The two prior-art libraries answer differently, and neither uses cppgc.
A native cppgc DOM is a third option with distinct trade-offs (see §4).

**There is standing demand for this.** Deno has repeatedly been asked to ship a
built-in DOM/`DOMParser`: issue
[#6794 "Support for HTMLElement"](https://github.com/denoland/deno/issues/6794)
(2020, 48 reactions), [#5635](https://github.com/denoland/deno/issues/5635), and
the current canonical thread
[#24995 "Request: `DOMParser` (again)"](https://github.com/denoland/deno/issues/24995)
(open, 44 reactions, active as of 2026-06). The official manual currently just
points users at `deno-dom`/`linkedom`; Deno contributors in #24995 argue HTML
parse/stringify should be a built-in spec Web API or in `std` rather than a
third-party module. So this research targets a real, recurring user request.

## 2. Prior art

### 2.1 deno-dom (`b-fuze/deno-dom`)

- **Split design.** A Rust HTML parser (html5ever + markup5ever, scripting
  disabled) compiled to **both** WASM and a native dynamic library (FFI). A
  pure-TypeScript layer implements the actual DOM object model.
- **Parser is stateless / fire-and-forget.** Rust parses, **serializes the whole
  tree to a compact JSON string** (numeric node-type tags: document=9,
  element=1, text=3, comment=8, doctype=10), ships that string across the
  WASM/FFI boundary, and discards its tree. Native FFI transfers the same JSON
  via a pointer + copy (`deno_dom_parse_sync` → `deno_dom_copy_buf`).
- **The live DOM is pure JS/TS.** `src/deserialize.ts` does
  `JSON.parse(parse(html))` and rebuilds real `Element`/`Text`/`Comment`/...
  instances. After parse, **Rust retains nothing**; all mutation happens in JS.
- **Selectors** are pluggable: `nwsapi` when codegen (`new Function`) is allowed,
  falling back to `Sizzle` under CSP / Deno Deploy.
- **Coverage:** Node/Element/Document/DocumentFragment/Text/Comment, `innerHTML`/
  `outerHTML`/`textContent`, `children`/`childNodes`, attributes,
  `classList` (`DOMTokenList`), `querySelector(All)`. Ships WPT as a submodule
  and targets spec compliance.
- **Non-goals:** headless browser, running `<script>`, CSS/CSSOM, layout,
  obsolete-element quirks.
- **Perf:** native > WASM (avoids WASM startup); slower than LinkeDOM at parse,
  faster at some mutation; both far faster than jsdom. `classList` is lazily
  allocated (`UninitializedDOMTokenList`) to save memory.

### 2.2 jsdom

- **Pure JS, code-generated.** Interfaces are generated from WebIDL via
  `webidl2js`, which produces wrapper classes that enforce IDL conversions,
  brand checks, and reflected attributes. Tracks the WHATWG living standards.
- **Parser:** `parse5` (spec-compliant HTML5 tree construction) via a custom
  `JSDOMParse5Adapter` tree adapter — parse5 builds jsdom's own impl nodes
  directly rather than its default plain objects.
- **Selectors:** historically `nwsapi`; jsdom flip-flopped (v23.2.0 →
  `@asamuzakjp/dom-selector`, reverted in v24.0.0 for a perf regression, then
  re-added on `main`). **Styles:** historically `cssstyle` + CSSOM; current
  `main` uses `css-tree` + csstools patches (no layout/computed styles). Plus
  events, `MutationObserver`, `NodeIterator`/`TreeWalker`, ranges, forms, URL,
  cookies, optional script execution via Node `vm` — by far the most complete.
- **Weaknesses:** heaviest and slowest; ~50–65 MB just on `require`; ~500 ms
  startup; documented perf regressions (e.g. 23.1→23.2 roughly doubled some test
  suite times). The `webidl2js` wrapper/impl indirection adds a cost on every
  property access and method call (brand check → `this[implSymbol].method()`).
- **`webidl2js` is the key idea worth stealing conceptually:** each interface is
  a generated **wrapper** (enforces IDL type coercion, brand checks, arg-length,
  `@@toStringTag`) delegating to a hand-written **impl** class. Deno's
  `#[op2] impl` + `#[webidl]` + `#[derive(WebIDL)]` play the same role natively —
  the macro is the code generator, the Rust struct is the impl.

### 2.3 Faster pure-JS alternatives (context)

- **linkedom** (SSR-focused): a triple-linked-list node model (no recursion/array
  ops) that scales linearly to huge documents and serializes very fast (~82 KB
  vs jsdom ~1 MB), but **deliberately drops live collections** and full spec
  compliance.
- **happy-dom** (test-focused, Vitest default): ~2–10× faster than jsdom by
  sacrificing edge-case spec compliance.
- Both run on Deno Deploy; **jsdom does not**. Community consensus: deno-dom and
  linkedom both far outperform jsdom; deno-dom is slower than linkedom at parse,
  faster at some mutation.

### 2.4 Takeaway for a native design

`deno-dom` proves you can get a usable spec-ish DOM with a Rust parser + JS tree;
`jsdom` proves WebIDL-driven generation scales to near-complete coverage but is
slow. **Neither keeps the live tree in Rust.** A cppgc DOM is novel precisely
because the nodes themselves become GC-managed native objects — eliminating the
serialize/rebuild step and the JS-object-graph overhead, at the cost of more
Rust complexity and careful GC tracing.

## 3. The cppgc plumbing that already exists in Deno

This is the key enabler: Deno already exposes native, GC-managed classes to JS,
and it's used in production by WebGPU, `node:sqlite`, and canvas. A DOM would
reuse this machinery wholesale.

### 3.1 Declaring a native class

An extension registers cppgc-backed classes in the `objects = [...]` list of the
`extension!` macro (e.g. `ext/canvas/lib.rs:14-19`). Each class is a Rust struct
that:

1. implements `unsafe impl GarbageCollected` with `trace()` and `get_name()`, and
2. has an `#[op2] impl` block whose methods become JS prototype members.

Concrete example — `ext/canvas/canvas.rs`:

```rust
pub struct OffscreenCanvas {
  data: Rc<RefCell<DynamicImage>>,
  active_context: OnceCell<(String, v8::Global<v8::Value>)>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for OffscreenCanvas {
  fn trace(&self, _visitor: &mut Visitor) {}            // <- trace child refs here
  fn get_name(&self) -> &'static std::ffi::CStr { c"OffscreenCanvas" }
}

#[op2]
impl OffscreenCanvas {
  #[getter] fn width(&self) -> u32 { self.data.borrow().width() }
  #[setter] fn width(&self, /* ... */ #[webidl(options(enforce_range = true))] value: u64) { /* ... */ }

  #[constructor]
  #[cppgc]
  #[required(2)]
  fn new(
    #[webidl(options(enforce_range = true))] width: u64,
    #[webidl(options(enforce_range = true))] height: u64,
  ) -> OffscreenCanvas { /* ... */ }

  fn get_context<'s>(&self, #[this] this: v8::Global<v8::Object>, /* ... */) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> { /* ... */ }

  #[cppgc] fn transfer_to_image_bitmap(&self, /* ... */) -> Result<ImageBitmap, JsErrorBox> { /* ... */ }
}
```

The JS side just re-exports the auto-generated class (`ext/canvas/01_canvas.js`):

```js
import { OffscreenCanvas } from "ext:core/ops";
export { OffscreenCanvas };
```

…and it is wired as a global lazily (`runtime/js/98_global_scope_shared.js:300`):

```js
OffscreenCanvas: core.propNonEnumerableLazyLoaded((canvas) => canvas.OffscreenCanvas, loadCanvas),
```

### 3.2 Op signatures with cppgc (from agent investigation + tests)

- **Return a native object:** mark the fn `#[cppgc]` (or just return the type);
  conversion to a wrapped JS object is automatic via `make_cppgc_object`.
- **Accept a native object:** `#[cppgc] _x: &T` or `#[cppgc] _x: Option<&T>`.
  Unwrapping uses `try_unwrap_cppgc_object` (slow path) /
  `try_unwrap_cppgc::<T>` (fast path).
- **Async:** cppgc args are auto-rooted (`UnsafePtr::root()`) across `await` so
  the GC can't collect them mid-op.
- **`#[this]`** gives a method access to its own JS wrapper object — needed when
  a returned child must reference its parent's JS object.

### 3.3 Inheritance (critical for DOM)

The DOM is deeply hierarchical (`EventTarget → Node → Element → HTMLElement →
HTMLDivElement …`). Deno's cppgc layer **already supports native inheritance**:

- Base class: `#[op2(base)] impl BaseType` + `#[derive(CppgcBase)]`,
  `#[repr(C)]`.
- Derived: `#[op2(inherit = BaseType)] impl Derived`, with
  `#[derive(CppgcInherits, CppgcBase)] #[cppgc_inherits_from(BaseType)]` and the
  **base struct as the first field at offset 0** (compile-time asserted via
  `offset_of!`).
- Polymorphic unwrap via `try_unwrap_cppgc_base_object` accepts any subclass.
- TypeId tagging (inventory-built transitive graph) enforces type safety.

Source: `libs/core/cppgc.rs`, `libs/ops/cppgc.rs`,
`libs/ops/op2/test_cases/{sync,async}/*cppgc*.rs`, `ext/webgpu/*` (20+ GPU
classes use exactly this pattern).

### 3.4 Supporting infrastructure already present

- **WebIDL conversions:** `libs/core/webidl.rs` + `#[webidl]` arg attribute +
  `#[derive(WebIDL)]` dictionaries/enums (used heavily in `ext/webgpu`). DOM
  needs reflected attributes, enums (`insertAdjacentHTML` positions, etc.),
  and dictionary options — all expressible here.
- **EventTarget:** `ext/web` already implements `EventTarget`/`Event` in JS.
  `Node` must inherit `EventTarget`; we'd decide whether to keep EventTarget in
  JS (and have the native `Node` extend it) or port it native. See §6.
- **Extension registration / snapshot / types:** new ext goes in `ext/dom/`,
  is added to the workspace `Cargo.toml` members, registered in
  `runtime/snapshot.rs`, `runtime/snapshot_info.rs`, `runtime/worker.rs`,
  `runtime/web_worker.rs`, exposed as globals in
  `runtime/js/98_global_scope_shared.js`, and given a `lib.deno_dom.d.ts`
  under `cli/tsc/dts/` (referenced from the appropriate `lib.deno.*.d.ts`).

## 4. Two viable native architectures

### Option A — "node lives in cppgc" (the requested design)

Each DOM node is a cppgc struct:

```
EventTarget (cppgc base, possibly the JS one)
└─ Node          { parent: Member<Node>, first_child, next_sibling, prev_sibling, ... }
   ├─ Document
   ├─ DocumentFragment
   ├─ Element     { local_name, namespace, attrs, ... } ── HTMLElement ── HTML*Element
   ├─ CharacterData ── Text / Comment / CDATASection / ProcessingInstruction
   └─ DocumentType
```

- **Tree links** are `cppgc::Member<T>` fields, and `trace()` visits them so the
  GC keeps reachable nodes alive and collects detached subtrees. This is the
  whole point of using cppgc: the tree's lifetime is managed by V8, cycles
  (parent↔child) are fine, and there's no manual refcounting or arena.
- **Parsing** uses html5ever/`markup5ever` but with a **custom `TreeSink`** that
  builds cppgc nodes directly (instead of `RcDom` + JSON). This avoids
  deno-dom's serialize→`JSON.parse`→rebuild round-trip entirely.
- **Live collections** (`NodeList`, `HTMLCollection`, `children`) can be true
  live views because the backing tree is native and queryable on demand.

### Option B — "node lives in a Rust arena, JS holds handles"

A single cppgc-managed `Document` owns an arena (e.g. `Vec<NodeData>` with
index-based links, à la `kuchikiki`/`indextree`); each JS node is a thin cppgc
wrapper holding `(document, NodeId)`. Simpler memory model (one owner, no
per-node GC tracing of the tree), but: node identity/liveness must be managed
manually, detached-subtree GC is manual, and `trace()` only roots the document.

**Recommendation:** Option A matches the request ("gppgc API") and is the more
elegant fit for V8's GC — but it is the harder one to get right (every
tree-mutation algorithm runs in Rust and must trace correctly). Option B is a
reasonable fallback if per-node cppgc overhead proves too high. A hybrid is also
possible (cppgc `Node` objects whose links are arena indices).

## 5. Rust crate landscape (all actively maintained as of mid-2026)

| Crate | Role | Builds a tree? | Use here |
|---|---|---|---|
| **html5ever** `0.39` (Servo) | WHATWG HTML5 tokenizer + tree builder | No — `TreeSink` callbacks; you own the tree | **Parser. Recommended** (same one deno-dom uses) |
| **markup5ever** `0.39` / **tendril** `0.5` | shared `TreeSink` traits, interned names (`web_atoms`), zero-copy strings | No (infra) | Transitive deps of html5ever |
| **xml5ever** `0.39` | XML5 push parser | No — `TreeSink` | If/when `DOMParser` XML mode is wanted |
| **markup5ever_rcdom** | reference `Rc<RefCell>` DOM | Yes, mutable | *Reference only* — `publish=false`, "don't build a browser with it" |
| **selectors** `0.38` + **cssparser** `0.37` (Servo/Stylo) | CSS parse + match, generic over any tree via the `Element` trait | No (matching engine) | **Native `querySelector(All)`** — implement `selectors::Element` for our `Element` |
| **ego-tree** `0.11` | Vec-backed arena ID-tree | Yes, mutable | Model for Option B (note: "detach, never free" — orphans keep their slot) |
| **scraper** `0.27` | html5ever + ego-tree + selectors | Yes (read-query-first) | Study/wiring reference, not the live tree |
| **kuchikiki** (Brave fork of archived `kuchiki`) | html5ever-backed `Rc` DOM + selectors | Yes, mutable | Reference for wiring `selectors` to a custom tree (`select.rs`) |
| **lol_html** (Cloudflare) | streaming rewriter | **No tree** | Wrong shape for a DOM |

Key point: **no existing crate is a drop-in WHATWG DOM.** html5ever gives a
correct parser but explicitly no DOM ("it does not provide any DOM tree
representation" — you implement `TreeSink`). The tree crates are either reference
quality (`RcDom`), scraping-oriented (`scraper`/`ego-tree`), or tree-manipulation
libraries without live collections / ranges / mutation observers / spec-exact
mutation algorithms (`kuchikiki`). So a native Deno DOM ≈ **html5ever (parser)
via a custom cppgc `TreeSink` + selectors/cssparser (querying) + a bespoke
spec-faithful DOM API layer.** `selectors`/`cssparser` keep matching in Rust and
are far better than bundling nwsapi/Sizzle.

### 5.1 The gold-standard reference: Servo's `script` crate

Servo's `components/script/dom/` is the closest real-world analog to what's
proposed, and validates the cppgc approach:

- DOM objects are **native Rust structs whose lifetime is managed by the JS
  engine's GC** (SpiderMonkey), not by Rust ownership — via a **reflector**
  (the JS wrapper object) that owns the native object. This is *exactly* Deno's
  cppgc model (`make_cppgc_object` wraps the Rust value in a V8 object; the GC
  frees it). Blink uses the same pattern with C++ + cppgc/Oilpan.
- Inter-node edges use a traced smart pointer `Dom<T>` (cf. cppgc `Member<T>`),
  with explicit **rooting** (`DomRoot<T>`) for stack use and a `JSTraceable`
  trace hook (cf. our `trace()`); a custom lint forbids un-rooted `Dom<T>` on
  the stack to prevent use-after-free.
- Interior mutability via `DomRefCell` for the shared, GC-reachable graph.

Takeaway: the **reflector + traced-member + rooting** triad is precisely what
Deno's cppgc layer already provides. Servo's `script` crate is not reusable (it's
welded to SpiderMonkey), but it's the proof that this architecture yields a full,
mutable, spec-compliant DOM — and a design template for ours.

## 6. Hard problems / risks (in rough priority order)

1. **GC tracing correctness.** Every `Member<T>` reference in the tree must be
   traced; a missed trace = use-after-free, an over-retained ref = leak. Mutation
   ops (`appendChild`, `removeChild`, `insertBefore`, `replaceChild`,
   re-parenting) must update links *and* keep tracing sound. This is the single
   biggest correctness risk and where most review effort will go.
2. **EventTarget integration.** `Node extends EventTarget`. Either (a) keep
   `ext/web`'s JS EventTarget and have the native `Node` prototype chain into it
   (mixing native + JS prototypes), or (b) port EventTarget to native cppgc.
   Listener storage must be traced if native. Needs a decision early.
3. **The HTML parsing spec is large.** Foster parenting, the "adoption agency"
   algorithm, template contents, fragment parsing context, namespaces
   (SVG/MathML), insertion modes. html5ever handles tokenization/tree-construction,
   but our `TreeSink` must implement all its callbacks correctly.
4. **Live collections & invalidation.** `getElementsByTagName`/`.children`/
   `NodeList` liveness; caching with correct invalidation on mutation. jsdom and
   browsers use generation counters — we'd do similar.
5. **WebIDL surface area.** Reflected attributes (`id`, `className`,
   `htmlFor`…), `DOMTokenList`, `NamedNodeMap`/`Attr`, stringifiers, legacy
   behaviors. Large but mechanical given `#[webidl]` + `#[derive(WebIDL)]`.
6. **Serialization** (`innerHTML`/`outerHTML` getters): the HTML fragment
   serialization algorithm (escaping rules, void elements, `<template>`,
   raw-text elements). Plus `innerHTML` *setter* = fragment parsing.
7. **Scope creep.** CSSOM, `MutationObserver`, ranges, `TreeWalker`,
   `XMLSerializer`/`DOMParser` for XML, `Document` events/`readyState`. Must be
   explicitly deferred (mirror deno-dom non-goals) to keep v1 tractable.
8. **`structuredClone`/serialization & workers.** DOM nodes are not
   transferable/serializable across workers — must be excluded from the
   structured-clone path cleanly.
9. **Type declarations & `lib` choice.** TypeScript already ships full DOM lib
   types; Deno deliberately does *not* expose `lib.dom`. We'd need our own
   `lib.deno_dom.d.ts` and to decide which global scopes (window/worker) get it,
   plus whether it's `--unstable-dom`-gated initially.

## 7. Suggested phasing (if it proceeds)

1. **Spike:** `ext/dom` with cppgc `Node`/`Document`/`Element`/`Text`/`Comment`,
   manual tree construction (`createElement`/`appendChild`/`textContent`), no
   parser. Prove `trace()` soundness with stress tests + `--v8-flags` GC
   stress. Decide Option A vs B and EventTarget strategy here.
2. **Parser:** custom html5ever `TreeSink` → cppgc tree; `DOMParser`,
   `innerHTML` setter (fragment parsing).
3. **Serialization:** `innerHTML`/`outerHTML`/`textContent` getters.
4. **Querying:** native `querySelector(All)` via `selectors` + `cssparser`;
   `getElementById/ByClassName/ByTagName`.
5. **Attributes & reflection:** `Attr`/`NamedNodeMap`, `classList`, reflected
   IDL attributes.
6. **Live collections, then WPT** (`tests/wpt` `dom/`, `domparsing/`,
   `html/dom/`) as the compliance bar; `tests/unit/dom_test.ts` for unit tests;
   `tests/specs/dom/` for CLI-level behavior.
7. **Gate behind `--unstable-dom`** until WPT pass-rate is acceptable.

## 8. Bottom line

- The **hard infrastructure already exists**: Deno can expose native,
  GC-managed, inheriting classes to JS with getters/setters/constructors/async,
  WebIDL conversions, and global registration. WebGPU (20+ classes) and
  `node:sqlite` are proof.
- The **novel, valuable part** of a cppgc DOM is keeping the *live tree* in
  native GC-managed objects (Option A), avoiding deno-dom's serialize/rebuild
  and jsdom's JS-object-graph overhead — potentially the fastest server DOM
  available while staying spec-compliant.
- The **dominant cost/risk is not plumbing but DOM semantics**: GC-tracing
  soundness across mutation, the HTML parsing/serialization algorithms,
  EventTarget integration, live collections, and the sheer WebIDL surface.
- A **realistic v1** = parse + tree + mutation + serialization + selectors +
  reflected attributes, behind `--unstable-dom`, validated by WPT, with CSSOM /
  MutationObserver / ranges / XML explicitly deferred.

### Key file references

- cppgc core: `libs/core/cppgc.rs`, `libs/ops/cppgc.rs`,
  `libs/core/runtime/ops_rust_to_v8.rs`
- op2 cppgc handling: `libs/ops/op2/signature.rs`,
  `libs/ops/op2/dispatch_{fast,slow}.rs`,
  `libs/ops/op2/test_cases/{sync,async}/*cppgc*.rs`
- Example native classes: `ext/canvas/canvas.rs`, `ext/webgpu/*`,
  `ext/node_sqlite/database.rs`
- WebIDL: `libs/core/webidl.rs`, `ext/webgpu/webidl.rs`
- Extension wiring: `ext/canvas/lib.rs`, `runtime/snapshot.rs`,
  `runtime/snapshot_info.rs`, `runtime/worker.rs`, `runtime/web_worker.rs`,
  `runtime/js/98_global_scope_shared.js`, `cli/tsc/dts/`
