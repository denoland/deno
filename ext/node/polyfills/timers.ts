// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  setUnrefTimeout,
  Timeout,
} from "internal:deno_node/polyfills/internal/timers.mjs";
import { validateFunction } from "internal:deno_node/polyfills/internal/validators.mjs";
import { promisify } from "internal:deno_node/polyfills/internal/util.mjs";
export { setUnrefTimeout } from "internal:deno_node/polyfills/internal/timers.mjs";

const clearTimeout_ = globalThis.clearTimeout;
const clearInterval_ = globalThis.clearInterval;

export function setTimeout(
  callback: (...args: unknown[]) => void,
  timeout?: number,
  ...args: unknown[]
) {
  validateFunction(callback, "callback");
  return new Timeout(callback, timeout, args, false, true);
}

Object.defineProperty(setTimeout, promisify.custom, {
  value: (timeout: number, ...args: unknown[]) => {
    return new Promise((cb) => setTimeout(cb, timeout, ...args));
  },
  enumerable: true,
});
export function clearTimeout(timeout?: Timeout | number) {
  if (timeout == null) {
    return;
  }
  clearTimeout_(+timeout);
}
export function setInterval(
  callback: (...args: unknown[]) => void,
  timeout?: number,
  ...args: unknown[]
) {
  validateFunction(callback, "callback");
  return new Timeout(callback, timeout, args, true, true);
}
export function clearInterval(timeout?: Timeout | number | string) {
  if (timeout == null) {
    return;
  }
  clearInterval_(+timeout);
}
// TODO(bartlomieju): implement the 'NodeJS.Immediate' versions of the timers.
// https://github.com/DefinitelyTyped/DefinitelyTyped/blob/1163ead296d84e7a3c80d71e7c81ecbd1a130e9a/types/node/v12/globals.d.ts#L1120-L1131
export const setImmediate = (
  cb: (...args: unknown[]) => void,
  ...args: unknown[]
): Timeout => setTimeout(cb, 0, ...args);
export const clearImmediate = clearTimeout;

export default {
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  setImmediate,
  setUnrefTimeout,
  clearImmediate,
};
