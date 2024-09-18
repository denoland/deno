// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  MapPrototypeGet,
  MapPrototypeDelete,
  ObjectDefineProperty,
  Promise,
  SafeArrayIterator,
} = primordials;

import {
  activeTimers,
  Immediate,
  setUnrefTimeout,
  Timeout,
} from "ext:deno_node/internal/timers.mjs";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
export { setUnrefTimeout } from "ext:deno_node/internal/timers.mjs";
import * as timers from "ext:deno_web/02_timers.js";

const clearTimeout_ = timers.clearTimeout;
const clearInterval_ = timers.clearInterval;

export function setTimeout(
  callback: (...args: unknown[]) => void,
  timeout?: number,
  ...args: unknown[]
) {
  validateFunction(callback, "callback");
  return new Timeout(callback, timeout, args, false, true);
}

ObjectDefineProperty(setTimeout, promisify.custom, {
  __proto__: null,
  value: (timeout: number, ...args: unknown[]) => {
    return new Promise((cb) =>
      setTimeout(cb, timeout, ...new SafeArrayIterator(args))
    );
  },
  enumerable: true,
});
export function clearTimeout(timeout?: Timeout | number) {
  if (timeout == null) {
    return;
  }
  const id = +timeout;
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    timeout._destroyed = true;
    MapPrototypeDelete(activeTimers, id);
  }
  clearTimeout_(id);
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
  const id = +timeout;
  const timer = MapPrototypeGet(activeTimers, id);
  if (timer) {
    timeout._destroyed = true;
    MapPrototypeDelete(activeTimers, id);
  }
  clearInterval_(id);
}
export function setImmediate(
  cb: (...args: unknown[]) => void,
  ...args: unknown[]
): Timeout {
  return new Immediate(cb, ...new SafeArrayIterator(args));
}
export function clearImmediate(immediate: Immediate) {
  if (immediate == null) {
    return;
  }

  // FIXME(nathanwhit): will probably change once
  //  deno_core has proper support for immediates
  clearTimeout_(immediate._immediateId);
}

export const promises = {
  setTimeout: promisify(setTimeout),
  setImmediate: promisify(setImmediate),
  setInterval: promisify(setInterval),
};

promises.scheduler = {
  async wait(
    delay: number,
    options?: { signal?: AbortSignal },
  ): Promise<void> {
    return await promises.setTimeout(delay, undefined, options);
  },
  yield: promises.setImmediate,
};

export default {
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  setImmediate,
  setUnrefTimeout,
  clearImmediate,
  promises,
};
