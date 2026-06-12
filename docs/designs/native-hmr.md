# Native Hot Module Replacement for Deno

Status: draft (Phases 1-3 implemented; `deno run --watch-hmr` works) Author:
Bartek Iwanczuk Related: dynamic-uncached-import (#25780), ESM-HMR spec
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

Algorithm for `reload_es_module(changed: &ModuleSpecifier)` (implemented):

1. Evict the changed specifier from `by_name` (`ModuleNameTypeMap::remove` +
   `ModuleMapData::evict_modules`). The `handles`/`info` slots are tombstoned
   (left in place so existing `ModuleId` indices stay valid, but unreachable by
   specifier). This keeps `next_load_id` and index invariants intact; eviction
   only ever runs at runtime, never at snapshot time, so the snapshot length
   asserts are unaffected.
2. Re-drive a load for the changed specifier (mirroring
   `load_side_es_module_from_code`, `jsrealm.rs:645`). It misses `by_name` and
   is recompiled into a fresh `v8::Module`; its dependencies are skipped and
   keep their instances. The JS escape hatch `Deno.core.reloadEsModule` does the
   same via `op_reload_module_evict` + a dynamic `import()`, reusing the
   dynamic-import pipeline for the async load/instantiate/evaluate.
3. Evaluate the fresh module.

Only the named module is evicted -- not its importer closure. Importers of the
old instance are not rebound; that is the job of Layer 2 boundaries, which
re-acquire the new instance via their `accept` callbacks. So the "A imports C, B
imports C, change C" case: reloading C re-runs only C, and B/A keep their state;
a boundary in B or A receives the new C through its accept handler.

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

Implemented in the JS HMR runtime in `libs/core/01_core.js`, exposed as
`Deno.core.enableHmr()` (registers the `import.meta.hot` factory via
`op_set_create_hot_context`; returns `applyHmrUpdate`) and
`Deno.core.applyHmrUpdate(changedSpecifier)`.

A module that calls any `accept` form becomes a boundary. `applyHmrUpdate` walks
_up_ the importer graph from the changed module -- querying direct importers
from Rust via `op_hmr_module_importers` (backed by
`ModuleMapData::direct_importers`, the inverse of `ModuleInfo.requests`) --
until each path reaches an accepting boundary. A path that reaches an entry
point with no boundary, or hits a `decline()`d module, makes `applyHmrUpdate`
return `false` (the caller must do a full reload). Relative `accept(['./dep'])`
specifiers are resolved to absolute URLs via `op_hmr_resolve` so they match the
graph. `dispose(cb)` runs before the reload and writes into `hot.data`, which is
preserved across the reload (the factory reuses the context object, resetting
handlers but keeping `data`) -- the state hand-off that replaces "restart and
lose everything". The changed module is reloaded once via `reloadEsModule`, then
each boundary's accept callback receives the fresh namespace. `invalidate()`
throws a sentinel that aborts the update and reports `false`.

Each boundary registers itself in a JS-side registry (specifier ->
accept/dispose handlers, accepted deps, data). `applyHmrUpdate` queries it to
find the nearest accepting boundary. The legacy global `"hmr"` event keeps
firing for backward compatibility.

No source transform is needed. Unlike Vite (a bundler that rewrites
`import.meta.hot`), Deno controls `import.meta` natively: when HMR is enabled,
`host_initialize_import_meta_object_callback` (`bindings.rs`) attaches `hot` to
each `file:` module's `import.meta` by invoking the registered JS factory. In
non-HMR runs the factory is never registered, so `import.meta.hot` is simply
`undefined` -- production / `deno bundle` / `deno compile` are unaffected with
no dead-code stripping.

## CLI integration (`--watch-hmr`)

`deno run --watch-hmr` drives the engine natively (the CDP
`Debugger.setScriptSource` runner is removed). Before the main module loads, the
worker calls `Deno.core.enableHmr()` so `import.meta.hot` is attached to the
entry module and its whole graph. While the program's event loop is live, the
worker `select!`s it against the file watcher's `watch_for_changed_paths`; on a
change it reads and transpiles the new source (`emit_for_hmr`), registers it in
a per-specifier `HmrSourceOverrides` map that the module loader checks first in
`load_code_source` (so the post-evict `import()` recompiles from fresh source,
not the stale module-graph source), then drives `applyHmrUpdate`. If no
accepting boundary handles the change -- or a path can't be hot-replaced -- it
falls back to the watcher's full restart (`force_restart`). When the program's
event loop finishes, control returns to the outer watcher, which restarts on
further changes as usual.

Under symlinked paths (e.g. macOS `/tmp` -> `/private/tmp`) the watcher's
canonical path can differ from the module's specifier. The runner maps each
changed path back to the specifier the module was registered under: if the
direct file-URL conversion isn't in the module map, it compares the
canonicalized changed path against the canonicalized path of every loaded
`file:` module (`JsRuntime::loaded_module_specifiers`) and uses the matching
registered specifier, so the boundary lookup and the loader source-override hit
regardless of which side the OS canonicalized.

## Open decisions

- Self-accept callback payload: both `accept(cb)` and `accept(deps, cb)` receive
  the ESM-HMR `{ module }` shape. Vite instead passes the bare namespace to
  self-accept callbacks, so code written for Vite needs a destructure change;
  revisit if Vite compatibility turns out to matter more than internal
  consistency.

- Module identity after reload: tombstone old `ModuleId` (chosen for Phase 1 --
  keeps `usize` indices stable, minimal blast radius) vs a generational
  `ModuleId` (cleaner but ripples through every index site and the snapshot
  format).
- Reload-set specification when invoked imperatively: keep-list vs reload-list
  vs predicate.
- Do not reintroduce bare `import.meta.invalidate` (#15685, closed not-planned)
  without the `hot.data` state hand-off.

## Phasing

1. deno_core reload engine + eviction primitives + `Deno.core.reloadEsModule`
   - unit tests. (Done.)
2. `import.meta.hot` registry + API + native attachment + boundary/bubbling
   (`Deno.core.enableHmr` / `applyHmrUpdate`). (Done.)
3. Native runner replacing the CDP runner for `deno run --watch-hmr`: enable HMR
   before the main module loads, map `FileWatcher` changes to `applyHmrUpdate`
   with a loader source-override, keep the `force_restart` fallback. (Done,
   including symlink-path canonicalization and integration tests for the
   hot-replace flow.) Remaining: user-facing `Deno.reloadModule` (unstable);
   `deno serve` mode.
4. Browser WebSocket ESM-HMR protocol for frontend serving.

## Testing

- deno_core unit tests (`libs/core/modules/tests.rs`), implemented: reload
  preserves a dependency singleton (Rust and JS APIs); `import.meta.hot`
  self-accept with `dispose` -> `data` hand-off; dependency-accept boundary;
  no-boundary change reports full-reload-required.
- Integration tests (`tests/integration/watcher_tests.rs`, `run_hmr_*`): server
  handler hot-swap through a dependency-accept boundary; JSX dependency
  hot-swap; self-accept with `hot.data` hand-off across a reload (also covers
  the symlinked temp-dir path mapping); transpile-failure fallback to restart;
  uncaught-error and unhandled-rejection restart flows. The pre-existing
  `run_hmr_server` / `run_hmr_jsx` / `run_hmr_compile_error` tests encoded CDP
  function-patching semantics (hot replacement with no boundary) and were
  rewritten for boundary semantics.
