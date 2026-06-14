// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core, primordials } = __bootstrap;
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
  ArrayPrototypePush,
  DateNow,
  FunctionPrototypeCall,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  NumberIsFinite,
  NumberIsNaN,
  ObjectDefineProperty,
  ReflectApply,
  SafeArrayIterator,
  SafeMapIterator,
  SafeMap,
  Symbol,
  SymbolDispose,
  SymbolToPrimitive,
} = primordials;
const {
  emitAfter,
  emitAfterNoPush,
  emitBefore,
  emitDestroy,
  emitInit,
  enabledHooksExist,
  executionAsyncId,
  newAsyncId: nextAsyncId,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");
const { inspect } = core.loadExtScript(
  "ext:deno_node/internal/util/inspect.mjs",
);
const {
  validateFunction,
  validateNumber,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { ERR_OUT_OF_RANGE } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const lazyProcess = core.createLazyLoader("node:process");

// Timeout values > TIMEOUT_MAX are set to 1.
const TIMEOUT_MAX = 2 ** 31 - 1;

const kDestroy = Symbol("destroy");
const kTimerId = Symbol("timerId");
const kTimeout = Symbol("timeout");
const kSuspended = Symbol("suspended");
const kRefed = core.kRefed;
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
function getActiveTimer(id) {
  return MapPrototypeGet(activeTimers, id);
}

function getActiveResourcesInfo() {
  const resources = [];
  for (const { 1: timeout } of new SafeMapIterator(activeTimers)) {
    if (timeout[kRefed]) {
      ArrayPrototypePush(resources, "Timeout");
    }
  }
  return resources;
}

let warnedNegativeNumber = false;
let warnedNotNumber = false;

// Timer constructor function.
function Timeout(callback, after, args, isRepeat, isRefed) {
  if (after === undefined) {
    after = 1;
  } else {
    after *= 1; // Coalesce to number or NaN
  }

  if (!(after >= 1 && after <= TIMEOUT_MAX)) {
    if (after > TIMEOUT_MAX) {
      lazyProcess().default.emitWarning(
        `${after} does not fit into a 32-bit signed integer.` +
          "\nTimeout duration was set to 1.",
        "TimeoutOverflowWarning",
      );
    } else if (after < 0 && !warnedNegativeNumber) {
      warnedNegativeNumber = true;
      lazyProcess().default.emitWarning(
        `${after} is a negative number.` +
          "\nTimeout duration was set to 1.",
        "TimeoutNegativeWarning",
      );
    } else if (NumberIsNaN(after) && !warnedNotNumber) {
      warnedNotNumber = true;
      lazyProcess().default.emitWarning(
        `${after} is not a number.` +
          "\nTimeout duration was set to 1.",
        "TimeoutNaNWarning",
      );
    }
    after = 1;
  }
  this._idleTimeout = after;
  this._idleStart = DateNow();
  this._idlePrev = null;
  this._idleNext = null;
  this._onTimeout = callback;
  this._timerArgs = args;
  this._repeat = isRepeat;
  this._destroyed = false;
  ObjectDefineProperty(this, kSuspended, {
    __proto__: null,
    value: false,
    writable: true,
  });
  this[kRefed] = isRefed;

  const asyncId = nextAsyncId();
  const triggerAsyncId = executionAsyncId();
  this._asyncId = asyncId;
  this._triggerAsyncId = triggerAsyncId;
  this._asyncDestroyed = false;

  this[kTimerId] = this[createTimer]();

  // Match node: only emit async_hooks init if there are live hooks.
  // emitInit does non-trivial work (try/finally, empty-array loop,
  // lookupPublicResource) that's pure overhead in the common case.
  if (enabledHooksExist()) {
    emitInit(asyncId, "Timeout", triggerAsyncId, this);
  }
}

Timeout.prototype[createTimer] = function () {
  const self = this;
  const callback = this._onTimeout;
  const asyncContext = getAsyncContext();
  const asyncId = this._asyncId;
  const triggerAsyncId = this._triggerAsyncId;
  // Whether async_hooks were enabled when this timer was *created*. We still
  // re-check `enabledHooksExist()` at fire time below, because a hook can be
  // enabled (or disabled) between creation and the callback running -- e.g.
  // test-async-hooks-enable-before-promise-resolve.js enables a hook from
  // inside the very setTimeout callback and expects its trailing `after`.
  let cb;
  function invokeCallback() {
    const wasRepeat = self._repeat;
    if (!wasRepeat) {
      MapPrototypeDelete(activeTimers, self[kTimerId]);
    } else {
      const currentCb = self._onTimeout;
      if (currentCb === null) {
        self[kDestroy]();
        return;
      }
    }
    const currentCb = wasRepeat ? self._onTimeout : callback;
    const args = self._timerArgs;
    let ret;
    if (args !== undefined && args.length > 0) {
      ret = ReflectApply(currentCb, self, args);
    } else {
      ret = FunctionPrototypeCall(currentCb, self);
    }
    if (wasRepeat) {
      if (self._idleTimeout < 0 || self._onTimeout === null) {
        self[kDestroy]();
      }
    } else if (self._repeat) {
      // timeout was converted to interval inside callback
      self[kTimerId] = self[createTimer]();
    } else {
      self._destroyed = true;
    }
    return ret;
  }
  cb = function () {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      // Decide at fire time, not creation time: a hook may have been enabled
      // or disabled since this timer was scheduled.
      const beforeEmitted = enabledHooksExist();
      if (beforeEmitted) {
        emitBefore(asyncId, triggerAsyncId, self);
      }
      const ret = invokeCallback();
      // Only emit after/destroy on success. On error, the domain's uncaught
      // exception handler manages the stack cleanup.
      if (beforeEmitted) {
        // We pushed onto the executionAsyncId stack via emitBefore, so we must
        // always pop via emitAfter to keep it balanced -- even if the callback
        // disabled every hook in the meantime.
        emitAfter(asyncId);
      } else if (enabledHooksExist()) {
        // A hook was enabled from inside the callback. `before` never ran, so
        // nothing was pushed; deliver the trailing `after` without popping.
        emitAfterNoPush(asyncId);
      }
      if (
        (beforeEmitted || enabledHooksExist()) &&
        !self._repeat && !self._asyncDestroyed
      ) {
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
    this._repeat,
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
  if (!this._destroyed || this[kSuspended]) {
    this._destroyed = true;
    this[kSuspended] = false;
    this._idleTimeout = -1;
    this._idleStart = DateNow();
    this._onTimeout = null;
    cancelTimer_(this._timer);
    MapPrototypeDelete(activeTimers, this[kTimerId]);
    if (
      this._asyncId !== undefined &&
      !this._asyncDestroyed &&
      enabledHooksExist()
    ) {
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
  if (this._destroyed) {
    // Reactivate a timer that fired naturally (callback still set).
    // Do NOT reactivate a timer cancelled via clearTimeout (callback
    // nulled by kDestroy or _onTimeout explicitly cleared).
    if (this._onTimeout !== null) {
      this._destroyed = false;
      this[kSuspended] = false;
      this[kTimerId] = this[createTimer]();
    }
  } else {
    refreshTimer_(this._timer);
  }
  this._idleStart = DateNow();
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

Timeout.prototype.close = function () {
  this[kDestroy]();
  return this;
};

Timeout.prototype[SymbolDispose] = function () {
  this[kDestroy]();
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
function getTimerDuration(msecs, name) {
  validateNumber(msecs, name);

  if (msecs < 0 || !NumberIsFinite(msecs)) {
    throw new ERR_OUT_OF_RANGE(name, "a non-negative finite number", msecs);
  }

  // Ensure that msecs fits into signed int32
  if (msecs > TIMEOUT_MAX) {
    lazyProcess().default.emitWarning(
      `${msecs} does not fit into a 32-bit signed integer.` +
        `\nTimer duration was truncated to ${TIMEOUT_MAX}.`,
      "TimeoutOverflowWarning",
    );

    return TIMEOUT_MAX;
  }

  return msecs;
}

function setUnrefTimeout(callback, timeout, ...args) {
  validateFunction(callback, "callback");
  return new Timeout(callback, timeout, args, false, false);
}

function suspendTimeout(timeout) {
  if (timeout !== null && timeout !== undefined && !timeout._destroyed) {
    timeout._destroyed = true;
    timeout[kSuspended] = true;
    timeout._idleStart = DateNow();
    cancelTimer_(timeout._timer);
    MapPrototypeDelete(activeTimers, timeout[kTimerId]);
  }
}

// Re-export immediate queue and runImmediates from core for consumers
const immediateQueue = core.immediateQueue;
const runImmediates = core.runImmediates;

class Immediate {
  constructor(unboundCallback, ...args) {
    const asyncContext = getAsyncContext();
    // Match Node's `immediate._onImmediate(...argv)` invocation: the callback's
    // `this` is the Immediate instance, not the global.
    const self = this;
    const callback = (...argv) => {
      const oldContext = getAsyncContext();
      try {
        setAsyncContext(asyncContext);
        return ReflectApply(unboundCallback, self, argv);
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

  [SymbolDispose]() {
    core.clearImmediate(this);
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

return {
  TIMEOUT_MAX,
  kDestroy,
  kTimerId,
  kTimeout,
  kRefed,
  getActiveTimer,
  getActiveResourcesInfo,
  Timeout,
  getTimerDuration,
  setUnrefTimeout,
  suspendTimeout,
  immediateQueue,
  runImmediates,
  Immediate,
};
})();
