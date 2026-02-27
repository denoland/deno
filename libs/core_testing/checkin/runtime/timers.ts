// Copyright 2018-2025 the Deno authors. MIT license.

/**
 * Call a callback function after a delay.
 */
export function setTimeout(callback: () => void, delay = 0) {
  return Deno.core.queueUserTimer(
    Deno.core.getTimerDepth() + 1,
    false,
    delay,
    callback,
  );
}

/**
 * Call a callback function after a delay.
 */
export function setInterval(callback: () => void, delay = 0) {
  return Deno.core.queueUserTimer(
    Deno.core.getTimerDepth() + 1,
    true,
    delay,
    callback,
  );
}

/**
 * Clear a timeout or interval.
 */
export function clearTimeout(id: number) {
  Deno.core.cancelTimer(id);
}

/**
 * Clear a timeout or interval.
 */
export function clearInterval(id: number) {
  Deno.core.cancelTimer(id);
}

/**
 * Mark a timer as not blocking event loop exit.
 */
export function unrefTimer(id: number) {
  Deno.core.unrefTimer(id);
}

/**
 * Mark a timer as blocking event loop exit.
 */
export function refTimer(id: number) {
  Deno.core.refTimer(id);
}
