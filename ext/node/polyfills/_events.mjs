// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

"use strict";

const kRejection = Symbol.for("nodejs.rejection");

import { inspect } from "ext:deno_node/internal/util/inspect.mjs";
import {
  AbortError,
  // kEnhanceStackBeforeInspector,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_THIS,
  ERR_OUT_OF_RANGE,
  ERR_UNHANDLED_ERROR,
} from "ext:deno_node/internal/errors.ts";

import { AsyncResource } from "node:async_hooks";
import {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { spliceOne } from "ext:deno_node/_utils.ts";
import { nextTick } from "ext:deno_node/_process/process.ts";

export { addAbortListener } from "./internal/events/abort_listener.mjs";

const kCapture = Symbol("kCapture");
const kErrorMonitor = Symbol("events.errorMonitor");
const kMaxEventTargetListeners = Symbol("events.maxEventTargetListeners");
const kMaxEventTargetListenersWarned = Symbol(
  "events.maxEventTargetListenersWarned",
);

let process;
export function setProcess(p) {
  process = p;
}

/**
 * Creates a new `EventEmitter` instance.
 * @param {{ captureRejections?: boolean; }} [opts]
 * @returns {EventEmitter}
 */
export function EventEmitter(opts) {
  EventEmitter.init.call(this, opts);
}
export default EventEmitter;
EventEmitter.on = on;
EventEmitter.once = once;
EventEmitter.getEventListeners = getEventListeners;
EventEmitter.setMaxListeners = setMaxListeners;
EventEmitter.listenerCount = listenerCount;
// Backwards-compat with node 0.10.x
EventEmitter.EventEmitter = EventEmitter;
EventEmitter.usingDomains = false;

EventEmitter.captureRejectionSymbol = kRejection;
export const captureRejectionSymbol = EventEmitter.captureRejectionSymbol;
export const errorMonitor = EventEmitter.errorMonitor;

Object.defineProperty(EventEmitter, "captureRejections", {
  get() {
    return EventEmitter.prototype[kCapture];
  },
  set(value) {
    validateBoolean(value, "EventEmitter.captureRejections");

    EventEmitter.prototype[kCapture] = value;
  },
  enumerable: true,
});

EventEmitter.errorMonitor = kErrorMonitor;

// The default for captureRejections is false
Object.defineProperty(EventEmitter.prototype, kCapture, {
  value: false,
  writable: true,
  enumerable: false,
});

EventEmitter.prototype._events = undefined;
EventEmitter.prototype._eventsCount = 0;
EventEmitter.prototype._maxListeners = undefined;

// By default EventEmitters will print a warning if more than 10 listeners are
// added to it. This is a useful default which helps finding memory leaks.
export let defaultMaxListeners = 10;

function checkListener(listener) {
  validateFunction(listener, "listener");
}

Object.defineProperty(EventEmitter, "defaultMaxListeners", {
  enumerable: true,
  get: function () {
    return defaultMaxListeners;
  },
  set: function (arg) {
    if (typeof arg !== "number" || arg < 0 || Number.isNaN(arg)) {
      throw new ERR_OUT_OF_RANGE(
        "defaultMaxListeners",
        "a non-negative number",
        arg,
      );
    }
    defaultMaxListeners = arg;
  },
});

Object.defineProperties(EventEmitter, {
  kMaxEventTargetListeners: {
    value: kMaxEventTargetListeners,
    enumerable: false,
    configurable: false,
    writable: false,
  },
  kMaxEventTargetListenersWarned: {
    value: kMaxEventTargetListenersWarned,
    enumerable: false,
    configurable: false,
    writable: false,
  },
});

/**
 * Sets the max listeners.
 * @param {number} n
 * @param {EventTarget[] | EventEmitter[]} [eventTargets]
 * @returns {void}
 */
export function setMaxListeners(
  n = defaultMaxListeners,
  ...eventTargets
) {
  if (typeof n !== "number" || n < 0 || Number.isNaN(n)) {
    throw new ERR_OUT_OF_RANGE("n", "a non-negative number", n);
  }
  if (eventTargets.length === 0) {
    defaultMaxListeners = n;
  } else {
    for (let i = 0; i < eventTargets.length; i++) {
      const target = eventTargets[i];
      if (target instanceof EventTarget) {
        target[kMaxEventTargetListeners] = n;
        target[kMaxEventTargetListenersWarned] = false;
      } else if (typeof target.setMaxListeners === "function") {
        target.setMaxListeners(n);
      } else {
        throw new ERR_INVALID_ARG_TYPE(
          "eventTargets",
          ["EventEmitter", "EventTarget"],
          target,
        );
      }
    }
  }
}

EventEmitter.init = function (opts) {
  if (
    this._events === undefined ||
    this._events === Object.getPrototypeOf(this)._events
  ) {
    this._events = Object.create(null);
    this._eventsCount = 0;
  }

  this._maxListeners = this._maxListeners || undefined;

  if (opts?.captureRejections) {
    validateBoolean(opts.captureRejections, "options.captureRejections");
    this[kCapture] = Boolean(opts.captureRejections);
  } else {
    // Assigning the kCapture property directly saves an expensive
    // prototype lookup in a very sensitive hot path.
    this[kCapture] = EventEmitter.prototype[kCapture];
  }
};

function addCatch(that, promise, type, args) {
  if (!that[kCapture]) {
    return;
  }

  // Handle Promises/A+ spec, then could be a getter
  // that throws on second use.
  try {
    const then = promise.then;

    if (typeof then === "function") {
      then.call(promise, undefined, function (err) {
        // The callback is called with nextTick to avoid a follow-up
        // rejection from this promise.
        nextTick(emitUnhandledRejectionOrErr, that, err, type, args);
      });
    }
  } catch (err) {
    that.emit("error", err);
  }
}

function emitUnhandledRejectionOrErr(ee, err, type, args) {
  if (typeof ee[kRejection] === "function") {
    ee[kRejection](err, type, ...args);
  } else {
    // We have to disable the capture rejections mechanism, otherwise
    // we might end up in an infinite loop.
    const prev = ee[kCapture];

    // If the error handler throws, it is not catcheable and it
    // will end up in 'uncaughtException'. We restore the previous
    // value of kCapture in case the uncaughtException is present
    // and the exception is handled.
    try {
      ee[kCapture] = false;
      ee.emit("error", err);
    } finally {
      ee[kCapture] = prev;
    }
  }
}

/**
 * Increases the max listeners of the event emitter.
 * @param {number} n
 * @returns {EventEmitter}
 */
EventEmitter.prototype.setMaxListeners = function setMaxListeners(n) {
  if (typeof n !== "number" || n < 0 || Number.isNaN(n)) {
    throw new ERR_OUT_OF_RANGE("n", "a non-negative number", n);
  }
  this._maxListeners = n;
  return this;
};

function _getMaxListeners(that) {
  if (that._maxListeners === undefined) {
    return EventEmitter.defaultMaxListeners;
  }
  return that._maxListeners;
}

/**
 * Returns the current max listener value for the event emitter.
 * @returns {number}
 */
EventEmitter.prototype.getMaxListeners = function getMaxListeners() {
  return _getMaxListeners(this);
};

// Returns the length and line number of the first sequence of `a` that fully
// appears in `b` with a length of at least 4.
function identicalSequenceRange(a, b) {
  for (let i = 0; i < a.length - 3; i++) {
    // Find the first entry of b that matches the current entry of a.
    const pos = b.indexOf(a[i]);
    if (pos !== -1) {
      const rest = b.length - pos;
      if (rest > 3) {
        let len = 1;
        const maxLen = Math.min(a.length - i, rest);
        // Count the number of consecutive entries.
        while (maxLen > len && a[i + len] === b[pos + len]) {
          len++;
        }
        if (len > 3) {
          return [len, i];
        }
      }
    }
  }

  return [0, 0];
}

// deno-lint-ignore no-unused-vars
function enhanceStackTrace(err, own) {
  let ctorInfo = "";
  try {
    const { name } = this.constructor;
    if (name !== "EventEmitter") {
      ctorInfo = ` on ${name} instance`;
    }
  } catch {
    // pass
  }
  const sep = `\nEmitted 'error' event${ctorInfo} at:\n`;

  const errStack = err.stack.split("\n").slice(1);
  const ownStack = own.stack.split("\n").slice(1);

  const { 0: len, 1: off } = identicalSequenceRange(ownStack, errStack);
  if (len > 0) {
    ownStack.splice(
      off + 1,
      len - 2,
      "    [... lines matching original stack trace ...]",
    );
  }

  return err.stack + sep + ownStack.join("\n");
}

/**
 * Synchronously calls each of the listeners registered
 * for the event.
 * @param {string | symbol} type
 * @param {...any} [args]
 * @returns {boolean}
 */
EventEmitter.prototype.emit = function emit(type, ...args) {
  let doError = type === "error";

  const events = this._events;
  if (events !== undefined) {
    if (doError && events[kErrorMonitor] !== undefined) {
      this.emit(kErrorMonitor, ...args);
    }
    doError = doError && events.error === undefined;
  } else if (!doError) {
    return false;
  }

  // If there is no 'error' event listener then throw.
  if (doError) {
    let er;
    if (args.length > 0) {
      er = args[0];
    }
    if (er instanceof Error) {
      try {
        const capture = {};
        Error.captureStackTrace(capture, EventEmitter.prototype.emit);
        // Object.defineProperty(er, kEnhanceStackBeforeInspector, {
        //   value: enhanceStackTrace.bind(this, er, capture),
        //   configurable: true
        // });
      } catch {
        // pass
      }

      // Note: The comments on the `throw` lines are intentional, they show
      // up in Node's output if this results in an unhandled exception.
      throw er; // Unhandled 'error' event
    }

    let stringifiedEr;
    try {
      stringifiedEr = inspect(er);
    } catch {
      stringifiedEr = er;
    }

    // At least give some kind of context to the user
    const err = new ERR_UNHANDLED_ERROR(stringifiedEr);
    err.context = er;
    throw err; // Unhandled 'error' event
  }

  const handler = events[type];

  if (handler === undefined) {
    return false;
  }

  if (typeof handler === "function") {
    const result = handler.apply(this, args);

    // We check if result is undefined first because that
    // is the most common case so we do not pay any perf
    // penalty
    if (result !== undefined && result !== null) {
      addCatch(this, result, type, args);
    }
  } else {
    const len = handler.length;
    const listeners = arrayClone(handler);
    for (let i = 0; i < len; ++i) {
      const result = listeners[i].apply(this, args);

      // We check if result is undefined first because that
      // is the most common case so we do not pay any perf
      // penalty.
      // This code is duplicated because extracting it away
      // would make it non-inlineable.
      if (result !== undefined && result !== null) {
        addCatch(this, result, type, args);
      }
    }
  }

  return true;
};

function _addListener(target, type, listener, prepend) {
  let m;
  let events;
  let existing;

  checkListener(listener);

  events = target._events;
  if (events === undefined) {
    events = target._events = Object.create(null);
    target._eventsCount = 0;
  } else {
    // To avoid recursion in the case that type === "newListener"! Before
    // adding it to the listeners, first emit "newListener".
    if (events.newListener !== undefined) {
      target.emit("newListener", type, listener.listener ?? listener);

      // Re-assign `events` because a newListener handler could have caused the
      // this._events to be assigned to a new object
      events = target._events;
    }
    existing = events[type];
  }

  if (existing === undefined) {
    // Optimize the case of one listener. Don't need the extra array object.
    events[type] = listener;
    ++target._eventsCount;
  } else {
    if (typeof existing === "function") {
      // Adding the second element, need to change to array.
      existing = events[type] = prepend
        ? [listener, existing]
        : [existing, listener];
      // If we've already got an array, just append.
    } else if (prepend) {
      existing.unshift(listener);
    } else {
      existing.push(listener);
    }

    // Check for listener leak
    m = _getMaxListeners(target);
    if (m > 0 && existing.length > m && !existing.warned) {
      existing.warned = true;
      // No error code for this since it is a Warning
      // eslint-disable-next-line no-restricted-syntax
      const w = new Error(
        "Possible EventEmitter memory leak detected. " +
          `${existing.length} ${String(type)} listeners ` +
          `added to ${inspect(target, { depth: -1 })}. Use ` +
          "emitter.setMaxListeners() to increase limit",
      );
      w.name = "MaxListenersExceededWarning";
      w.emitter = target;
      w.type = type;
      w.count = existing.length;
      process.emitWarning(w);
    }
  }

  return target;
}

/**
 * Adds a listener to the event emitter.
 * @param {string | symbol} type
 * @param {Function} listener
 * @returns {EventEmitter}
 */
EventEmitter.prototype.addListener = function addListener(type, listener) {
  return _addListener(this, type, listener, false);
};

EventEmitter.prototype.on = EventEmitter.prototype.addListener;

/**
 * Adds the `listener` function to the beginning of
 * the listeners array.
 * @param {string | symbol} type
 * @param {Function} listener
 * @returns {EventEmitter}
 */
EventEmitter.prototype.prependListener = function prependListener(
  type,
  listener,
) {
  return _addListener(this, type, listener, true);
};

function onceWrapper() {
  if (!this.fired) {
    this.target.removeListener(this.type, this.wrapFn);
    this.fired = true;
    if (arguments.length === 0) {
      return this.listener.call(this.target);
    }
    return this.listener.apply(this.target, arguments);
  }
}

function _onceWrap(target, type, listener) {
  const state = { fired: false, wrapFn: undefined, target, type, listener };
  const wrapped = onceWrapper.bind(state);
  wrapped.listener = listener;
  state.wrapFn = wrapped;
  return wrapped;
}

/**
 * Adds a one-time `listener` function to the event emitter.
 * @param {string | symbol} type
 * @param {Function} listener
 * @returns {EventEmitter}
 */
EventEmitter.prototype.once = function once(type, listener) {
  checkListener(listener);

  this.on(type, _onceWrap(this, type, listener));
  return this;
};

/**
 * Adds a one-time `listener` function to the beginning of
 * the listeners array.
 * @param {string | symbol} type
 * @param {Function} listener
 * @returns {EventEmitter}
 */
EventEmitter.prototype.prependOnceListener = function prependOnceListener(
  type,
  listener,
) {
  checkListener(listener);

  this.prependListener(type, _onceWrap(this, type, listener));
  return this;
};

/**
 * Removes the specified `listener` from the listeners array.
 * @param {string | symbol} type
 * @param {Function} listener
 * @returns {EventEmitter}
 */
EventEmitter.prototype.removeListener = function removeListener(
  type,
  listener,
) {
  checkListener(listener);

  const events = this._events;
  if (events === undefined) {
    return this;
  }

  const list = events[type];
  if (list === undefined) {
    return this;
  }

  if (list === listener || list.listener === listener) {
    if (--this._eventsCount === 0) {
      this._events = Object.create(null);
    } else {
      delete events[type];
      if (events.removeListener) {
        this.emit("removeListener", type, list.listener || listener);
      }
    }
  } else if (typeof list !== "function") {
    let position = -1;

    for (let i = list.length - 1; i >= 0; i--) {
      if (list[i] === listener || list[i].listener === listener) {
        position = i;
        break;
      }
    }

    if (position < 0) {
      return this;
    }

    if (position === 0) {
      list.shift();
    } else {
      spliceOne(list, position);
    }

    if (list.length === 1) {
      events[type] = list[0];
    }

    if (events.removeListener !== undefined) {
      this.emit("removeListener", type, listener);
    }
  }

  return this;
};

EventEmitter.prototype.off = EventEmitter.prototype.removeListener;

/**
 * Removes all listeners from the event emitter. (Only
 * removes listeners for a specific event name if specified
 * as `type`).
 * @param {string | symbol} [type]
 * @returns {EventEmitter}
 */
EventEmitter.prototype.removeAllListeners = function removeAllListeners(type) {
  const events = this._events;
  if (events === undefined) {
    return this;
  }

  // Not listening for removeListener, no need to emit
  if (events.removeListener === undefined) {
    if (arguments.length === 0) {
      this._events = Object.create(null);
      this._eventsCount = 0;
    } else if (events[type] !== undefined) {
      if (--this._eventsCount === 0) {
        this._events = Object.create(null);
      } else {
        delete events[type];
      }
    }
    return this;
  }

  // Emit removeListener for all listeners on all events
  if (arguments.length === 0) {
    for (const key of Reflect.ownKeys(events)) {
      if (key === "removeListener") continue;
      this.removeAllListeners(key);
    }
    this.removeAllListeners("removeListener");
    this._events = Object.create(null);
    this._eventsCount = 0;
    return this;
  }

  const listeners = events[type];

  if (typeof listeners === "function") {
    this.removeListener(type, listeners);
  } else if (listeners !== undefined) {
    // LIFO order
    for (let i = listeners.length - 1; i >= 0; i--) {
      this.removeListener(type, listeners[i]);
    }
  }

  return this;
};

function _listeners(target, type, unwrap) {
  const events = target._events;

  if (events === undefined) {
    return [];
  }

  const evlistener = events[type];
  if (evlistener === undefined) {
    return [];
  }

  if (typeof evlistener === "function") {
    return unwrap ? [evlistener.listener || evlistener] : [evlistener];
  }

  return unwrap ? unwrapListeners(evlistener) : arrayClone(evlistener);
}

/**
 * Returns a copy of the array of listeners for the event name
 * specified as `type`.
 * @param {string | symbol} type
 * @returns {Function[]}
 */
EventEmitter.prototype.listeners = function listeners(type) {
  return _listeners(this, type, true);
};

/**
 * Returns a copy of the array of listeners and wrappers for
 * the event name specified as `type`.
 * @param {string | symbol} type
 * @returns {Function[]}
 */
EventEmitter.prototype.rawListeners = function rawListeners(type) {
  return _listeners(this, type, false);
};

/**
 * Returns the number of listeners listening to event name
 * specified as `type`.
 * @param {string | symbol} type
 * @returns {number}
 */
const _listenerCount = function listenerCount(type) {
  const events = this._events;

  if (events !== undefined) {
    const evlistener = events[type];

    if (typeof evlistener === "function") {
      return 1;
    } else if (evlistener !== undefined) {
      return evlistener.length;
    }
  }

  return 0;
};

EventEmitter.prototype.listenerCount = _listenerCount;

/**
 * Returns the number of listeners listening to the event name
 * specified as `type`.
 * @deprecated since v3.2.0
 * @param {EventEmitter} emitter
 * @param {string | symbol} type
 * @returns {number}
 */
export function listenerCount(emitter, type) {
  if (typeof emitter.listenerCount === "function") {
    return emitter.listenerCount(type);
  }
  return _listenerCount.call(emitter, type);
}

/**
 * Returns an array listing the events for which
 * the emitter has registered listeners.
 * @returns {any[]}
 */
EventEmitter.prototype.eventNames = function eventNames() {
  return this._eventsCount > 0 ? Reflect.ownKeys(this._events) : [];
};

function arrayClone(arr) {
  // At least since V8 8.3, this implementation is faster than the previous
  // which always used a simple for-loop
  switch (arr.length) {
    case 2:
      return [arr[0], arr[1]];
    case 3:
      return [arr[0], arr[1], arr[2]];
    case 4:
      return [arr[0], arr[1], arr[2], arr[3]];
    case 5:
      return [arr[0], arr[1], arr[2], arr[3], arr[4]];
    case 6:
      return [arr[0], arr[1], arr[2], arr[3], arr[4], arr[5]];
  }
  return arr.slice();
}

function unwrapListeners(arr) {
  const ret = arrayClone(arr);
  for (let i = 0; i < ret.length; ++i) {
    const orig = ret[i].listener;
    if (typeof orig === "function") {
      ret[i] = orig;
    }
  }
  return ret;
}

/**
 * Returns a copy of the array of listeners for the event name
 * specified as `type`.
 * @param {EventEmitter | EventTarget} emitterOrTarget
 * @param {string | symbol} type
 * @returns {Function[]}
 */
export function getEventListeners(emitterOrTarget, type) {
  // First check if EventEmitter
  if (typeof emitterOrTarget.listeners === "function") {
    return emitterOrTarget.listeners(type);
  }
  if (emitterOrTarget instanceof EventTarget) {
    // TODO: kEvents is not defined
    const root = emitterOrTarget[kEvents].get(type);
    const listeners = [];
    let handler = root?.next;
    while (handler?.listener !== undefined) {
      const listener = handler.listener?.deref
        ? handler.listener.deref()
        : handler.listener;
      listeners.push(listener);
      handler = handler.next;
    }
    return listeners;
  }
  throw new ERR_INVALID_ARG_TYPE(
    "emitter",
    ["EventEmitter", "EventTarget"],
    emitterOrTarget,
  );
}

/**
 * Creates a `Promise` that is fulfilled when the emitter
 * emits the given event.
 * @param {EventEmitter} emitter
 * @param {string} name
 * @param {{ signal: AbortSignal; }} [options]
 * @returns {Promise}
 */
// deno-lint-ignore require-await
export async function once(emitter, name, options = {}) {
  const signal = options?.signal;
  validateAbortSignal(signal, "options.signal");
  if (signal?.aborted) {
    throw new AbortError();
  }
  return new Promise((resolve, reject) => {
    const errorListener = (err) => {
      emitter.removeListener(name, resolver);
      if (signal != null) {
        eventTargetAgnosticRemoveListener(signal, "abort", abortListener);
      }
      reject(err);
    };
    const resolver = (...args) => {
      if (typeof emitter.removeListener === "function") {
        emitter.removeListener("error", errorListener);
      }
      if (signal != null) {
        eventTargetAgnosticRemoveListener(signal, "abort", abortListener);
      }
      resolve(args);
    };
    eventTargetAgnosticAddListener(emitter, name, resolver, { once: true });
    if (name !== "error" && typeof emitter.once === "function") {
      emitter.once("error", errorListener);
    }
    function abortListener() {
      eventTargetAgnosticRemoveListener(emitter, name, resolver);
      eventTargetAgnosticRemoveListener(emitter, "error", errorListener);
      reject(new AbortError());
    }
    if (signal != null) {
      eventTargetAgnosticAddListener(
        signal,
        "abort",
        abortListener,
        { once: true },
      );
    }
  });
}

const AsyncIteratorPrototype = Object.getPrototypeOf(
  Object.getPrototypeOf(async function* () {}).prototype,
);

function createIterResult(value, done) {
  return { value, done };
}

function eventTargetAgnosticRemoveListener(emitter, name, listener, flags) {
  if (typeof emitter.removeListener === "function") {
    emitter.removeListener(name, listener);
  } else if (typeof emitter.removeEventListener === "function") {
    emitter.removeEventListener(name, listener, flags);
  } else {
    throw new ERR_INVALID_ARG_TYPE("emitter", "EventEmitter", emitter);
  }
}

function eventTargetAgnosticAddListener(emitter, name, listener, flags) {
  if (typeof emitter.on === "function") {
    if (flags?.once) {
      emitter.once(name, listener);
    } else {
      emitter.on(name, listener);
    }
  } else if (typeof emitter.addEventListener === "function") {
    // EventTarget does not have `error` event semantics like Node
    // EventEmitters, we do not listen to `error` events here.
    emitter.addEventListener(name, (arg) => {
      listener(arg);
    }, flags);
  } else {
    throw new ERR_INVALID_ARG_TYPE("emitter", "EventEmitter", emitter);
  }
}

/**
 * Returns an `AsyncIterator` that iterates `event` events.
 * @param {EventEmitter} emitter
 * @param {string | symbol} event
 * @param {{ signal: AbortSignal; }} [options]
 * @returns {AsyncIterator}
 */
export function on(emitter, event, options) {
  const signal = options?.signal;
  validateAbortSignal(signal, "options.signal");
  if (signal?.aborted) {
    throw new AbortError();
  }

  const unconsumedEvents = [];
  const unconsumedPromises = [];
  let error = null;
  let finished = false;

  const iterator = Object.setPrototypeOf({
    next() {
      // First, we consume all unread events
      const value = unconsumedEvents.shift();
      if (value) {
        return Promise.resolve(createIterResult(value, false));
      }

      // Then we error, if an error happened
      // This happens one time if at all, because after 'error'
      // we stop listening
      if (error) {
        const p = Promise.reject(error);
        // Only the first element errors
        error = null;
        return p;
      }

      // If the iterator is finished, resolve to done
      if (finished) {
        return Promise.resolve(createIterResult(undefined, true));
      }

      // Wait until an event happens
      return new Promise(function (resolve, reject) {
        unconsumedPromises.push({ resolve, reject });
      });
    },

    return() {
      eventTargetAgnosticRemoveListener(emitter, event, eventHandler);
      eventTargetAgnosticRemoveListener(emitter, "error", errorHandler);

      if (signal) {
        eventTargetAgnosticRemoveListener(
          signal,
          "abort",
          abortListener,
          { once: true },
        );
      }

      finished = true;

      for (const promise of unconsumedPromises) {
        promise.resolve(createIterResult(undefined, true));
      }

      return Promise.resolve(createIterResult(undefined, true));
    },

    throw(err) {
      if (!err || !(err instanceof Error)) {
        throw new ERR_INVALID_ARG_TYPE(
          "EventEmitter.AsyncIterator",
          "Error",
          err,
        );
      }
      error = err;
      eventTargetAgnosticRemoveListener(emitter, event, eventHandler);
      eventTargetAgnosticRemoveListener(emitter, "error", errorHandler);
    },

    [Symbol.asyncIterator]() {
      return this;
    },
  }, AsyncIteratorPrototype);

  eventTargetAgnosticAddListener(emitter, event, eventHandler);
  if (event !== "error" && typeof emitter.on === "function") {
    emitter.on("error", errorHandler);
  }

  if (signal) {
    eventTargetAgnosticAddListener(
      signal,
      "abort",
      abortListener,
      { once: true },
    );
  }

  return iterator;

  function abortListener() {
    errorHandler(new AbortError());
  }

  function eventHandler(...args) {
    const promise = unconsumedPromises.shift();
    if (promise) {
      promise.resolve(createIterResult(args, false));
    } else {
      unconsumedEvents.push(args);
    }
  }

  function errorHandler(err) {
    finished = true;

    const toError = unconsumedPromises.shift();

    if (toError) {
      toError.reject(err);
    } else {
      // The next time we call next()
      error = err;
    }

    iterator.return();
  }
}

const kAsyncResource = Symbol("kAsyncResource");
const kEventEmitter = Symbol("kEventEmitter");

class EventEmitterReferencingAsyncResource extends AsyncResource {
  /**
   * @param {EventEmitter} ee
   * @param {string} [type]
   * @param {{
   *   triggerAsyncId?: number,
   *   requireManualDestroy?: boolean,
   * }} [options]
   */
  constructor(ee, type, options) {
    super(type, options);
    this[kEventEmitter] = ee;
  }

  /**
   * @type {EventEmitter}
   */
  get eventEmitter() {
    if (this[kEventEmitter] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterReferencingAsyncResource");
    }
    return this[kEventEmitter];
  }
}

export class EventEmitterAsyncResource extends EventEmitter {
  /**
   * @param {{
   *   name?: string,
   *   triggerAsyncId?: number,
   *   requireManualDestroy?: boolean,
   * }} [options]
   */
  constructor(options = undefined) {
    let name;
    if (typeof options === "string") {
      name = options;
      options = undefined;
    } else {
      if (new.target === EventEmitterAsyncResource) {
        validateString(options?.name, "options.name");
      }
      name = options?.name || new.target.name;
    }
    super(options);

    this[kAsyncResource] = new EventEmitterReferencingAsyncResource(
      this,
      name,
      options,
    );
  }

  /**
   * @param {symbol,string} event
   * @param  {...any} args
   * @returns {boolean}
   */
  emit(event, ...args) {
    if (this[kAsyncResource] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterAsyncResource");
    }
    const { asyncResource } = this;
    args.unshift(super.emit, this, event);
    return asyncResource.runInAsyncScope.apply(asyncResource, args);
  }

  /**
   * @returns {void}
   */
  emitDestroy() {
    if (this[kAsyncResource] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterAsyncResource");
    }
    this.asyncResource.emitDestroy();
  }

  /**
   * @type {number}
   */
  get asyncId() {
    if (this[kAsyncResource] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterAsyncResource");
    }
    return this.asyncResource.asyncId();
  }

  /**
   * @type {number}
   */
  get triggerAsyncId() {
    if (this[kAsyncResource] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterAsyncResource");
    }
    return this.asyncResource.triggerAsyncId();
  }

  /**
   * @type {EventEmitterReferencingAsyncResource}
   */
  get asyncResource() {
    if (this[kAsyncResource] === undefined) {
      throw new ERR_INVALID_THIS("EventEmitterAsyncResource");
    }
    return this[kAsyncResource];
  }
}

EventEmitter.EventEmitterAsyncResource = EventEmitterAsyncResource;
