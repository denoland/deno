// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  FunctionPrototypeBind,
  ObjectDefineProperty,
  Promise,
  PromiseReject,
  PromiseWithResolvers,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
} = primordials;
import { op_immediate_count, op_immediate_ref_count } from "ext:core/ops";
import {
  getActiveTimer,
  Immediate,
  immediateQueue,
  kDestroy,
  kRefed,
  setUnrefTimeout,
  Timeout,
} from "ext:deno_node/internal/timers.mjs";
import {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateNumber,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { kEmptyObject, promisify } from "ext:deno_node/internal/util.mjs";
export { setUnrefTimeout } from "ext:deno_node/internal/timers.mjs";
import * as timers from "ext:deno_web/02_timers.js";
import { AbortError } from "ext:deno_node/internal/errors.ts";
import { kResistStopPropagation } from "ext:deno_node/internal/event_target.mjs";
import type { Abortable } from "node:events";

interface TimerOptions extends Abortable {
  ref?: boolean | undefined;
}

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

function cancelListenerHandler(
  clear: typeof clearTimeout,
  reject: typeof PromiseReject,
  signal: AbortSignal | undefined,
) {
  if (!this._destroyed) {
    clear(this);
    reject(new AbortError(undefined, { cause: signal?.reason }));
  }
}

function setTimeoutPromise<T = void>(
  after: number | undefined,
  value: T,
  options: TimerOptions = kEmptyObject,
): Promise<T> {
  try {
    if (typeof after !== "undefined") {
      validateNumber(after, "delay");
    }

    validateObject(options, "options");

    if (typeof options?.signal !== "undefined") {
      validateAbortSignal(options.signal, "options.signal");
    }

    if (typeof options?.ref !== "undefined") {
      validateBoolean(options.ref, "options.ref");
    }
  } catch (err) {
    return PromiseReject(err);
  }

  const { signal, ref = true } = options;

  if (signal?.aborted) {
    return PromiseReject(new AbortError(undefined, { cause: signal.reason }));
  }

  let oncancel: EventListenerOrEventListenerObject | undefined;
  const { promise, resolve, reject } = PromiseWithResolvers();
  const timeout = new Timeout(resolve, after, [value], false, ref);
  if (signal) {
    oncancel = FunctionPrototypeBind(
      cancelListenerHandler,
      timeout,
      clearTimeout,
      reject,
      signal,
    );

    signal.addEventListener("abort", oncancel, {
      __proto__: null,
      [kResistStopPropagation]: true,
    });
  }

  return oncancel !== undefined
    ? SafePromisePrototypeFinally(
      promise,
      () => signal!.removeEventListener("abort", oncancel),
    )
    : promise;
}

ObjectDefineProperty(setTimeout, promisify.custom, {
  __proto__: null,
  enumerable: true,
  get() {
    return setTimeoutPromise;
  },
});

export function clearTimeout(timeout?: Timeout | number) {
  if (timeout == null) {
    return;
  }
  const id = +timeout;
  getActiveTimer(id)?.[kDestroy]();
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
  getActiveTimer(id)?.[kDestroy]();
  clearInterval_(id);
}
export function setImmediate(
  cb: (...args: unknown[]) => void,
  ...args: unknown[]
): Timeout {
  validateFunction(cb, "callback");
  return new Immediate(cb, ...new SafeArrayIterator(args));
}

export function clearImmediate(immediate: Immediate) {
  if (!immediate?._onImmediate || immediate._destroyed) {
    return;
  }

  op_immediate_count(false);
  immediate._destroyed = true;

  if (immediate[kRefed]) {
    op_immediate_ref_count(false);
  }
  immediate[kRefed] = null;

  immediate._onImmediate = null;

  immediateQueue.remove(immediate);
}

async function* setIntervalAsync(
  after: number,
  value: number,
  options: { signal?: AbortSignal; ref?: boolean } = { __proto__: null },
) {
  validateObject(options, "options");

  if (typeof options?.signal !== "undefined") {
    validateAbortSignal(options.signal, "options.signal");
  }

  if (typeof options?.ref !== "undefined") {
    validateBoolean(options.ref, "options.ref");
  }

  const { signal, ref = true } = options;

  if (signal?.aborted) {
    throw new AbortError(undefined, { cause: signal?.reason });
  }

  let onCancel: (() => void) | undefined = undefined;
  let interval: Timeout | undefined = undefined;
  try {
    let notYielded = 0;
    let callback: ((value?: object) => void) | undefined = undefined;
    let rejectCallback: ((message?: string) => void) | undefined = undefined;
    interval = new Timeout(
      () => {
        notYielded++;
        if (callback) {
          callback();
          callback = undefined;
          rejectCallback = undefined;
        }
      },
      after,
      [],
      true,
      ref,
    );
    if (signal) {
      onCancel = () => {
        clearInterval(interval);
        if (rejectCallback) {
          rejectCallback(signal.reason);
          callback = undefined;
          rejectCallback = undefined;
        }
      };
      signal.addEventListener("abort", onCancel, { once: true });
    }
    while (!signal?.aborted) {
      if (notYielded === 0) {
        await new Promise((resolve: () => void, reject: () => void) => {
          callback = resolve;
          rejectCallback = reject;
        });
      }
      for (; notYielded > 0; notYielded--) {
        yield value;
      }
    }
  } catch (error) {
    if (signal?.aborted) {
      throw new AbortError(undefined, { cause: signal?.reason });
    }
    throw error;
  } finally {
    if (interval) {
      clearInterval(interval);
    }
    if (onCancel) {
      signal?.removeEventListener("abort", onCancel);
    }
  }
}

export const promises = {
  setTimeout: setTimeoutPromise,
  setImmediate: promisify(setImmediate),
  setInterval: setIntervalAsync,
};

promises.scheduler = {
  async wait(
    delay: number,
    options?: { signal?: AbortSignal },
  ): Promise<void> {
    return await setTimeoutPromise(delay, undefined, options);
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
