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
  Deno.core.cancelTimer2(timer);
}

/**
 * Clear a timeout or interval.
 */
// deno-lint-ignore no-explicit-any
export function clearInterval(timer: any) {
  Deno.core.cancelTimer2(timer);
}

/**
 * Mark a timer as not blocking event loop exit.
 */
// deno-lint-ignore no-explicit-any
export function unrefTimer(timer: any) {
  Deno.core.unrefTimer2(timer);
}

/**
 * Mark a timer as blocking event loop exit.
 */
// deno-lint-ignore no-explicit-any
export function refTimer(timer: any) {
  Deno.core.refTimer2(timer);
}
