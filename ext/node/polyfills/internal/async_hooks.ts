// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core, primordials } = __bootstrap;
// deno-lint-ignore camelcase
const async_wrap = core.loadExtScript(
  "ext:deno_node/internal_binding/async_wrap.ts",
);
const { ERR_ASYNC_CALLBACK } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { asyncIdSymbol, ownerSymbol } = core.loadExtScript(
  "ext:deno_node/internal_binding/symbols.ts",
);
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypePop,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  FunctionPrototypeApply,
  ObjectKeys,
  SafeWeakMap,
  SafeWeakSet,
  Symbol,
} = primordials;
const { isPromiseHooksSuppressed } = core;
const {
  AsyncVariable,
  getAsyncContext,
  kNoAsyncContextRestore,
  setAsyncContext,
} = core;

interface ActiveHooks {
  array: AsyncHook[];
  // deno-lint-ignore camelcase
  call_depth: number;
  // deno-lint-ignore camelcase
  tmp_array: AsyncHook[] | null;
  // deno-lint-ignore camelcase
  tmp_fields: number[] | null;
}

// Properties in active_hooks are used to keep track of the set of hooks being
// executed in case another hook is enabled/disabled. The new set of hooks is
// then restored once the active set of hooks is finished executing.
// deno-lint-ignore camelcase
const active_hooks: ActiveHooks = {
  // Array of all AsyncHooks that will be iterated whenever an async event
  // fires. Using var instead of (preferably const) in order to assign
  // active_hooks.tmp_array if a hook is enabled/disabled during hook
  // execution.
  array: [],
  // Use a counter to track nested calls of async hook callbacks and make sure
  // the active_hooks.array isn't altered mid execution.
  // deno-lint-ignore camelcase
  call_depth: 0,
  // Use to temporarily store and updated active_hooks.array if the user
  // enables or disables a hook while hooks are being processed. If a hook is
  // enabled() or disabled() during hook execution then the current set of
  // active hooks is duplicated and set equal to active_hooks.tmp_array. Any
  // subsequent changes are on the duplicated array. When all hooks have
  // completed executing active_hooks.tmp_array is assigned to
  // active_hooks.array.
  // deno-lint-ignore camelcase
  tmp_array: null,
  // Keep track of the field counts held in active_hooks.tmp_array. Because the
  // async_hook_fields can't be reassigned, store each uint32 in an array that
  // is written back to async_hook_fields when active_hooks.array is restored.
  // deno-lint-ignore camelcase
  tmp_fields: null,
};

const registerDestroyHook = async_wrap.registerDestroyHook;
const {
  async_hook_fields,
  // deno-lint-ignore camelcase
  asyncIdFields: async_id_fields,
  newAsyncId,
  constants,
} = async_wrap;

// In Node.js the top-level execution async ID is 1 (kRootAsyncId). The trigger
// async ID at top level is 0 (no parent). Several Node compat tests assert
// this; e.g. test-async-hooks-promise-triggerid.js expects the first promise
// init to receive triggerId === 1.
const kRootAsyncId = 1;

// Parallel stacks for executionAsyncId() and triggerAsyncId(). They are pushed
// together by emitBefore() and popped together by emitAfter(), keeping them
// in sync for the lifetime of a single async callback.
const executionAsyncIdStack: number[] = [kRootAsyncId];
const triggerAsyncIdStack: number[] = [0];

function executionAsyncId(): number {
  return executionAsyncIdStack[executionAsyncIdStack.length - 1] || 0;
}

function triggerAsyncId(): number {
  return triggerAsyncIdStack[triggerAsyncIdStack.length - 1] || 0;
}

// Per-async-context "current resource" tracked via the AsyncVariable
// machinery (V8 ContinuationPreservedEmbedderData). This propagates across
// promises and await transitions automatically. The top-level resource is a
// shared singleton used before any specific resource has been entered.
// deno-lint-ignore no-explicit-any
const topLevelResource: any = { __proto__: null };
// deno-lint-ignore no-explicit-any
const executionResourceVariable: any = new AsyncVariable();

// deno-lint-ignore no-explicit-any
function executionAsyncResource(): any {
  const r = executionResourceVariable.get();
  return r === undefined ? topLevelResource : r;
}

// Enter a new "current resource" scope. The returned value is the previous
// async context snapshot that must be restored by exitAsyncResource.
// deno-lint-ignore no-explicit-any
function enterAsyncResource(resource: any): any {
  return executionResourceVariable.enter(resource);
}

// deno-lint-ignore no-explicit-any
function exitAsyncResource(previousContext: any): void {
  setAsyncContext(previousContext);
}

// deno-lint-ignore no-explicit-any
function enterAsyncResourceIfActive(resource: any): any {
  if (active_hooks.array.length > 0) {
    return executionResourceVariable.enter(resource);
  }
  return executionResourceVariable.enterIfActive(resource);
}

// deno-lint-ignore no-explicit-any
function exitAsyncResourceIfActive(previousContext: any): void {
  if (previousContext !== kNoAsyncContextRestore) {
    setAsyncContext(previousContext);
    return;
  }

  const currentContext = getAsyncContext();
  if (
    currentContext !== null &&
    currentContext !== undefined &&
    ObjectKeys(currentContext).length > 0
  ) {
    setAsyncContext(undefined);
  }
}

// Emit functions that work with the internal hook system
function emitBefore(asyncId: number, triggerAsyncId?: number): void {
  ArrayPrototypePush(executionAsyncIdStack, asyncId);
  ArrayPrototypePush(
    triggerAsyncIdStack,
    triggerAsyncId === undefined ? 0 : triggerAsyncId,
  );

  // Call hooks if they exist
  const hooks = active_hooks.array;
  try {
    for (let i = 0; i < hooks.length; i++) {
      const hook = hooks[i];
      if (hook[before_symbol]) {
        hook[before_symbol](asyncId);
      }
    }
  } catch (e) {
    // Clean up stack corruption on hook errors (Node.js pattern)
    if (executionAsyncIdStack.length > 1) {
      ArrayPrototypePop(executionAsyncIdStack);
      ArrayPrototypePop(triggerAsyncIdStack);
    }
    throw e;
  }
}

function emitAfter(asyncId: number): void {
  // Call hooks if they exist
  const hooks = active_hooks.array;
  try {
    for (let i = 0; i < hooks.length; i++) {
      const hook = hooks[i];
      if (hook[after_symbol]) {
        hook[after_symbol](asyncId);
      }
    }
  } finally {
    // Always pop stack even if hooks throw (Node.js pattern)
    if (executionAsyncIdStack.length > 1) {
      ArrayPrototypePop(executionAsyncIdStack);
      ArrayPrototypePop(triggerAsyncIdStack);
    }
  }
}

function emitDestroy(asyncId: number): void {
  // Call hooks if they exist
  const hooks = active_hooks.array;
  for (let i = 0; i < hooks.length; i++) {
    const hook = hooks[i];
    if (hook[destroy_symbol]) {
      hook[destroy_symbol](asyncId);
    }
  }
}

function emitPromiseResolve(asyncId: number): void {
  const hooks = active_hooks.array;
  for (let i = 0; i < hooks.length; i++) {
    const hook = hooks[i];
    if (hook[promise_resolve_symbol]) {
      hook[promise_resolve_symbol](asyncId);
    }
  }
}
const {
  kInit,
  kBefore,
  kAfter,
  kDestroy,
  kPromiseResolve,
  kTotals,
  kCheck,
  kDefaultTriggerAsyncId,
  kStackLength,
} = constants;

// deno-lint-ignore camelcase
const resource_symbol = Symbol("resource");
// Alias to the same symbol used by `internal_binding/symbols.ts` so that
// `socket[asyncIdSymbol]` (set in net.ts/dgram.ts) and
// `socket[require('internal/async_hooks').symbols.async_id_symbol]`
// (read by Node test fixtures) refer to the same slot on objects.
// deno-lint-ignore camelcase
const async_id_symbol = asyncIdSymbol;
// deno-lint-ignore camelcase
const trigger_async_id_symbol = Symbol("trigger_async_id");
// deno-lint-ignore camelcase
const init_symbol = Symbol("init");
// deno-lint-ignore camelcase
const before_symbol = Symbol("before");
// deno-lint-ignore camelcase
const after_symbol = Symbol("after");
// deno-lint-ignore camelcase
const destroy_symbol = Symbol("destroy");
// deno-lint-ignore camelcase
const promise_resolve_symbol = Symbol("promiseResolve");

const symbols = {
  // deno-lint-ignore camelcase
  async_id_symbol,
  // deno-lint-ignore camelcase
  trigger_async_id_symbol,
  // deno-lint-ignore camelcase
  init_symbol,
  // deno-lint-ignore camelcase
  before_symbol,
  // deno-lint-ignore camelcase
  after_symbol,
  // deno-lint-ignore camelcase
  destroy_symbol,
  // deno-lint-ignore camelcase
  promise_resolve_symbol,
};

// deno-lint-ignore no-explicit-any
function lookupPublicResource(resource: any) {
  if (typeof resource !== "object" || resource === null) return resource;
  // TODO(addaleax): Merge this with owner_symbol and use it across all
  // AsyncWrap instances.
  const publicResource = resource[resource_symbol];
  if (publicResource !== undefined) {
    return publicResource;
  }
  return resource;
}

// Used by C++ to call all init() callbacks. Because some state can be setup
// from C++ there's no need to perform all the same operations as in
// emitInitScript.
function emitInitNative(
  asyncId: number,
  // deno-lint-ignore no-explicit-any
  type: any,
  triggerAsyncId: number,
  // deno-lint-ignore no-explicit-any
  resource: any,
) {
  active_hooks.call_depth += 1;
  resource = lookupPublicResource(resource);
  // Use a single try/catch for all hooks to avoid setting up one per iteration.
  try {
    for (let i = 0; i < active_hooks.array.length; i++) {
      if (typeof active_hooks.array[i][init_symbol] === "function") {
        active_hooks.array[i][init_symbol](
          asyncId,
          type,
          triggerAsyncId,
          resource,
        );
      }
    }
  } catch (e) {
    throw e;
  } finally {
    active_hooks.call_depth -= 1;
  }

  // Hooks can only be restored if there have been no recursive hook calls.
  // Also the active hooks do not need to be restored if enable()/disable()
  // weren't called during hook execution, in which case active_hooks.tmp_array
  // will be null.
  if (active_hooks.call_depth === 0 && active_hooks.tmp_array !== null) {
    restoreActiveHooks();
  }
}

function getHookArrays(): [AsyncHook[], number[] | Uint32Array] {
  if (active_hooks.call_depth === 0) {
    return [active_hooks.array, async_hook_fields];
  }
  // If this hook is being enabled while in the middle of processing the array
  // of currently active hooks then duplicate the current set of active hooks
  // and store this there. This shouldn't fire until the next time hooks are
  // processed.
  if (active_hooks.tmp_array === null) {
    storeActiveHooks();
  }
  return [active_hooks.tmp_array!, active_hooks.tmp_fields!];
}

function storeActiveHooks() {
  active_hooks.tmp_array = ArrayPrototypeSlice(active_hooks.array);
  // Don't want to make the assumption that kInit to kDestroy are indexes 0 to
  // 4. So do this the long way.
  active_hooks.tmp_fields = [];
  copyHooks(active_hooks.tmp_fields, async_hook_fields);
}

function copyHooks(
  destination: number[] | Uint32Array,
  source: number[] | Uint32Array,
) {
  destination[kInit] = source[kInit];
  destination[kBefore] = source[kBefore];
  destination[kAfter] = source[kAfter];
  destination[kDestroy] = source[kDestroy];
  destination[kPromiseResolve] = source[kPromiseResolve];
}

// Then restore the correct hooks array in case any hooks were added/removed
// during hook callback execution.
function restoreActiveHooks() {
  active_hooks.array = active_hooks.tmp_array!;
  copyHooks(async_hook_fields, active_hooks.tmp_fields!);

  active_hooks.tmp_array = null;
  active_hooks.tmp_fields = null;
}

// ---------------------------------------------------------------------------
// Promise hook integration
//
// V8 exposes four promise hooks (init, before, after, resolve). Once any
// AsyncHook with init/before/after/promiseResolve is enabled we install our
// own V8 promise hooks; from there we assign an async id to each promise on
// first observation, track the parent->child relationship for triggerAsyncId,
// and fan out to the user's createHook() callbacks.
// ---------------------------------------------------------------------------

// Map promise -> { asyncId, triggerAsyncId }
const promiseInfo = new SafeWeakMap();

// Promises created while `core.isPromiseHooksSuppressed()` was true. We track
// them so that subsequent before/after/resolve V8 hook callbacks know to
// skip them as well.
const suppressedPromises = new SafeWeakSet();

// We deliberately do NOT register promises with a FinalizationRegistry to fire
// `destroy()`. Doing so would queue one cleanup callback per Promise in V8's
// finalizer queue and noticeably delay unrelated FinalizationRegistry callbacks
// (notably `AsyncResource`'s destroy), causing GC-timing-sensitive Node tests
// such as `test-zlib-invalid-input-memory.js` and `test-net-connect-memleak.js`
// to fail. Promise `destroy` events are best-effort in Node too; user code that
// needs to know when a promise is collected should use a FinalizationRegistry
// directly. Init/before/after/promiseResolve are still wired up below.

let promiseHooksInstalled = false;

// TODO(@divy-work): `core.setPromiseHooks` is additive-only -- deno_core has no
// API to remove the V8 promise hooks once installed. So disabling every
// async_hook leaves these four callbacks resident for the rest of the process
// lifetime. The `kTotals === 0` fast path in promiseInitHook keeps the residual
// per-promise cost minimal until a removal API exists.
function ensurePromiseHooks() {
  if (promiseHooksInstalled) return;
  promiseHooksInstalled = true;
  core.setPromiseHooks(
    promiseInitHook,
    promiseBeforeHook,
    promiseAfterHook,
    promiseResolveHook,
  );
}

// Assign a fresh async id pair to a promise, recording the parent->child
// relationship. Returns the assigned id.
function trackPromise(
  // deno-lint-ignore no-explicit-any
  promise: any,
  // deno-lint-ignore no-explicit-any
  parent: any,
): { asyncId: number; triggerAsyncId: number } {
  const asyncId = newAsyncId();
  let trigger;
  if (parent != null && promiseInfo.has(parent)) {
    trigger = promiseInfo.get(parent).asyncId;
  } else {
    trigger = executionAsyncId();
  }
  const info = { asyncId, triggerAsyncId: trigger };
  promiseInfo.set(promise, info);
  return info;
}

// deno-lint-ignore no-explicit-any
function promiseInitHook(promise: any, parent: any): void {
  if (isPromiseHooksSuppressed()) {
    // This promise was created by deno_core infrastructure (async-op
    // wrapper, etc.); user code never observes it directly so we skip
    // tracking and firing any of the four async_hooks callbacks for it.
    suppressedPromises.add(promise);
    return;
  }
  // Fast path: no async hook is active (e.g. every hook has been disabled,
  // but the V8 promise hooks stay installed since deno_core has no removal
  // API -- see ensurePromiseHooks). Skip the newAsyncId()/WeakMap.set cost.
  // This stays balanced because promiseBefore/After/ResolveHook backfill via
  // trackPromise(promise, null) if a hook is enabled before this promise
  // settles.
  if (async_hook_fields[kTotals] === 0) {
    return;
  }
  // Always assign an async id pair (so before/after/resolve can resolve it)
  // but only fire user init() callbacks once.
  //
  // NOTE: emitInitNative invokes user init() callbacks synchronously. If such
  // a callback allocates a promise, V8 re-enters this hook reentrantly. The
  // tests enabled here don't exercise that; Node guards against it explicitly.
  const info = trackPromise(promise, parent);

  if (async_hook_fields[kInit] > 0) {
    emitInitNative(info.asyncId, "PROMISE", info.triggerAsyncId, promise);
  }
}

// deno-lint-ignore no-explicit-any
function promiseBeforeHook(promise: any): void {
  if (suppressedPromises.has(promise)) return;
  let info = promiseInfo.get(promise);
  if (info === undefined) {
    // Promise was created before any async_hook was enabled. Backfill an
    // async id pair so before/after stay balanced. Do NOT fire init() for
    // this promise (matches Node's fast-path behavior). See
    // test-async-wrap-promise-after-enabled.js.
    info = trackPromise(promise, null);
  }
  emitBefore(info.asyncId, info.triggerAsyncId);
}

// deno-lint-ignore no-explicit-any
function promiseAfterHook(promise: any): void {
  if (suppressedPromises.has(promise)) return;
  const info = promiseInfo.get(promise);
  if (info !== undefined) {
    emitAfter(info.asyncId);
  }
}

// deno-lint-ignore no-explicit-any
function promiseResolveHook(promise: any): void {
  if (suppressedPromises.has(promise)) return;
  const info = promiseInfo.get(promise);
  // Only fire promiseResolve for promises we actually tracked (observed at
  // init or before time). A promise that reaches resolve while still untracked
  // is deno_core infrastructure -- e.g. the module-evaluation result promise,
  // which is created before the user's hook is installed and resolves while it
  // is active. Node never surfaces those, so we must not backfill+emit here or
  // user `promiseResolve` callbacks see a spurious extra resolve.
  if (info !== undefined && async_hook_fields[kPromiseResolve] > 0) {
    emitPromiseResolve(info.asyncId);
  }
}

function enableHooks() {
  async_hook_fields[kCheck] += 1;
}

function disableHooks() {
  async_hook_fields[kCheck] -= 1;
}

// Return the triggerAsyncId meant for the constructor calling it. It's up to
// the user to safeguard this call and make sure it's zero'd out when the
// constructor is complete.
function getDefaultTriggerAsyncId() {
  const defaultTriggerAsyncId =
    async_id_fields[async_wrap.UidFields.kDefaultTriggerAsyncId];
  // If defaultTriggerAsyncId isn't set, use the executionAsyncId
  if (defaultTriggerAsyncId < 0) {
    return executionAsyncId();
  }
  return defaultTriggerAsyncId;
}

function defaultTriggerAsyncIdScope(
  triggerAsyncId: number | undefined,
  // deno-lint-ignore no-explicit-any
  block: (...arg: any[]) => void,
  ...args: unknown[]
) {
  if (triggerAsyncId === undefined) {
    return FunctionPrototypeApply(block, null, args);
  }
  // CHECK(NumberIsSafeInteger(triggerAsyncId))
  // CHECK(triggerAsyncId > 0)
  const oldDefaultTriggerAsyncId = async_id_fields[kDefaultTriggerAsyncId];
  async_id_fields[kDefaultTriggerAsyncId] = triggerAsyncId;

  try {
    return FunctionPrototypeApply(block, null, args);
  } finally {
    async_id_fields[kDefaultTriggerAsyncId] = oldDefaultTriggerAsyncId;
  }
}

function hasHooks(key: number) {
  return async_hook_fields[key] > 0;
}

function enabledHooksExist() {
  return active_hooks.array.length > 0;
}

function hasAsyncIdStack() {
  return hasHooks(kStackLength);
}

type Fn = (...args: unknown[]) => unknown;

class AsyncHook {
  [init_symbol]: Fn;
  [before_symbol]: Fn;
  [after_symbol]: Fn;
  [destroy_symbol]: Fn;
  [promise_resolve_symbol]: Fn;

  constructor({
    init,
    before,
    after,
    destroy,
    promiseResolve,
  }: {
    init: Fn;
    before: Fn;
    after: Fn;
    destroy: Fn;
    promiseResolve: Fn;
  }) {
    if (init !== undefined && typeof init !== "function") {
      throw new ERR_ASYNC_CALLBACK("hook.init");
    }
    if (before !== undefined && typeof before !== "function") {
      throw new ERR_ASYNC_CALLBACK("hook.before");
    }
    if (after !== undefined && typeof after !== "function") {
      throw new ERR_ASYNC_CALLBACK("hook.after");
    }
    if (destroy !== undefined && typeof destroy !== "function") {
      throw new ERR_ASYNC_CALLBACK("hook.destroy");
    }
    if (promiseResolve !== undefined && typeof promiseResolve !== "function") {
      throw new ERR_ASYNC_CALLBACK("hook.promiseResolve");
    }

    this[init_symbol] = init;
    this[before_symbol] = before;
    this[after_symbol] = after;
    this[destroy_symbol] = destroy;
    this[promise_resolve_symbol] = promiseResolve;
  }

  enable() {
    // The set of callbacks for a hook should be the same regardless of whether
    // enable()/disable() are run during their execution. The following
    // references are reassigned to the tmp arrays if a hook is currently being
    // processed.
    // deno-lint-ignore camelcase
    const { 0: hooks_array, 1: hook_fields } = getHookArrays();

    // Each hook is only allowed to be added once.
    if (ArrayPrototypeIncludes(hooks_array, this)) {
      return this;
    }

    // deno-lint-ignore camelcase
    const prev_kTotals = hook_fields[kTotals];

    // createHook() has already enforced that the callbacks are all functions,
    // so here simply increment the count of whether each callbacks exists or
    // not.
    hook_fields[kTotals] = hook_fields[kInit] += +!!this[init_symbol];
    hook_fields[kTotals] += hook_fields[kBefore] += +!!this[before_symbol];
    hook_fields[kTotals] += hook_fields[kAfter] += +!!this[after_symbol];
    hook_fields[kTotals] += hook_fields[kDestroy] += +!!this[destroy_symbol];
    hook_fields[kTotals] += hook_fields[kPromiseResolve] +=
      +!!this[promise_resolve_symbol];
    ArrayPrototypePush(hooks_array, this);

    if (prev_kTotals === 0 && hook_fields[kTotals] > 0) {
      enableHooks();
    }

    // Install V8 promise hooks lazily, the first time any hook needs them.
    // This handles init/before/after/promiseResolve for PROMISE async ids.
    ensurePromiseHooks();

    return this;
  }

  disable() {
    // deno-lint-ignore camelcase
    const { 0: hooks_array, 1: hook_fields } = getHookArrays();

    const index = ArrayPrototypeIndexOf(hooks_array, this);
    if (index === -1) {
      return this;
    }

    // deno-lint-ignore camelcase
    const prev_kTotals = hook_fields[kTotals];

    hook_fields[kTotals] = hook_fields[kInit] -= +!!this[init_symbol];
    hook_fields[kTotals] += hook_fields[kBefore] -= +!!this[before_symbol];
    hook_fields[kTotals] += hook_fields[kAfter] -= +!!this[after_symbol];
    hook_fields[kTotals] += hook_fields[kDestroy] -= +!!this[destroy_symbol];
    hook_fields[kTotals] += hook_fields[kPromiseResolve] -=
      +!!this[promise_resolve_symbol];
    ArrayPrototypeSplice(hooks_array, index, 1);

    if (prev_kTotals > 0 && hook_fields[kTotals] === 0) {
      disableHooks();
    }

    return this;
  }
}

return {
  asyncIdSymbol,
  ownerSymbol,
  newAsyncId,
  emitInit: emitInitNative,
  constants,
  executionAsyncId,
  triggerAsyncId,
  executionAsyncResource,
  enterAsyncResource,
  exitAsyncResource,
  enterAsyncResourceIfActive,
  exitAsyncResourceIfActive,
  emitBefore,
  emitAfter,
  emitDestroy,
  emitPromiseResolve,
  getDefaultTriggerAsyncId,
  defaultTriggerAsyncIdScope,
  enabledHooksExist,
  hasAsyncIdStack,
  AsyncHook,
  registerDestroyHook,
  async_id_symbol,
  trigger_async_id_symbol,
  init_symbol,
  before_symbol,
  after_symbol,
  destroy_symbol,
  promise_resolve_symbol,
  symbols,
};
})();
