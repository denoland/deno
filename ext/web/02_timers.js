// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_defer } from "ext:core/ops";
const {
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  PromisePrototypeThen,
  SafeMap,
  TypeError,
  indirectEval,
  ReflectApply,
} = primordials;
const {
  getAsyncContext,
  setAsyncContext,
  createTimer,
  cancelTimer2,
  refTimer2,
  unrefTimer2,
  getTimerDepth,
} = core;

import * as webidl from "ext:deno_webidl/00_webidl.js";

// ---------------------------------------------------------------------------
// Map numeric timer IDs to internal timer objects for clearTimeout/clearInterval.
const activeTimers = new SafeMap();
let nextWebTimerId = 1;

function checkThis(thisArg) {
  if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
    throw new TypeError("Illegal invocation");
  }
}

/**
 * Call a callback function after a delay.
 */
function setTimeout(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  const depth = getTimerDepth();
  const wrappedCallback = function () {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      // WHATWG timer nesting depth: track and clamp
      const prevDepth = getTimerDepth();
      core.__setTimerDepth(depth + 1);
      ReflectApply(unboundCallback, globalThis, args);
      core.__setTimerDepth(prevDepth);
    } finally {
      setAsyncContext(oldContext);
    }
    // One-shot: remove from map
    MapPrototypeDelete(activeTimers, webTimerId);
  };
  timeout = webidl.converters.long(timeout);
  const timer = createTimer(wrappedCallback, timeout, undefined, false, true);
  const webTimerId = nextWebTimerId++;
  MapPrototypeSet(activeTimers, webTimerId, timer);
  return webTimerId;
}

/**
 * Call a callback function repeatedly at a given interval.
 */
function setInterval(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  const depth = getTimerDepth();
  const wrappedCallback = function () {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      const prevDepth = getTimerDepth();
      core.__setTimerDepth(depth + 1);
      ReflectApply(unboundCallback, globalThis, args);
      core.__setTimerDepth(prevDepth);
    } finally {
      setAsyncContext(oldContext);
    }
  };
  timeout = webidl.converters.long(timeout);
  const timer = createTimer(wrappedCallback, timeout, undefined, true, true);
  const webTimerId = nextWebTimerId++;
  MapPrototypeSet(activeTimers, webTimerId, timer);
  return webTimerId;
}

/**
 * Clear a timeout or interval.
 */
function clearTimeout(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    cancelTimer2(timer);
    MapPrototypeDelete(activeTimers, id);
  } else {
    // Fall back to the old op for system timers (e.g. AbortSignal.timeout)
    core.cancelTimer(id);
  }
}

/**
 * Clear a timeout or interval.
 */
function clearInterval(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    cancelTimer2(timer);
    MapPrototypeDelete(activeTimers, id);
  } else {
    core.cancelTimer(id);
  }
}

/**
 * Mark a timer as not blocking event loop exit.
 */
function unrefTimer(id) {
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    unrefTimer2(timer);
  } else {
    // Fall back to the old op for system timers (e.g. AbortSignal.timeout)
    core.unrefTimer(id);
  }
}

/**
 * Mark a timer as blocking event loop exit.
 */
function refTimer(id) {
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    refTimer2(timer);
  } else {
    // Fall back to the old op for system timers (e.g. AbortSignal.timeout)
    core.refTimer(id);
  }
}

// Defer to avoid starving the event loop. Not using queueMicrotask()
// for that reason: it lets promises make forward progress but can
// still starve other parts of the event loop.
function defer(go) {
  PromisePrototypeThen(op_defer(), () => go());
}

export {
  clearInterval,
  clearTimeout,
  defer,
  refTimer,
  setInterval,
  setTimeout,
  unrefTimer,
};
