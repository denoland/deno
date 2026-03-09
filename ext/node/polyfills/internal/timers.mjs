// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  getAsyncContext,
  setAsyncContext,
  immediateRefCount,
} = core;
const {
  FunctionPrototypeBind,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  NumberIsFinite,
  ReflectApply,
  SafeArrayIterator,
  SafeMap,
  Symbol,
  SymbolToPrimitive,
} = primordials;
import {
  emitInit,
  executionAsyncId,
  newAsyncId as nextAsyncId,
} from "ext:deno_node/internal/async_hooks.ts";
import { inspect } from "ext:deno_node/internal/util/inspect.mjs";
import {
  validateFunction,
  validateNumber,
} from "ext:deno_node/internal/validators.mjs";
import { ERR_OUT_OF_RANGE } from "ext:deno_node/internal/errors.ts";
import { emitWarning } from "node:process";
import {
  clearTimeout as clearTimeout_,
  setInterval as setInterval_,
  setTimeout as setTimeout_,
} from "ext:deno_web/02_timers.js";

// Timeout values > TIMEOUT_MAX are set to 1.
export const TIMEOUT_MAX = 2 ** 31 - 1;

export const kDestroy = Symbol("destroy");
export const kTimerId = Symbol("timerId");
export const kTimeout = Symbol("timeout");
export const kRefed = core.kRefed;
const createTimer = Symbol("createTimer");

/**
 * The keys in this map correspond to the key ID's in the spec's map of active
 * timers. The values are the timeout's status.
 *
 * @type {Map<number, Timeout>}
 */
const activeTimers = new SafeMap();

/**
 * @param {number} id
 * @returns {Timeout | undefined}
 */
export function getActiveTimer(id) {
  return MapPrototypeGet(activeTimers, id);
}

// Timer constructor function.
export function Timeout(callback, after, args, isRepeat, isRefed) {
  if (typeof after === "number" && after > TIMEOUT_MAX) {
    after = 1;
  }
  this._idleTimeout = after;
  this._onTimeout = callback;
  this._timerArgs = args;
  this._isRepeat = isRepeat;
  this._destroyed = false;
  this[kRefed] = isRefed;
  this[kTimerId] = this[createTimer]();
}

Timeout.prototype[createTimer] = function () {
  const callback = this._onTimeout;
  const cb = (...args) => {
    if (!this._isRepeat) {
      MapPrototypeDelete(activeTimers, this[kTimerId]);
    }
    return FunctionPrototypeBind(callback, this)(
      ...new SafeArrayIterator(args),
    );
  };
  const id = this._isRepeat
    ? setInterval_(
      cb,
      this._idleTimeout,
      ...new SafeArrayIterator(this._timerArgs),
    )
    : setTimeout_(
      cb,
      this._idleTimeout,
      ...new SafeArrayIterator(this._timerArgs),
    );
  if (!this[kRefed]) {
    Deno.unrefTimer(id);
  }
  MapPrototypeSet(activeTimers, id, this);
  return id;
};

Timeout.prototype[kDestroy] = function () {
  this._destroyed = true;
  MapPrototypeDelete(activeTimers, this[kTimerId]);
};

// Make sure the linked list only shows the minimal necessary information.
Timeout.prototype[inspect.custom] = function (_, options) {
  return inspect(this, {
    ...options,
    // Only inspect one level.
    depth: 0,
    // It should not recurse.
    customInspect: false,
  });
};

Timeout.prototype.refresh = function () {
  if (!this._destroyed) {
    clearTimeout_(this[kTimerId]);
    MapPrototypeDelete(activeTimers, this[kTimerId]);
    this[kTimerId] = this[createTimer]();
  }
  return this;
};

Timeout.prototype.unref = function () {
  if (this[kRefed]) {
    this[kRefed] = false;
    Deno.unrefTimer(this[kTimerId]);
  }
  return this;
};

Timeout.prototype.ref = function () {
  if (!this[kRefed]) {
    this[kRefed] = true;
    Deno.refTimer(this[kTimerId]);
  }
  return this;
};

Timeout.prototype.hasRef = function () {
  return this[kRefed];
};

Timeout.prototype[SymbolToPrimitive] = function () {
  return this[kTimerId];
};

/**
 * @param {number} msecs
 * @param {string} name
 * @returns
 */
export function getTimerDuration(msecs, name) {
  validateNumber(msecs, name);

  if (msecs < 0 || !NumberIsFinite(msecs)) {
    throw new ERR_OUT_OF_RANGE(name, "a non-negative finite number", msecs);
  }

  // Ensure that msecs fits into signed int32
  if (msecs > TIMEOUT_MAX) {
    emitWarning(
      `${msecs} does not fit into a 32-bit signed integer.` +
        `\nTimer duration was truncated to ${TIMEOUT_MAX}.`,
      "TimeoutOverflowWarning",
    );

    return TIMEOUT_MAX;
  }

  return msecs;
}

export function setUnrefTimeout(callback, timeout, ...args) {
  validateFunction(callback, "callback");
  return new Timeout(callback, timeout, args, false, false);
}

// Re-export immediate queue and runImmediates from core for consumers
export const immediateQueue = core.immediateQueue;
export const runImmediates = core.runImmediates;

export class Immediate {
  constructor(unboundCallback, ...args) {
    const asyncContext = getAsyncContext();
    const callback = (...argv) => {
      const oldContext = getAsyncContext();
      try {
        setAsyncContext(asyncContext);
        return ReflectApply(unboundCallback, globalThis, argv);
      } finally {
        setAsyncContext(oldContext);
      }
    };

    this._idleNext = null;
    this._idlePrev = null;
    this._onImmediate = callback;
    this._argv = args;
    this._destroyed = false;
    this[kRefed] = false;

    const asyncId = nextAsyncId();
    const triggerAsyncId = executionAsyncId();
    this.asyncId = asyncId;
    this.triggerAsyncId = triggerAsyncId;
    emitInit(asyncId, "Immediate", triggerAsyncId, this);

    this.ref();
    core.queueImmediate(this);
  }

  ref() {
    if (this[kRefed] === false) {
      this[kRefed] = true;
      immediateRefCount(true);
    }
    return this;
  }

  unref() {
    if (this[kRefed] === true) {
      this[kRefed] = false;
      immediateRefCount(false);
    }
    return this;
  }

  hasRef() {
    return !!this[kRefed];
  }

  [inspect.custom] = function (_, options) {
    return inspect(this, {
      ...options,
      // Only inspect one level.
      depth: 0,
      // It should not recurse.
      customInspect: false,
    });
  };
}

export default {
  getTimerDuration,
  kTimerId,
  kTimeout,
  setUnrefTimeout,
  Timeout,
  TIMEOUT_MAX,
};
