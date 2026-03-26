// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  createTimer: createTimer_,
  cancelTimer: cancelTimer_,
  refreshTimer: refreshTimer_,
  refTimer: refTimer_,
  unrefTimer: unrefTimer_,
  getAsyncContext,
  setAsyncContext,
  immediateRefCount,
} = core;
const {
  FunctionPrototypeCall,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  NumberIsFinite,
  ObjectDefineProperty,
  ReflectApply,
  SafeArrayIterator,
  SafeMap,
  Symbol,
  SymbolToPrimitive,
} = primordials;
import {
  emitAfter,
  emitBefore,
  emitDestroy,
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

  const asyncId = nextAsyncId();
  const triggerAsyncId = executionAsyncId();
  this._asyncId = asyncId;
  this._triggerAsyncId = triggerAsyncId;
  this._asyncDestroyed = false;

  this[kTimerId] = this[createTimer]();

  emitInit(asyncId, "Timeout", triggerAsyncId, this);
}

Timeout.prototype[createTimer] = function () {
  const self = this;
  const callback = this._onTimeout;
  const asyncContext = getAsyncContext();
  const asyncId = this._asyncId;
  const triggerAsyncId = this._triggerAsyncId;
  const cb = function () {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      emitBefore(asyncId, triggerAsyncId, self);
      if (!self._isRepeat) {
        MapPrototypeDelete(activeTimers, self[kTimerId]);
      }
      const args = self._timerArgs;
      let ret;
      if (args !== undefined && args.length > 0) {
        ret = ReflectApply(callback, self, args);
      } else {
        ret = FunctionPrototypeCall(callback, self);
      }
      // Only emit after/destroy on success. On error, the domain's
      // uncaught exception handler manages the stack cleanup.
      emitAfter(asyncId);
      if (!self._isRepeat && !self._asyncDestroyed) {
        self._asyncDestroyed = true;
        emitDestroy(asyncId);
      }
      return ret;
    } finally {
      setAsyncContext(oldContext);
    }
  };
  const timer = createTimer_(
    cb,
    this._idleTimeout,
    undefined,
    this._isRepeat,
    this[kRefed],
  );
  ObjectDefineProperty(this, "_timer", {
    __proto__: null,
    value: timer,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  const id = timer._timerId;
  MapPrototypeSet(activeTimers, id, this);
  return id;
};

Timeout.prototype[kDestroy] = function () {
  if (!this._destroyed) {
    this._destroyed = true;
    cancelTimer_(this._timer);
    MapPrototypeDelete(activeTimers, this[kTimerId]);
    if (this._asyncId !== undefined && !this._asyncDestroyed) {
      this._asyncDestroyed = true;
      emitDestroy(this._asyncId);
    }
  }
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
    refreshTimer_(this._timer);
  }
  return this;
};

Timeout.prototype.unref = function () {
  if (this[kRefed]) {
    this[kRefed] = false;
    if (!this._destroyed) {
      unrefTimer_(this._timer);
    }
  }
  return this;
};

Timeout.prototype.ref = function () {
  if (!this[kRefed]) {
    this[kRefed] = true;
    if (!this._destroyed) {
      refTimer_(this._timer);
    }
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
