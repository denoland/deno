# Per-module permissions (prototype proposal)

Status: draft / exploratory Author: @bartlomieju Scope: deno_core + ext/node +
cli

## 1. Goal

Today Deno's permission model is process-wide. A single `--allow-read` grant
applies to every line of JavaScript running in the isolate, regardless of which
module it lives in. We want a finer model: grant a capability (file read,
network, env, ffi, ...) to specific modules and deny it to others.

Concretely, the question a permission check must answer changes from:

> "Is this process allowed to read files?"

to:

> "Is the module that is currently executing this operation allowed to read
> files?"

The key design constraint is performance. The check has to be cheap enough to
run on every op invocation, including ops that take the V8 fast-call path. This
document proposes a prototype and, in particular, analyzes whether the "which
module is running" lookup is available inside fast calls.

## 2. Background: V8 already tracks the current script for us

We do not need to assign and thread module identity by hand if we lean on a
mechanism V8 maintains for free.

When deno_core compiles a module it builds a `v8::ScriptOrigin`. One field of
that origin is `host_defined_options`, an arbitrary `v8::PrimitiveArray` the
embedder controls. V8 stores it on the compiled `Script` for the lifetime of
that script and never interprets it.

We already use this. `ModuleLoader::get_host_defined_options`
(`libs/core/modules/loaders.rs`) is called during instantiation in
`ModuleMap::instantiate_module` (`libs/core/modules/map.rs`), and the result is
passed into `script_origin(...)` ->
`v8::ScriptOrigin::new(..., host_defined_options)`. The CLI overrides the hook
in `cli/module_loader.rs` and `cli/rt/run.rs` to stamp npm-package modules with
`[Boolean(true)]` (see `create_host_defined_options` in `ext/node/lib.rs`). That
marker is what the "managed globals" code reads back.

The read side is `Isolate::GetCurrentHostDefinedOptions()`, exposed in the v8
crate as `scope.get_current_host_defined_options()`. It returns the host-defined
options attached to the script of the **top JS frame currently executing**. It
is a direct lookup of "what script is running right now", not a stack walk, so
it is cheap and allocation-free on our side.

This gives us a natural substrate for per-module permissions: store a module
identity in `host_defined_options` at compile time, then read it back at op time
via the current script.

### Important semantic: "current" means the top frame

`GetCurrentHostDefinedOptions()` reflects the script of the immediately
executing frame, not the "first user frame" and not the importing module. If
privileged module A calls a helper function defined in unprivileged module B,
and B's code is what calls `op_read_file`, the check sees B. This is a
defensible semantic (the code literally performing the syscall is the code being
checked), but it has confused-deputy implications that section 8 discusses. It
is deliberately different from `op_current_user_call_site`, which walks frames
to find the first user frame and is comparatively expensive.

## 3. Design overview

Three pieces:

1. **Module identity.** Assign each compiled module a stable id. A monotonically
   increasing `u32`/`u64` per realm is enough; UUIDs are unnecessary. The id is
   written into the module's `host_defined_options` at instantiation.

2. **Permission table.** A side table held in `OpState` (or a dedicated realm
   structure) maps `module_id -> PermissionSet`. It is populated from config
   (import-map-adjacent policy, a manifest, or flags) before or during module
   loading.

3. **Op-level check.** Permission-sensitive ops resolve the current module id,
   look it up in the table, and allow or deny. This replaces (or augments) the
   current process-wide `PermissionsContainer` check.

```
compile module ──► host_defined_options = [kind, module_id]
                          │
run module code, calls op_read_file(path)
                          │
   op reads current script's module_id  ◄── GetCurrentHostDefinedOptions()
                          │
   look up module_id in PermissionSet table (OpState)
                          │
          allow ──► proceed     deny ──► throw PermissionDenied
```

### Identity encoding

Extend the existing `host_defined_options` PrimitiveArray rather than inventing
a parallel channel. Reserve index slots:

- index 0: kind tag (already used; e.g. node-managed marker, vm markers)
- index 1: module id (new)

This keeps it compatible with the existing `host_defined_options_kind` machinery
in `libs/core/runtime/host_defined_options.rs` and the `node:vm` markers, which
already use index 0 for a kind and index 1 for a key.

## 4. The central question: is the lookup available in fast calls?

This is the part the prototype must de-risk before committing to the design.

### 4a. What op2 fast calls can access

Reviewing the op2 macro (`libs/ops/op2/dispatch_fast.rs`), a fast op has more
reach than is commonly assumed:

- `&OpState`, `&mut OpState`, `Rc<RefCell<OpState>>`, and `&JsRuntimeState` are
  all fastcall-compatible (per the args table in `libs/ops/op2/README.md`).
- A `HandleScope` is available in fast calls. When an op requests a scope, the
  macro sets `needs_scope` and constructs a `CallbackScope` from the
  `FastApiCallbackOptions` (`dispatch_fast.rs` around the `Special::HandleScope`
  handling and `isolate_unchecked_mut()`).
- The op's `data` external (its `OpCtx`) is reachable in fast calls via
  `FastApiCallbackOptions.data`.

So purely mechanically, a fast op can obtain a scope and therefore call
`scope.get_current_host_defined_options()` and read the PrimitiveArray.

### 4b. The real unknown: does V8 report the correct current script in a fast call?

The mechanical availability of a scope does not guarantee correct semantics. V8
fast API calls are invoked directly from optimized (TurboFan) code without the
normal C++ entry frame. Some isolate-introspection APIs depend on that entry
frame / entered-context state. In particular `Isolate::GetCurrentContext()` is
known to be unreliable in fast calls. `GetCurrentHostDefinedOptions()` is in the
same family (it resolves "the script of the current frame"), so its behavior in
a fast call is **not guaranteed by the public contract** and must be verified
empirically.

Two outcomes are possible:

- Best case: because the fast call is made directly from the calling JS frame
  with no intervening C++ frame, the "current" script is exactly the caller's,
  and the API returns the right options. If so, per-module checks work in fast
  calls with only the cost of building a scope and reading two array slots.
- Worst case: V8 returns an empty handle or the wrong script in fast calls. Then
  the ambient lookup is unusable on the fast path and we need design B below.

Action item: a small spike op (`op2(fast)`) that calls
`get_current_host_defined_options()` and compares against the slow-path value
across regular ESM, npm modules, dynamic imports, and re-entrant calls. This is
maybe a day of work and decides the architecture.

### 4c. Even in the best case, cost matters

Building a `CallbackScope` and reading a PrimitiveArray in every fast op is not
free. A fast op that previously took only scalars would now always materialize a
scope. We should benchmark; if the overhead is meaningful we either (a) gate the
check behind a "permissions are partitioned" flag so the default single-policy
case stays on the cheap path, or (b) move to design B.

### 4d. The pragmatic escape hatch: exclude permission ops from fast calls

Section 4b is only a risk if a permission check must run inside a fast call. But
the ops that actually need permission checks (filesystem open, network connect,
ffi, env, run) are I/O or capability-granting ops. Almost all of them are async
or otherwise already on the slow path; the genuinely hot fast-call ops
(arithmetic-like ops, buffer length, `op_void_fast`) never touch permissions. So
in practice the set "ops that need a per-module check" and the set "ops that
must be fast calls" barely intersect.

That suggests a clean policy: when the per-module permission system is enabled,
permission-sensitive ops are not exposed as fast calls. Concretely, register the
slow-only op decl for those ops when the feature flag is on (or simply never
mark them `fast`, since they are not perf-critical). With per-module permissions
off, which is the default, nothing changes and the existing fast paths remain.

This retires the section 4b uncertainty for the common case: we no longer need
`GetCurrentHostDefinedOptions()` to work inside a fast call, because the
permission-bearing ops run on the slow path where the lookup is known-good. It
costs a per-call slow path only for ops that are already slow, and only when the
feature is enabled. This is the recommended default, with design B reserved for
any future op that is both hot and permission-bearing.

## 5. Execution contexts: timers, async, eval, and new Function

"The currently executing module" is well defined for ordinary synchronous calls,
but JavaScript has several ways to detach execution from its lexical origin.
This section works through each and how attribution is preserved (or lost).

There are two distinct mechanisms in play, and it helps to name them:

- Definition-site identity: the script a running function was compiled from,
  read via `GetCurrentHostDefinedOptions()`. V8 carries this on the function for
  free.
- Ambient (causal) identity: a "current module" token stored in V8's
  Continuation-Preserved Embedder Data (CPED) and propagated across async
  boundaries. Deno already binds this as `getContinuationPreservedEmbedderData`
  / `setContinuationPreservedEmbedderData` (`libs/core/01_core.js`), aliased to
  `getAsyncContext` / `setAsyncContext`, and uses it for AsyncContext.

The default semantic in this proposal is definition-site. The ambient token is
the fallback for cases where there is no usable script origin.

### 5a. setTimeout / setInterval / queueMicrotask: already covered

A timer or microtask callback is just a function, and that function carries the
script it was compiled from. When the callback runs, its frame's script origin
is its own module, so `GetCurrentHostDefinedOptions()` returns the right module
with no extra work. Definition-site attribution is automatic here.

If we instead want causal attribution (the module that scheduled the timer,
which may differ from the module that defined the callback), the machinery is
also already present: `ext/web/02_timers.js` snapshots the async context at
schedule time (`const asyncContext = getAsyncContext()`) and restores it around
the callback (`setAsyncContext(asyncContext)` inside the wrapped callback). If
the per-module token rides in CPED alongside the async context, this existing
snapshot/restore propagates it across the timer boundary for free. This is the
"implicit arg captured at schedule time" idea, and it is already implemented for
async context; we would only be adding a field to what is snapshotted.

### 5b. async / await and promise chains: covered by CPED

V8 propagates CPED across promise continuations, so the ambient token survives
`await`, `.then()`, and microtask resumption automatically. Separately, the
definition-site lookup also still works after an await, because the resumed
frame belongs to the async function's module. So both mechanisms hold across
async; no new work is needed beyond storing the module token in CPED if we want
the ambient path.

### 5c. eval: likely covered, must verify

For direct `eval(code)`, V8 compiles the code with the calling function's script
context and is expected to propagate the caller's host-defined options to the
eval'd script, so eval'd code inherits the enclosing module's identity. This
must be verified against the installed V8 (a one-line spike: `eval` inside a
stamped module, then read the current options from an op). Indirect eval
(`(0, eval)(s)`) compiles in the global scope and does not inherit, behaving
like new Function below.

### 5d. new Function and indirect eval: the real gap

`new Function(body)` and indirect eval produce a script compiled in the global /
API context with no (or default) host-defined options. The resulting function
carries no module identity, so definition-site attribution returns the default
policy. This is a genuine hole: code could wrap its sensitive calls in
`new Function` to shed its module identity.

Three options, not mutually exclusive:

1. Ambient fallback. When the current frame has no usable host-defined options,
   fall back to the CPED ambient token, which reflects the module whose logical
   context generated the code. This closes the hole for code that runs in the
   same async context as its creator, which is the common case.
2. Stamp at generation. Intercept dynamic code generation and attach the calling
   module's id to the new script. V8 exposes
   `SetModifyCodeGenerationFromStringsCallback` for eval / new Function; today
   it gates allow/deny, so using it to inject host-defined options needs
   investigation (and possibly a small V8 / rusty_v8 addition).
3. Deny by default. Treat origin-less dynamically generated code as having the
   most restrictive policy unless explicitly allowed. This pairs naturally with
   a CSP-style "no dynamic code" hardening posture and is the safe default;
   option 1 or 2 can relax it where needed.

For the prototype, option 3 (deny / most-restrictive by default) is the safest
starting point, with option 1 (ambient fallback) as the first relaxation since
the CPED plumbing already exists.

## 6. Two candidate designs

### Design A: ambient lookup via current script (recommended, see 4d)

- Stamp `module_id` into `host_defined_options[1]` at instantiation.
- Permission-sensitive ops take `&mut OpState` and a scope, call
  `get_current_host_defined_options()`, decode `module_id`, look up the table.
  When the origin is missing (section 5d), fall back to the CPED ambient token.
- Permission-sensitive ops run on the slow path while the feature is enabled
  (section 4d), so the lookup is always in a context where it is known-good. No
  dependence on the section 4b fast-call behavior.

Pros: minimal new infrastructure, reuses the existing host-defined-options path,
single shared op table, no V8 fast-call risk given 4d. Cons: a per-call scope
build and table lookup on those (already slow) ops; "current frame" semantics
(section 2); needs the section 5d handling for origin-less code.

### Design B: capability token baked into per-module op bindings (fallback, fast-call-safe by construction)

Instead of asking "who is calling me?" at runtime, give each module its own view
of the ops where the module id (or a direct capability handle) is part of the
op's bound `data`.

Mechanics: today every op's fast-call `data` external points to a single shared
`OpCtx` per realm, so it cannot distinguish modules. To make identity available
without any stack/context introspection, each module would import an ops
namespace whose op functions carry a per-module `OpCtx` (or an
`(OpCtx,
module_id)` pair) in their `data`. The fast op then reads `module_id`
straight from `FastApiCallbackOptions.data`, with no scope and no V8 frame
introspection. This is the formal version of "pass the id directly to the op".

Pros: guaranteed to work in fast calls, no dependence on V8 internals, the id
read is a pointer deref. Cons: significantly heavier. It requires per-module op
tables (more allocation per module, larger snapshots or per-module setup), and a
way to deliver the per-module ops object to module scope (the ESM
`import { ... } from "ext:core/ops"` channel is currently shared). This trends
toward a compartment/realm-per-policy model.

### Design B-lite: partition by policy group, not by module

A middle ground: most deployments will have a small number of policy groups (for
example "app code" vs "third-party deps"), not a distinct policy per module.
Assign each module to a policy-group id and bind one ops namespace per group.
This bounds the number of per-binding op tables to the number of groups (often 2
to 3) instead of the number of modules, making design B affordable. The
npm-vs-userland split that `host_defined_options` already encodes is exactly a
2-group partition and a natural first target.

## 7. Recommended prototype plan

Phase 0 (cheap verification spikes, run in parallel): (a) confirm direct `eval`
inherits the enclosing module's host-defined options (section 5c); (b)
optionally check whether `GetCurrentHostDefinedOptions()` returns the correct
script inside an `op2(fast)` call (section 4b). With the section 4d exclusion,
(b) is no longer on the critical path; it only matters if we later want a hot
permission-bearing fast op.

Phase 1 (slow-path end to end): wire the full feature on the slow path.

- Add `module_id` assignment and write it to `host_defined_options[1]` in the
  CLI `get_host_defined_options` hooks.
- Add a `module_id -> PermissionSet` table to `OpState`.
- Pick one op (for example `op_fs_open` / file read), force it to the slow path
  under the feature flag (section 4d), and add a per-module check in front of
  the existing process-wide check (deny is the intersection).
- Handle origin-less code per section 5d: start with deny / most-restrictive by
  default, with the CPED ambient token as the fallback once the token is stored.
- Add a policy source (start with a hardcoded or JSON manifest; integrate with
  import maps later).
- Spec test: two modules, one allowed to read, one denied; plus an `eval` /
  `new Function` case asserting the section 5d behavior.

Phase 2 (async + ambient token): store the per-module token in CPED so it
propagates across async and timers (sections 5a, 5b), and wire the section 5d
fallback to read it. This reuses the existing AsyncContext snapshot/restore in
`ext/web/02_timers.js`.

Phase 3: broaden op coverage, define the policy/manifest format, handle the CJS
and `node:vm` paths (section 8), and decide the default behavior when a module
has no entry in the table (fail-open to process perms, or fail-closed). If a hot
permission-bearing op ever appears, evaluate design B-lite for it.

## 8. Open questions and risks

- **CJS / require.** The host-defined-options write path described here is the
  ESM instantiation path. CJS modules loaded through `01_require.js` are
  compiled via a different route and may not carry a module id. Per-module
  permissions for `require`d code needs a matching stamp on that path, or those
  modules inherit a default policy.
- **`node:vm`.** Scripts compiled by `node:vm` already use
  `host_defined_options` index 0/1 for dynamic-import gating. The module-id
  encoding must not collide with those markers. The `eval` / `new Function`
  handling is covered in section 5 (c, d).
- **Confused deputy.** Because the check is on the executing frame, a privileged
  module can be tricked into doing work on behalf of an unprivileged caller, and
  an unprivileged helper called by privileged code will be denied even though
  the intent was privileged. We must document the model clearly: a capability is
  about which code runs the syscall, not which code requested it. Wrappers that
  intentionally re-expose a capability are an explicit, auditable pattern.
- **Async ops.** The permission decision must be captured at op-call time
  (synchronously, while the right script is current), not when an async op later
  resolves on the event loop, where there is no meaningful "current module".
- **Snapshot / startup code.** Internal `ext:` modules and bootstrap run before
  any user policy exists and must be treated as fully privileged.
- **Spoofing.** Module ids live in V8-managed `host_defined_options`, not
  reachable or forgeable from JS, which is the right trust boundary. We must
  ensure no op or API lets user JS set its own host-defined options for an
  arbitrary id.
- **Default policy / fail mode.** Modules with no table entry need a defined
  behavior. Fail-closed is safer but breaks code incrementally adopting the
  feature; a per-run default policy is probably the pragmatic choice.

## 9. Summary

V8's `host_defined_options` plus `GetCurrentHostDefinedOptions()` gives us an
allocation-free way to identify the currently executing module, and we already
use it for managed globals. Building per-module permissions on top is
straightforward on the slow op path. The fast-call uncertainty (section 4b)
mostly dissolves once we observe that permission-bearing ops are I/O ops that
can run on the slow path; section 4d makes that an explicit policy (exclude
permission ops from fast calls while the feature is enabled), so the lookup only
ever runs where it is known-good.

Detached execution contexts are largely handled by mechanisms already in the
tree. Timer and async callbacks carry their own script origin, so
definition-site attribution is automatic; and if causal attribution is wanted,
the per-module token can ride in CPED, which V8 propagates across async and
which `ext/web/02_timers.js` already snapshots and restores across timers.
Direct `eval` should inherit the enclosing module (verify), while `new Function`
and indirect eval are the real gap (origin-less code) and are handled by a
deny-by-default policy with a CPED ambient-token fallback. Design B (capability
token baked into per-module or per-policy-group op bindings) remains the
fast-call-safe escape hatch if a hot permission-bearing op ever appears.
Recommended path: run the small phase-0 spikes, build the slow path end to end,
then layer the CPED token for async and origin-less code.

## 10. Update after prototype: op-level checks need a stack walk

A prototype (branch `feat/per-module-permissions`) surfaced a correction to the
central mechanism. `GetCurrentHostDefinedOptions()` reflects the script of the
**top** JS frame. For an op, the top frame is never user code: it is always the
internal `ext:` wrapper that calls the op (for example `readTextFileSync` in
`ext/fs/30_fs.js` calling `op_fs_read_file_text_sync`). That wrapper script has
no host-defined options, so the lookup returns empty and the user module is
invisible.

In other words, host-defined options + `GetCurrentHostDefinedOptions()` work for
**global property interceptors** (managed globals), where user code is genuinely
the top frame when it touches `process`/`Buffer`. They do **not** work for
**op-level permission checks**, where an `ext:` frame always sits on top.

The fix is the same shape as `op_node_call_is_from_dependency`: walk the current
V8 stack and take the first user frame (skipping `ext:` and `node:` frames),
then map that frame's script name to the module/package and look up the policy.
This is the `op_current_user_call_site` "first user frame" semantic, not the
"current frame" semantic of section 2. The prototype does exactly this in
`ext/fs/ops.rs` (`first_user_script_name` + `check_per_module_read`) and it
correctly denies reads from a targeted module while allowing others, all under a
process-wide `--allow-read`.

Consequences:

- The module-id-in-host-defined-options optimization is moot for the op path. We
  identify the executing module by the first user frame's **script name**, which
  we already have, and map name to policy directly. Host-defined options may
  still be useful for the managed-globals-style fast path, but not here.
- Cost: the check now captures a small stack trace per permission-bearing op.
  Section 4d still applies (these ops are slow-path anyway). The stack walk is
  bounded (first user frame, small frame limit) and only runs when the feature
  is enabled.
- The async / timer / eval analysis in section 5 still holds, because a stack
  walk observes whatever frames are actually executing: inside a timer or async
  continuation the first user frame is the callback's own module, and inside
  `eval`/`new Function` it is the generated script (origin-less, so it falls
  through to the default policy, matching section 5d).

#### Async ops have no live scope: capture the caller at dispatch

An async op runs its body off the event loop, where there is no valid v8
`HandleScope`, so it cannot walk the stack itself. But argument conversion
(`FromV8::from_v8`) runs **synchronously at op dispatch**, while the user frame
is still on the stack and a scope is available. The prototype exploits this with
a `PathWithCaller` argument type (`ext/fs/ops.rs`): its `from_v8` reads the path
string and, in the same breath, captures `first_user_script_name(scope)`. The
async body then enforces the policy against that captured caller, with no scope
of its own. To avoid paying for the stack walk when the feature is off, the
conversion first checks `PerModulePermissions::enabled()` via
`JsRuntime::op_state_from(scope)`. This closes the otherwise-glaring bypass
where `await Deno.readTextFile(...)` would evade a check that only covered the
sync path; `op_fs_read_file_async` and `op_fs_read_file_text_async` are both
gated this way, and the `per_package_read` spec test exercises both.

## 11. Per-package permissions (config)

The unit users most want to govern is the **package**, not the individual file.
"Let `npm:express` open a listening socket on `:8080`, and give some other
package (say `npm:left-pad`) nothing" is a per-package statement. Per-file is
the mechanism; per-package is the policy users author.

### 11a. Config shape

Extend the existing `permissions` object in `deno.json` with a `perPackage` map
from package identifier to a permission set:

```jsonc
{
  "permissions": {
    // process-wide baseline / ceiling (existing semantics)
    "net": true,
    "perPackage": {
      "npm:express": { "net": [":8080"] },
      "npm:left-pad": {}
    }
  }
}
```

Semantics:

- A package listed with a permission set is granted exactly those capabilities.
- A package listed with `{}` is granted nothing (the `left-pad` case).
- The process-wide `permissions` is the ceiling: a per-package grant is
  intersected with it, so `perPackage` can only ever **narrow**, never escalate.
  (Whether unlisted packages default to "inherit process" or "deny all" is a
  policy knob; deny-all is the safer default and matches the intent of locking
  down dependencies.)
- First-party code (the app's own modules, not resolved inside a package) is not
  a package and is governed by the process-wide permissions as today.

### 11b. Package identity

- npm: `npm:<name>` (for example `npm:express`), version-agnostic to start; a
  later refinement can key on `npm:<name>@<range>`.
- jsr: `jsr:@scope/name`.
- Resolution maps an executing module's specifier to its package id. For npm the
  specifier resolves inside a `node_modules/<name>` (or
  `node_modules/@scope/name`) path; for jsr it is a `jsr:`/registry URL. The npm
  resolver already knows the owning package and is the robust source; a
  path-based fallback (`.../node_modules/<pkg>/...`) covers the common case.

### 11c. Enforcement

Same machinery as section 10: at a permission-bearing op, walk to the first user
frame, resolve its script name to a package id, and consult the per-package set.
Read/write are checked at the fs ops; net at the net ops (`connect`/`listen`)
against the package's allowed net descriptors, intersected with the process net
grant. The decision can be precomputed per module specifier at instantiation
time (when the package is already known) so the op-time step is a map lookup
keyed by the first user frame's script name.

### 11d. Prototype status

The branch implements the runtime enforcement path end to end for both **read**
and **net**: a `PerModulePermissions` table in `OpState`, populated from a
policy source, with `ext/fs` read ops and `ext/net` `op_net_listen_tcp` denying
disallowed packages. The shared op-side mechanism is
`deno_core::first_user_script_name` (the stack walk from section 10) plus
`package_id_of` and the per-package lookup in `deno_permissions`. Spec tests
(`tests/specs/permission/per_package_read`, `per_package_net`) cover read
allow/deny and net allow-list matching (including per-port and per-package
scoping), all under a permissive process-wide grant.

Async read ops (`op_fs_read_file_async`, `op_fs_read_file_text_async`) are gated
too, via the dispatch-time caller capture described in section 10, so
`await Deno.readTextFile(...)` is denied for an unprivileged package just like
the sync path.

Remaining: the policy source is still a stand-in (the
`DENO_PER_PACKAGE_PERMISSIONS` environment variable) rather than `deno.json`;
wiring `permissions.perPackage` through config parsing into the worker,
package-identity resolution via the npm/jsr resolver (currently a
`node_modules/<pkg>` path heuristic), the async net path (`connect`) and write
ops via the same caller-capture mechanism, and the remaining capabilities
(write, env, ffi, run) are the next steps.

## 12. Related work: LavaMoat

[LavaMoat](https://github.com/LavaMoat/LavaMoat) (notably `lavamoat-node`) is
the closest existing system. It targets the same threat (a malicious or
compromised dependency) and is also per-package, resolving package identity from
the module graph. The instructive difference is the **enforcement layer**, which
drives every other difference.

| Dimension         | LavaMoat (lavamoat-node)                                                                                            | This proposal                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Enforcement layer | JS-level: SES Compartments                                                                                          | Rust op boundary (the syscall kernel)                                        |
| Mechanism         | Each package runs in a compartment with a curated `globalThis` + module map; forbidden capabilities are unreachable | Capability is reachable, but the op checks the calling package and denies    |
| Trust base        | SES hardening (frozen intrinsics) inside the JS engine                                                              | V8/Rust boundary, below JS                                                   |
| Granularity       | Module + global ("can this package import `fs`/`net`? see `process.env`?")                                          | Resource/parameter (`net=:8080` vs `:9090`, specific read paths)             |
| Caller identity   | Structural: the compartment is the identity                                                                         | Inferred: stack walk to the first user frame                                 |
| Scope             | Full environment virtualization; also isolates packages from each other                                             | I/O capabilities only                                                        |
| Policy            | Auto-generated `policy.json` (static analysis) over globals/builtins/packages                                       | Hand-authored `permissions.perPackage`, reusing Deno's permission vocabulary |
| Maturity          | Production (MetaMask)                                                                                               | Prototype                                                                    |

Where each is stronger:

- **Trust base (ours wins).** LavaMoat's boundary is enforced by JavaScript
  semantics plus frozen primordials: a large surface that must defend against
  `Function('return this')()`, prototype-pollution escapes, getters on shared
  objects, and packages that monkeypatch intrinsics SES froze. In our model,
  even if a package tampers with the JS environment or uses `new Function`, it
  still has to call an op to touch disk or network, and the Rust kernel checks
  the caller. JS-level escapes do not bypass it; we never have to freeze the
  world.
- **Granularity (ours wins).** LavaMoat decides whether a package may reference
  the `net` module at all. We decide which host:port it may bind or connect to,
  because we plug into Deno's permission descriptors. That is exactly the
  `express -> :8080` case (granted `:8080`, denied `:7070`).
- **Scope (LavaMoat wins, and this is its real advantage).** LavaMoat
  virtualizes the whole environment, so it also isolates packages _from each
  other_ within JS: a dependency cannot prototype-pollute `Array.prototype` to
  attack another, cannot read another package's module-internal state, and
  cannot import `child_process` or even other packages it was not granted
  (lateral-movement control). Op-gating does none of this; it only governs
  capabilities crossing the kernel.
- **Attribution (LavaMoat wins).** Because each package is its own compartment,
  "who is calling" is structural: no stack walk, no confused-deputy ambiguity,
  no `eval`/`new Function` origin gap (sections 5, 8, 10). LavaMoat sidesteps
  those entirely; the price is the heavyweight compartment + SES machinery and
  the packages SES breaks.

Bottom line: the two are largely **complementary, not competing**. If the goal
is "stop a dependency from exfiltrating files or secrets over the network," our
kernel-enforced, resource-granular model is a stronger guarantee and nearly free
to add (Deno already has the permission kernel). If the goal additionally
includes "stop dependencies from tampering with each other inside the VM," that
is SES/Compartment territory, which op-gating does not address. Section 13 asks
whether Deno could add that half too.

## 13. Could Deno implement the LavaMoat (SES/compartment) model too?

Yes, and Deno is arguably a better host for it than Node, because the riskiest
half (I/O capability enforcement) comes from the kernel rather than from JS. The
SES/compartment half is feasible but carries real friction. The LavaMoat model
decomposes into three separable pieces, and Deno can adopt them à la carte:

1. **Tamper-proofing** — freeze the shared intrinsics so one package cannot
   prototype-pollute another (`lockdown()` in SES).
2. **Per-package global curation** — each package sees only the globals its
   policy grants (a Compartment with a curated `globalThis`).
3. **Module-graph control** — a package can only import the packages and
   builtins it is allowed (the compartment's module map).

LavaMoat gets all three from SES (the `ses` shim) plus a loader hook. None of it
is Node-specific; SES is engine-agnostic JS that already runs on V8, so it runs
in Deno today. LavaMoat is structured as "SES + a loader" with loaders for
node/browserify/webpack; a lavamoat-deno loader is a conceivable fourth.

### 13a. Implementation paths

- **Path A — port the SES/Compartment model at the JS layer.** Run `lockdown()`
  during bootstrap, then route package module loading through Compartments with
  per-package globals from policy. Most faithful port. The integration point is
  Deno's module loader: each package's (already transpiled) source is evaluated
  inside its compartment instead of as a native V8 module under the real global.
- **Path B — transpile-time scope injection (lighter; mirrors LavaMoat's bundler
  loaders).** Deno already transpiles every module. Wrap each module body in a
  function that receives a curated `globalThis`/`require`, so the package has no
  lexical reference to ambient authority it was not granted. No full Compartment
  runtime; reuses Deno's existing transpile + load pipeline. Most Deno-native
  and least invasive starting point.
- **Path C — native realm primitives.** V8 has `ShadowRealm` (separate global
  and separate intrinsics per realm) and deno_core already has multi-realm
  support (`JsRealm`/`CreateRealmOptions`, plus `node:vm` contexts). A
  ShadowRealm-per-package gives isolation without `lockdown()` at all, since
  each package has its own `Array.prototype`. The catch is the boundary:
  ShadowRealm only lets primitives and callables cross, not objects, which
  breaks normal package interop (you cannot pass a config object between two
  packages). This is exactly why LavaMoat chose shared-frozen-intrinsics
  compartments over separate realms; module graphs need to share objects freely.

### 13b. The hard parts

- **`lockdown()` vs Deno's runtime and node compat.** Freezing intrinsics breaks
  any code that mutates prototypes after lockdown. Deno's own `ext:` internals
  are tractable (lock down after bootstrap), but the node-compat layer is large
  and monkeypatches aggressively, which is the real compatibility minefield and
  bigger than in lavamoat-node. Path C sidesteps this (nothing shared to
  freeze).
- **Module-loader integration.** Deno compiles ESM as native V8 modules against
  the real global. Putting a package under a curated global means either virtual
  module sources (Path A, feeding npm/jsr resolution into the compartment) or
  wrapping at transpile time (Path B).
- **Curating Deno's own globals.** The ambient authority object is `Deno` (and
  `fetch`, etc.). Important synergy: many of those globals already bottom out in
  gated ops, so even a package that keeps a reference is still I/O-checked by
  the kernel. Global curation then mostly needs to strip non-op ambient state
  and the `Deno` namespace surface.

### 13c. The pragmatic Deno-native design: hybrid

The strongest design is not "port LavaMoat" but to **combine the two halves**:

- **SES `lockdown()`** (or ShadowRealm) for intrinsic tamper-proofing and
  package-from-package isolation, the thing the op-boundary model does not
  address.
- **The op-boundary per-package permissions** from this proposal for I/O:
  kernel-enforced, resource-granular, and not defeatable by JS escapes, which is
  strictly better than SES endowment attenuation for the I/O half.
- **Per-package global curation** to remove ambient authority objects.

Bonus: if packages ran in compartments or realms, op-side attribution gets
cleaner. Instead of the stack walk (`first_user_script_name`), an op could read
the current V8 context's identity and map it to a package, which also closes the
`eval`/`new Function` origin gap because generated code runs in its package's
compartment. The two approaches reinforce each other.

Honest assessment: the I/O half (this proposal) is cheap and strong in Deno. The
intrinsic-isolation half is doable but the cost is node-compat compatibility
with `lockdown()` (Path A/B) or broken object interop (Path C). That trade-off,
not technical possibility, is the decision point.

### 13d. Suggested spike

The most informative small step is **Path B on this branch**: take the existing
per-package policy and, for one denied package, wrap its module at load time so
it has no lexical `Deno` reference, proving the global-curation path composes
with the op-boundary enforcement already built. Alternatively, a
`lockdown()`-after- bootstrap spike would measure how badly node-compat breaks.

## 14. Compartments: a detailed design

The compartment model is the direction of interest, so this section goes deeper
on what it would mean concretely in Deno.

### 14a. Which kind of compartment

Two families, with a sharp compatibility difference:

- **SES-style compartments (shared realm).** One V8 context, one set of
  intrinsics (frozen by `lockdown()`), and each package evaluated against a
  curated `globalThis` synthesized by scope virtualization. Objects pass freely
  between packages and `instanceof` works, because intrinsics are shared. This
  is what LavaMoat uses.
- **Separate realms (V8 `Context` / `ShadowRealm`).** Each package gets its own
  global _and its own intrinsics_. True isolation with no `lockdown()` needed,
  but cross-realm object identity breaks: an `Array`/`Error`/`Buffer` created in
  one package is not `instanceof` another package's `Array`, and `ShadowRealm`
  additionally forbids passing objects at all.

Recommendation: **SES-style, shared realm.** npm and node packages routinely
pass objects across package boundaries (buffers, streams, errors, options bags)
and rely on `instanceof`. Separate realms would break a large fraction of the
ecosystem. The price of the shared-realm choice is that security rests on
`lockdown()` and that a compartment is not, by itself, a V8-queryable boundary
at the op layer (see 14b).

### 14b. Key insight: capability-bound globals make attribution structural

A compartment already has to be handed a curated `globalThis`. That is exactly
the hook that removes the need for the section 10 stack walk.

Give each package's compartment a `Deno` namespace (and `fetch`, etc.) whose
methods are **bound to that package's permission token**. Then the op reads the
token from its own bound op `data` (the "Design B" token-in-`OpCtx` idea from
section 6), with no stack walk and no `GetCurrentHostDefinedOptions`.
Attribution becomes structural: the capability object a package holds _is_ its
identity, and it is impossible to confuse one package's authority for another
because they hold different bound objects.

This unifies the whole proposal:

- compartment curated globals → tamper isolation (with `lockdown()`) and removal
  of ambient authority;
- capability-bound globals → clean, fast-call-safe attribution;
- the op-boundary permission check (already built) → the actual enforcement, now
  keyed by the bound token instead of the stack walk.

So compartments do not replace the op-boundary work; they make its attribution
exact and retire the stack walk, the confused-deputy ambiguity (section 8), and
the `eval`/`new Function` origin gap (section 5d), because generated code runs
in its package's compartment and inherits its bound capabilities.

### 14c. Architecture in Deno

1. `lockdown()` at the end of bootstrap to freeze intrinsics (the SES half).
2. First-party app code runs in the root realm with full authority. Only
   packages (resolved via `package_id_of` / the npm-jsr resolver) get
   compartments.
3. For each package (or policy group), build a compartment whose `globalThis`
   exposes exactly the policy-granted globals, with capability objects (a `Deno`
   subset, `fetch`, ...) bound to the package token.
4. Evaluate package modules inside their compartment. Two integration options:
   - **Path B, transpile-time scope injection.** Deno already transpiles every
     module; wrap each package module body in a function that receives the
     compartment `globalThis`. Reuses the native load pipeline minus the ambient
     global. Easiest for live ESM and the recommended first step.
   - **Path A, full SES module map.** Route package loading through the
     Compartment import hooks with `StaticModuleRecord`s, feeding npm/jsr
     resolution into the compartment. More faithful, more invasive.

### 14d. The compatibility crux

- `lockdown()` vs the node-compat layer, which monkeypatches intrinsics
  aggressively. This is the main risk and must be measured before committing.
- The shared-intrinsics requirement rules out separate V8 contexts, so globals
  must be virtualized (scope injection or SES), not swapped via `v8::Context`.
- Capability-bound globals require materializing a per-package `Deno`-like
  object; the cost is per package (or per policy group, which bounds it to a
  handful).

### 14e. Phased plan

1. **Scope injection (Path B).** Per-package curated `globalThis` (strip
   `Deno`), composed with the op-boundary I/O enforcement already on this
   branch. No `lockdown()` yet. Proves global curation works for live ESM.
2. **Capability-bound globals.** Give the compartment a package-bound `Deno`;
   switch op attribution from the stack walk to the bound token. Retires the
   section 10 mechanism for compartmentalized packages.
3. **`lockdown()`.** Add intrinsic freezing; triage node-compat breakage. This
   is the go/no-go gate for the full SES guarantee.
4. **Policy-driven import graph + globals (Path A).** Full per-package control
   of which builtins/packages a package may import, matching LavaMoat's scope.

### 14f. Suggested first spike

On this branch: for a package denied by policy, evaluate its module wrapped in a
function whose `globalThis` omits `Deno` (Path B, phase 1). Verify that the
package cannot even name `Deno`, while a granted package still works, and that
this composes with the existing op-boundary read/net denial. That single step
validates the compartment integration point in Deno's loader without taking on
`lockdown()` or the full SES runtime yet.
