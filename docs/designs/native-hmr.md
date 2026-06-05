# Native Hot Module Replacement for Deno

Status: draft (Phase 1 in progress) Author: Bartek Iwanczuk Related:
dynamic-uncached-import (#25780), ESM-HMR spec
(https://github.com/FredKSchott/esm-hmr)

## Motivation

The current `--watch-hmr` flag (`cli/tools/run/hmr.rs`) is not module
replacement. It drives the V8 inspector over a `LocalInspectorSession` and calls
`Debugger.setScriptSource` to patch function bodies in place. V8's
`setScriptSource` can only swap function bodies: any structural change (a new or
removed export, a changed top-level binding, a new import) returns
`BlockedByTopLevelEsModuleChange` and the runner falls back to a full process
restart (`hmr.rs:58`, `hmr.rs:292`). The only runtime signal is a single global
`dispatchEvent(new CustomEvent("hmr"))` (`hmr.rs:347`) -- there is no per-module
boundary, no `accept`/`dispose`, no dependency-aware propagation, and no way to
hand off state across a reload.

The goal is true HMR: actually re-evaluate changed modules with new top-level
bindings, rebind their importers, propagate the update along the dependency
graph to the nearest accepting boundary, and expose the ESM-HMR / Vite
`import.meta.hot` API so frameworks and apps hand off state instead of
restarting.

## Architecture: two layers

Layer 1 -- a `deno_core` reload engine (`libs/core/modules/`). Given a set of
changed specifiers, evict them (and their transitive importers) from the
`ModuleMap`, re-drive a `RecursiveModuleLoad` from the root, re-instantiate and
re-evaluate the evicted set, and rely on V8's instantiation binding to wire the
fresh importers to the fresh dependencies. Survivors keep their live instances
and singletons.

Layer 2 -- the `import.meta.hot` boundary API (ESM-HMR / Vite shape) plus an
imperative `Deno.reloadModule`, implemented in a JS extension on top of Layer

1. `import.meta.hot` is the headline public API; `Deno.reloadModule` is the
   low-level escape hatch.

Layer 3 (later) -- a browser-facing WebSocket ESM-HMR protocol for frontend
frameworks served by Deno (`deno serve`), riding on the same boundary registry.

The CDP/inspector path is removed for HMR; the `FileWatcher` plumbing is kept as
the change source.

## Layer 1: the reload engine (code-grounded)

Verified mechanics in the current module system:

- A `RecursiveModuleLoad` registers the entire graph into `by_name` before
  instantiation. A module already present in `by_name` is skipped, not
  re-fetched or re-compiled (`map.rs:487-490` via `get_id`; recursion check at
  `recursive_load.rs:450-457`).
- Instantiation binds each import via `module_resolve_callback` ->
  `resolve_callback` -> `get_id` + `get_handle` (`map.rs:1557`). Once a
  `v8::Module` is instantiated its bindings are fixed; a `v8::Module` cannot be
  re-instantiated (status guard `map.rs:1290-1295`) nor re-evaluated (status
  guard `map.rs:1784`, `map.rs:1972`).
- A freshly compiled `v8::Module` (new `ModuleId`) instantiates and evaluates
  independently and resolves its imports through `by_name` -- so it picks up
  whatever instances are currently registered.
- The old `v8::Module`'s evaluated top-level state persists as long as its
  `v8::Global` handle is alive.
- Storage is append-only: `ModuleId` is a raw `usize` index into
  `handles`/`info` Vecs; `by_name` is a `HashMap` with no `remove`
  (`module_map_data.rs`).

Algorithm for `reload_modules(changed: &[ModuleSpecifier])`:

1. Compute the reload closure: `changed` plus all transitive importers. The
   importer relation is the inverse of `ModuleInfo.requests`; built on demand.
   Without boundaries (Phase 1) the closure always reaches the main module.
2. Evict every specifier in the closure from `by_name` (new
   `ModuleNameTypeMap::remove` + `ModuleMapData::evict`). Their handles/info
   slots are tombstoned (left in place so existing `ModuleId` indices stay
   valid, but unreachable by specifier). This keeps `next_load_id` and index
   invariants intact; eviction only ever runs at runtime, never at snapshot
   time, so the snapshot length asserts are unaffected.
3. Re-drive a `RecursiveModuleLoad` from the root (mirroring
   `load_side_es_module_from_code`, `jsrealm.rs:645`). Evicted specifiers miss
   `by_name` and are recompiled into fresh `v8::Module`s; survivors are skipped
   and keep their instances.
4. Instantiate the new root. V8 binds fresh importers to fresh dependencies and
   to surviving instances via `by_name`.
5. Evaluate the new modules (`mod_evaluate`, async for TLA).

The "A imports C, B imports C, change A" case: only A and the root import A, so
only A and the root are evicted; B and C survive with state intact. The root
re-evaluates, re-imports the fresh A and the surviving B/C.

## Layer 2: `import.meta.hot` (ESM-HMR / Vite shape)

```ts
interface ImportMetaHot {
  readonly data: Record<string, unknown>;
  accept(): void;
  accept(cb: (mod: { module: unknown }) => void): void;
  accept(
    deps: string[],
    cb: (a: { module: unknown; deps: unknown[] }) => void,
  ): void;
  dispose(cb: (data: Record<string, unknown>) => void): void;
  decline(): void;
  invalidate(): void;
  on(event: string, cb: (payload: unknown) => void): void;
  off(event: string, cb: (payload: unknown) => void): void;
}
```

A module that calls any `accept` form becomes a boundary: the closure climb in
Layer 1 stops there instead of reaching the root. The boundary re-acquires its
changed dependency itself (dynamic `import()` re-resolves through `by_name` to
the fresh instance). `dispose(cb)` runs before teardown and writes into
`hot.data`, which is exposed on the new instance -- the state hand-off that
replaces "restart and lose everything". `decline()`/`invalidate()` fall back to
`FileWatcher::force_restart` (`file_watcher.rs:201`).

Each boundary registers itself in a runtime-side registry (specifier ->
accept/dispose handlers, accepted deps, data). Layer 1 queries it to find the
nearest accepting boundary. The legacy global `"hmr"` event keeps firing for
backward compatibility.

Transform: `emit_for_hmr` (`libs/resolver/emit.rs:316`) injects a per-module
`import.meta.hot` bound to the module's specifier, and (for non-HMR builds)
dead-code-strips `if (import.meta.hot) { ... }` so production / `deno bundle` /
`deno compile` are unaffected. The current `source_map = None` workaround
(needed only because `setScriptSource` choked on embedded maps) is dropped.

## Open decisions

- Module identity after reload: tombstone old `ModuleId` (chosen for Phase 1 --
  keeps `usize` indices stable, minimal blast radius) vs a generational
  `ModuleId` (cleaner but ripples through every index site and the snapshot
  format).
- Reload-set specification when invoked imperatively: keep-list vs reload-list
  vs predicate.
- Do not reintroduce bare `import.meta.invalidate` (#15685, closed not-planned)
  without the `hot.data` state hand-off.

## Phasing

1. deno_core reload engine + eviction primitives + `Deno.reloadModule`
   (unstable) + unit tests. (In progress.)
2. `import.meta.hot` registry + API + transform injection + boundary/bubbling.
3. Native runner replacing the CDP runner in `run` and `serve`; keep
   `force_restart` fallback and legacy `"hmr"` event.
4. Browser WebSocket ESM-HMR protocol for frontend serving.

## Testing

- deno_core unit tests (`libs/core/modules/tests.rs`): evict + re-drive single
  module, A/B-keep-C singleton preservation, importer closure to root, cycles,
  TLA module reload, error propagation.
- Spec tests (`tests/specs/run/hmr_*`): accept-self, accept-with-handler,
  dispose state hand-off, bubble to parent boundary, decline -> full restart,
  shared-singleton preservation, TLA reload.
  </content>
  </invoke>
