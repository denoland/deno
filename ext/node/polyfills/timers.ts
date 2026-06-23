// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  FunctionPrototypeBind,
  ObjectCreate,
  ObjectDefineProperty,
  Promise,
  PromiseReject,
  PromiseWithResolvers,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
  SymbolFor,
} = primordials;

// `node:test`'s `mock.timers` installs itself here while enabled. Routing the
// interception through a module-level hook (rather than swapping the module's
// exports) makes every call site honor the virtual clock regardless of how it
// reached these functions: `globalThis`, `require("node:timers")`, or a static
// `import { setTimeout } from "node:timers/promises"` binding captured before
// `enable()`. Node's MockTimers likewise intercepts the timers modules, not
// just the globals. `null` means no mocking is active.
let mockTimers = null;
const kInstallMockTimers = SymbolFor("Deno.internal.node.mockTimers");
const {
  getActiveTimer,
  Immediate,
  kDestroy,
  Timeout,
} = core.loadExtScript("ext:deno_node/internal/timers.mjs");
const {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateNumber,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { kEmptyObject, promisify } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const {
  AbortError,
  ERR_ILLEGAL_CONSTRUCTOR,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const lazyEventTarget = core.createLazyLoader(
  "ext:deno_node/internal/event_target.mjs",
);

interface TimerOptions {
  signal?: AbortSignal | undefined;
  ref?: boolean | undefined;
}

function setTimeout(
  callback: (...args: unknown[]) => void,
  timeout?: number,
  ...args: unknown[]
) {
  validateFunction(callback, "callback");
  if (mockTimers !== null && mockTimers._apiEnabled("setTimeout")) {
    return mockTimers._setTimeout(callback, timeout, args, false);
  }
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
  // When mocking is active the timer is driven by the virtual clock, so the
  // promise resolves on `tick()`/`runAll()` instead of a real timer.
  const timeout = mockTimers !== null && mockTimers._apiEnabled("setTimeout")
    ? mockTimers._setTimeout(resolve, after, [value], false)
    : new Timeout(resolve, after, [value], false, ref);
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
      [lazyEventTarget().kResistStopPropagation]: true,
    });
  }

  return oncancel !== undefined
    ? SafePromisePrototypeFinally(
      promise,
      () => signal!.removeEventListener("abort", oncancel),
    )
    : promise;
}

ObjectDefineProperty(setTimeoutPromise, "name", {
  __proto__: null,
  value: "setTimeout",
});

ObjectDefineProperty(setTimeout, promisify.custom, {
  __proto__: null,
  enumerable: true,
  get() {
    return setTimeoutPromise;
  },
});

function clearTimeout(timeout?: Timeout | number) {
  if (timeout == null) {
    return;
  }
  if (mockTimers !== null && mockTimers._apiEnabled("setTimeout")) {
    mockTimers._clearTimer(timeout);
    return;
  }
  const id = +timeout;
  getActiveTimer(id)?.[kDestroy]();
}
function setInterval(
  callback: (...args: unknown[]) => void,
  timeout?: number,
  ...args: unknown[]
) {
  validateFunction(callback, "callback");
  if (mockTimers !== null && mockTimers._apiEnabled("setInterval")) {
    return mockTimers._setInterval(callback, timeout, args);
  }
  return new Timeout(callback, timeout, args, true, true);
}
function clearInterval(timeout?: Timeout | number | string) {
  if (timeout == null) {
    return;
  }
  if (mockTimers !== null && mockTimers._apiEnabled("setInterval")) {
    mockTimers._clearTimer(timeout);
    return;
  }
  const id = +timeout;
  getActiveTimer(id)?.[kDestroy]();
}
function setImmediate(
  cb: (...args: unknown[]) => void,
  ...args: unknown[]
): Timeout {
  validateFunction(cb, "callback");
  if (mockTimers !== null && mockTimers._apiEnabled("setImmediate")) {
    return mockTimers._setTimeout(cb, 0, args, true);
  }
  return new Immediate(cb, ...new SafeArrayIterator(args));
}

function setImmediatePromise<T = void>(
  value?: T,
  options: TimerOptions = kEmptyObject,
): Promise<T> {
  try {
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
  const immediate =
    mockTimers !== null && mockTimers._apiEnabled("setImmediate")
      ? mockTimers._setTimeout(() => resolve(value), 0, [], true)
      : new Immediate(() => resolve(value));
  if (!ref) {
    immediate.unref();
  }
  if (signal) {
    oncancel = FunctionPrototypeBind(
      cancelListenerHandler,
      immediate,
      clearImmediate,
      reject,
      signal,
    );

    signal.addEventListener("abort", oncancel, {
      __proto__: null,
      [lazyEventTarget().kResistStopPropagation]: true,
    });
  }

  return oncancel !== undefined
    ? SafePromisePrototypeFinally(
      promise,
      () => signal!.removeEventListener("abort", oncancel),
    )
    : promise;
}

ObjectDefineProperty(setImmediatePromise, "name", {
  __proto__: null,
  value: "setImmediate",
});

ObjectDefineProperty(setImmediate, promisify.custom, {
  __proto__: null,
  enumerable: true,
  get() {
    return setImmediatePromise;
  },
});

function clearImmediate(immediate: Immediate) {
  if (immediate == null) {
    return;
  }
  if (mockTimers !== null && mockTimers._apiEnabled("setImmediate")) {
    mockTimers._clearTimer(immediate);
    return;
  }
  if (!immediate?._onImmediate || immediate._destroyed) {
    return;
  }
  core.clearImmediate(immediate);
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
    const onInterval = () => {
      notYielded++;
      if (callback) {
        callback();
        callback = undefined;
        rejectCallback = undefined;
      }
    };
    interval = mockTimers !== null && mockTimers._apiEnabled("setInterval")
      ? mockTimers._setInterval(onInterval, after, [])
      : new Timeout(onInterval, after, [], true, ref);
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
    throw new AbortError(undefined, { cause: signal?.reason });
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

const promises = {
  setTimeout: setTimeoutPromise,
  setImmediate: setImmediatePromise,
  setInterval: setIntervalAsync,
};

class Scheduler {
  constructor() {
    throw new ERR_ILLEGAL_CONSTRUCTOR();
  }
  async wait(
    delay: number,
    options?: { signal?: AbortSignal },
  ): Promise<void> {
    return await setTimeoutPromise(delay, undefined, options);
  }
  yield() {
    return promises.setImmediate();
  }
}

const scheduler = ObjectCreate(Scheduler.prototype);
promises.scheduler = scheduler;

return {
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  setImmediate,
  clearImmediate,
  promises,
  // Internal entry point for `node:test`'s `mock.timers`. Keyed by a symbol so
  // it does not surface as a public `node:timers` export. Passing a MockTimers
  // instance enables interception; passing `null` disables it.
  [kInstallMockTimers]: (instance) => {
    mockTimers = instance;
  },
};
})();
