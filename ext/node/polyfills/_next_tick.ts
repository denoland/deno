// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";

import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { _exiting } from "ext:deno_node/_process/exiting.ts";
import {
  emitAfter,
  emitBefore,
  emitDestroy,
  emitInit,
  executionAsyncId,
  newAsyncId as nextAsyncId,
} from "ext:deno_node/internal/async_hooks.ts";

const {
  getAsyncContext,
} = core;

let nextTickEnabled = false;
export function enableNextTick() {
  nextTickEnabled = true;

  // TODO(bartlomieju): ideally this should not be needed
  // and async hook implementation would live in core
  // Register the async hook emit functions directly with core.
  // The core drain loop calls these inline per-tick -- no indirection.
  core.setAsyncHooksEmit(emitBefore, emitAfter, emitDestroy);
}

// Re-export from core for consumers (e.g. timers.mjs)
export const processTicksAndRejections = core.processTicksAndRejections;
export const runNextTicks = core.runNextTicks;

// `nextTick()` will not enqueue any callback when the process is about to
// exit since the callback would not have a chance to be executed.
export function nextTick(this: unknown, callback: () => void): void;
export function nextTick<T extends Array<unknown>>(
  this: unknown,
  callback: (...args: T) => void,
  ...args: T
): void;
export function nextTick<T extends Array<unknown>>(
  this: unknown,
  callback: (...args: T) => void,
  ...args: T
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

  // TODO(bartlomieju): seems superfluous if we don't depend on `arguments`
  let args_;
  switch (args.length) {
    case 0:
      break;
    case 1:
      args_ = [args[0]];
      break;
    case 2:
      args_ = [args[0], args[1]];
      break;
    case 3:
      args_ = [args[0], args[1], args[2]];
      break;
    default:
      args_ = new Array(args.length);
      for (let i = 0; i < args.length; i++) {
        args_[i] = args[i];
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
