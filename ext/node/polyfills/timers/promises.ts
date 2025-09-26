// Copyright 2018-2025 the Deno authors. MIT license.
import timers, { clearTimeout } from "ext:deno_node/timers.ts";
import { primordials } from "ext:core/mod.js";
import {
  validateAbortSignal,
  validateBoolean,
  validateNumber,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { AbortError } from "ext:deno_node/internal/errors.ts";
import { Timeout } from "ext:deno_node/internal/timers.mjs";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";
import type { Abortable } from "node:events";
import { kResistStopPropagation } from "ext:deno_node/internal/event_target.mjs";

const {
  FunctionPrototypeBind,
  PromiseReject,
  PromiseWithResolvers,
  SafePromisePrototypeFinally,
} = primordials;

export interface TimerOptions extends Abortable {
  ref?: boolean | undefined;
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

export function setTimeout(
  after: number | undefined,
  value: unknown,
  options: TimerOptions = kEmptyObject,
) {
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

export const setImmediate = timers.promises.setImmediate;
export const setInterval = timers.promises.setInterval;

export const scheduler = timers.promises.scheduler;

export default {
  setTimeout,
  setImmediate,
  setInterval,
  scheduler,
};
