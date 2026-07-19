// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.

(function () {
const { core, primordials } = __bootstrap;
const { Array } = primordials;

const { validateFunction } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { _exiting } = core.loadExtScript("ext:deno_node/_process/exiting.ts");
const {
  emitAfter,
  emitBefore,
  emitDestroy,
  emitInit,
  executionAsyncId,
  newAsyncId: nextAsyncId,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");

const {
  getAsyncContext,
} = core;

let nextTickEnabled = false;
function enableNextTick() {
  nextTickEnabled = true;

  // TODO(bartlomieju): ideally this should not be needed
  // and async hook implementation would live in core
  // Register the async hook emit functions directly with core.
  // The core drain loop calls these inline per-tick -- no indirection.
  core.setAsyncHooksEmit(emitBefore, emitAfter, emitDestroy);
}

// Re-export from core for consumers (e.g. timers.mjs)
const processTicksAndRejections = core.processTicksAndRejections;
const runNextTicks = core.runNextTicks;

// `nextTick()` will not enqueue any callback when the process is about to
// exit since the callback would not have a chance to be executed.
function nextTick(this: unknown, callback: () => void): void;
function nextTick<T extends Array<unknown>>(
  this: unknown,
  callback: (...args: T) => void,
  ...args: T
): void;
function nextTick(
  this: unknown,
  callback: (...args: unknown[]) => void,
) {
  // If we're snapshotting we don't want to push nextTick to be run. We'll
  // enable next ticks in "__bootstrapNodeProcess()";
  if (!nextTickEnabled) {
    return;
  }

  validateFunction(callback, "callback");

  if (_exiting) {
    return;
  }

  // Use `arguments` instead of a rest parameter so the common
  // `nextTick(callback)` case allocates no array (matches Node).
  let args_;
  switch (arguments.length) {
    case 1:
      break;
    case 2:
      args_ = [arguments[1]];
      break;
    case 3:
      args_ = [arguments[1], arguments[2]];
      break;
    case 4:
      args_ = [arguments[1], arguments[2], arguments[3]];
      break;
    default:
      args_ = new Array(arguments.length - 1);
      for (let i = 1; i < arguments.length; i++) {
        args_[i - 1] = arguments[i];
      }
  }

  const asyncId = nextAsyncId();
  const triggerAsyncId = executionAsyncId();
  const tickObject = {
    asyncId,
    triggerAsyncId,
    snapshot: getAsyncContext(),
    callback,
    args: args_,
  };
  emitInit(asyncId, "TickObject", triggerAsyncId, tickObject);
  if (!core.hasTickScheduled()) {
    core.setHasTickScheduled(true);
  }
  core.queueNextTick(tickObject);
}

return {
  enableNextTick,
  nextTick,
  processTicksAndRejections,
  runNextTicks,
};
})();
