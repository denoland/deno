// Copyright 2018-2026 the Deno authors. MIT license.

// Web timers (setTimeout/setInterval) built on top of Node's Timeout class.
// The Node Timeout class is the canonical timer implementation that wraps
// core.createTimer. Web timers add WHATWG-specific behavior on top:
// - webidl type coercion
// - string callback eval (WHATWG spec)
// - timer nesting depth tracking (WHATWG spec)
// - numeric timer IDs (vs Node's Timeout objects)

import { core, primordials } from "ext:core/mod.js";
import { op_defer } from "ext:core/ops";
const {
  PromisePrototypeThen,
  TypeError,
  indirectEval,
  ReflectApply,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";

const loadNodeTimers = core.createLazyLoader(
  "ext:deno_node/internal/timers.mjs",
);

// WHATWG timer nesting depth tracking.
let timerDepth = 0;

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
  const depth = timerDepth;
  const wrappedCallback = function () {
    const prevDepth = timerDepth;
    try {
      timerDepth = depth + 1;
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      timerDepth = prevDepth;
    }
  };
  timeout = webidl.converters.long(timeout);
  const { Timeout, kTimerId } = loadNodeTimers();
  const t = new Timeout(wrappedCallback, timeout, undefined, false, true);
  return t[kTimerId];
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
  const depth = timerDepth;
  const wrappedCallback = function () {
    const prevDepth = timerDepth;
    try {
      timerDepth = depth + 1;
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      timerDepth = prevDepth;
    }
  };
  timeout = webidl.converters.long(timeout);
  const { Timeout, kTimerId } = loadNodeTimers();
  const t = new Timeout(wrappedCallback, timeout, undefined, true, true);
  return t[kTimerId];
}

/**
 * Clear a timeout or interval.
 */
function clearTimeout(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const { getActiveTimer, kDestroy } = loadNodeTimers();
  const timer = getActiveTimer(id);
  if (timer) {
    timer[kDestroy]();
  }
}

/**
 * Clear a timeout or interval.
 */
function clearInterval(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const { getActiveTimer, kDestroy } = loadNodeTimers();
  const timer = getActiveTimer(id);
  if (timer) {
    timer[kDestroy]();
  }
}

/**
 * Mark a timer as not blocking event loop exit.
 */
function unrefTimer(id) {
  const { getActiveTimer, kTimerId } = loadNodeTimers();
  if (typeof id !== "number") {
    id = id[kTimerId];
  }
  const timer = getActiveTimer(id);
  if (timer) {
    timer.unref();
  }
}

/**
 * Mark a timer as blocking event loop exit.
 */
function refTimer(id) {
  const { getActiveTimer, kTimerId } = loadNodeTimers();
  if (typeof id !== "number") {
    id = id[kTimerId];
  }
  const timer = getActiveTimer(id);
  if (timer) {
    timer.ref();
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
