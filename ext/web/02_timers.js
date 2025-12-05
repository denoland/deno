// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_defer } from "ext:core/ops";
const {
  PromisePrototypeThen,
  TypeError,
  indirectEval,
  ReflectApply,
} = primordials;
const {
  getAsyncContext,
  setAsyncContext,
} = core;

import * as webidl from "ext:deno_webidl/00_webidl.js";

// ---------------------------------------------------------------------------

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
  // If callback is a string, replace it with a function that evals the string on every timeout
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  callback = () => {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      setAsyncContext(oldContext);
    }
  };
  timeout = webidl.converters.long(timeout);
  return core.queueUserTimer(
    core.getTimerDepth() + 1,
    false,
    timeout,
    callback,
  );
}

/**
 * Call a callback function after a delay.
 */
function setInterval(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  callback = () => {
    const oldContext = getAsyncContext(asyncContext);
    try {
      setAsyncContext(asyncContext);
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      setAsyncContext(oldContext);
    }
  };
  timeout = webidl.converters.long(timeout);
  return core.queueUserTimer(
    core.getTimerDepth() + 1,
    true,
    timeout,
    callback,
  );
}

/**
 * Clear a timeout or interval.
 */
function clearTimeout(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  core.cancelTimer(id);
}

/**
 * Clear a timeout or interval.
 */
function clearInterval(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  core.cancelTimer(id);
}

/**
 * Mark a timer as not blocking event loop exit.
 */
function unrefTimer(id) {
  core.unrefTimer(id);
}

/**
 * Mark a timer as blocking event loop exit.
 */
function refTimer(id) {
  core.refTimer(id);
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
