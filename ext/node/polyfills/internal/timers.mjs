// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { inspect } from "ext:deno_node/internal/util/inspect.mjs";
import { validateFunction, validateNumber } from "ext:deno_node/internal/validators.mjs";
import { ERR_OUT_OF_RANGE } from "ext:deno_node/internal/errors.ts";
import { emitWarning } from "ext:deno_node/process.ts";
import {
  setTimeout as setTimeout_,
  clearTimeout as clearTimeout_,
  setInterval as setInterval_,
} from "ext:deno_web/02_timers.js";

// Timeout values > TIMEOUT_MAX are set to 1.
export const TIMEOUT_MAX = 2 ** 31 - 1;

export const kTimerId = Symbol("timerId");
export const kTimeout = Symbol("timeout");
const kRefed = Symbol("refed");
const createTimer = Symbol("createTimer");

// Timer constructor function.
export function Timeout(callback, after, args, isRepeat, isRefed) {
  if (typeof after === "number" && after > TIMEOUT_MAX) {
    after = 1;
  }
  this._idleTimeout = after;
  this._onTimeout = callback;
  this._timerArgs = args;
  this._isRepeat = isRepeat;
  this[kRefed] = isRefed;
  this[kTimerId] = this[createTimer]();
}

Timeout.prototype[createTimer] = function () {
  const callback = this._onTimeout;
  const cb = (...args) => callback.bind(this)(...args);
  const id = this._isRepeat
    ? setInterval_(cb, this._idleTimeout, ...this._timerArgs)
    : setTimeout_(cb, this._idleTimeout, ...this._timerArgs);
  if (!this[kRefed]) {
    Deno.unrefTimer(id);
  }
  return id;
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
  clearTimeout_(this[kTimerId]);
  this[kTimerId] = this[createTimer]();
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

Timeout.prototype[Symbol.toPrimitive] = function () {
  return this[kTimerId];
};

/**
 * @param {number} msecs
 * @param {string} name
 * @returns
 */
export function getTimerDuration(msecs, name) {
  validateNumber(msecs, name);

  if (msecs < 0 || !Number.isFinite(msecs)) {
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

export default {
  getTimerDuration,
  kTimerId,
  kTimeout,
  setUnrefTimeout,
  Timeout,
  TIMEOUT_MAX,
};
