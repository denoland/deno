// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  getAsyncContext,
  setAsyncContext,
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
  op_immediate_count,
  op_immediate_ref_count,
  op_immediate_set_has_outstanding,
} from "ext:core/ops";
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
import { runNextTicks } from "ext:deno_node/_next_tick.ts";

// Timeout values > TIMEOUT_MAX are set to 1.
export const TIMEOUT_MAX = 2 ** 31 - 1;

export const kDestroy = Symbol("destroy");
export const kTimerId = Symbol("timerId");
export const kTimeout = Symbol("timeout");
export const kRefed = Symbol("refed");
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

// This code was forked from Node.js
// Copyright Node.js contributors. All rights reserved.
//
// A linked list for storing `setImmediate()` requests
class ImmediateList {
  constructor() {
    this.head = null;
    this.tail = null;
  }

  // Appends an item to the end of the linked list, adjusting the current tail's
  // next pointer and the item's previous pointer where applicable
  append(item) {
    // console.log("append", this.tail);
    if (this.tail !== null) {
      this.tail._idleNext = item;
      item._idlePrev = this.tail;
    } else {
      this.head = item;
    }
    this.tail = item;
  }

  // Removes an item from the linked list, adjusting the pointers of adjacent
  // items and the linked list's head or tail pointers as necessary
  remove(item) {
    if (item._idleNext) {
      item._idleNext._idlePrev = item._idlePrev;
    }

    if (item._idlePrev) {
      item._idlePrev._idleNext = item._idleNext;
    }

    if (item === this.head) {
      this.head = item._idleNext;
    }
    if (item === this.tail) {
      this.tail = item._idlePrev;
    }

    item._idleNext = null;
    item._idlePrev = null;
  }
}

// Create a single linked list instance only once at startup
export const immediateQueue = new ImmediateList();
// If an uncaught exception was thrown during execution of immediateQueue,
// this queue will store all remaining Immediates that need to run upon
// resolution of all error handling (if process is still alive).
const outstandingQueue = new ImmediateList();

export function runImmediates() {
  const queue = outstandingQueue.head !== null
    ? outstandingQueue
    : immediateQueue;
  let immediate = queue.head;
  // Clear the linked list early in case new `setImmediate()`
  // calls occur while immediate callbacks are executed
  if (queue !== outstandingQueue) {
    queue.head = queue.tail = null;
    op_immediate_set_has_outstanding(true);
  }

  let prevImmediate;
  let ranAtLeastOneImmediate = false;
  while (immediate !== null) {
    if (ranAtLeastOneImmediate) {
      runNextTicks();
    } else {
      ranAtLeastOneImmediate = true;
    }

    // It's possible for this current Immediate to be cleared while executing
    // the next tick queue above, which means we need to use the previous
    // Immediate's _idleNext which is guaranteed to not have been cleared.
    if (immediate._destroyed) {
      outstandingQueue.head = immediate = prevImmediate._idleNext;
      continue;
    }

    immediate._destroyed = true;

    op_immediate_count(false);
    if (immediate[kRefed]) {
      op_immediate_ref_count(false);
    }
    immediate[kRefed] = null;

    prevImmediate = immediate;

    // TODO:
    // const priorContextFrame = AsyncContextFrame.exchange(
    // immediate[async_context_frame],
    // );

    // TODO:
    // const asyncId = immediate[async_id_symbol];
    // emitBefore(asyncId, immediate[trigger_async_id_symbol], immediate);

    try {
      const argv = immediate._argv;
      if (!argv) {
        immediate._onImmediate();
      } else {
        immediate._onImmediate(...new SafeArrayIterator(argv));
      }
    } finally {
      immediate._onImmediate = null;

      // TODO:
      // if (destroyHooksExist()) {
      // emitDestroy(asyncId);
      // }

      outstandingQueue.head = immediate = immediate._idleNext;
    }
    // emitAfter(asyncId);

    // TODO:
    // AsyncContextFrame.set(priorContextFrame);
  }

  if (queue === outstandingQueue) {
    outstandingQueue.head = null;
  }

  op_immediate_set_has_outstanding(false);
}

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

    // TODO:
    // initAsyncResource(this, "Immediate");

    this.ref();
    op_immediate_count(true);
    immediateQueue.append(this);
  }

  ref() {
    if (this[kRefed] === false) {
      this[kRefed] = true;
      op_immediate_ref_count(true);
    }
    return this;
  }

  unref() {
    if (this[kRefed] === true) {
      this[kRefed] = false;
      op_immediate_ref_count(false);
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
