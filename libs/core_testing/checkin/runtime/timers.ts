// Copyright 2018-2026 the Deno authors. MIT license.

/**
 * Call a callback function after a delay.
 */
export function setTimeout(callback: () => void, delay = 0) {
  return Deno.core.createTimer(callback, delay, undefined, false, true);
}

/**
 * Call a callback function after a delay.
 */
export function setInterval(callback: () => void, delay = 0) {
  return Deno.core.createTimer(callback, delay, undefined, true, true);
}

/**
 * Clear a timeout or interval.
 */
// deno-lint-ignore no-explicit-any
export function clearTimeout(timer: any) {
  Deno.core.cancelTimer(timer);
}

/**
 * Clear a timeout or interval.
 */
// deno-lint-ignore no-explicit-any
export function clearInterval(timer: any) {
  Deno.core.cancelTimer(timer);
}

/**
 * Schedule a callback to run in the check phase (after I/O).
 */
export function setImmediate(callback: () => void) {
  const immediate = {
    _idleNext: null,
    _idlePrev: null,
    _onImmediate: callback,
    _argv: null,
    _destroyed: false,
    [Deno.core.kRefed]: false,
    asyncId: 0,
    triggerAsyncId: 0,
  };
  immediate[Deno.core.kRefed] = true;
  Deno.core.immediateRefCount(true);
  Deno.core.queueImmediate(immediate);
  return immediate;
}

/**
 * Cancel a scheduled immediate.
 */
// deno-lint-ignore no-explicit-any
export function clearImmediate(immediate: any) {
  Deno.core.clearImmediate(immediate);
}

/**
 * Mark an immediate as not blocking event loop exit.
 */
// deno-lint-ignore no-explicit-any
export function unrefImmediate(immediate: any) {
  if (immediate && immediate[Deno.core.kRefed]) {
    immediate[Deno.core.kRefed] = false;
    Deno.core.immediateRefCount(false);
  }
}

/**
 * Mark a timer as not blocking event loop exit.
 */
// deno-lint-ignore no-explicit-any
export function unrefTimer(timer: any) {
  Deno.core.unrefTimer(timer);
}

/**
 * Mark a timer as blocking event loop exit.
 */
// deno-lint-ignore no-explicit-any
export function refTimer(timer: any) {
  Deno.core.refTimer(timer);
}
