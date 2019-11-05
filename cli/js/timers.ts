// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";

const { console } = window;

// Timeout values > TIMEOUT_MAX are set to 1.
const TIMEOUT_MAX = 2 ** 31 - 1;
let nextTimerId = 1;

export type Args = unknown[];

function checkThis(thisArg: unknown): void {
  if (thisArg !== null && thisArg !== undefined && thisArg !== window) {
    throw new TypeError("Illegal invocation");
  }
}

function checkBigInt(n: unknown): void {
  if (typeof n === "bigint") {
    throw new TypeError("Cannot convert a BigInt value to a number");
  }
}

export function clearTimeout(id = 0): void {
  checkBigInt(id);
  if (id === 0) {
    return;
  }
  const timeout = timeoutMap.get(id);
  if (!timeout) {
    return;
  }
  timeout.cancel();
  timeoutMap.delete(id);
}

/** Sets a timer which executes a function once after the timer expires. */
export function setTimeout(
  cb: (...args: Args) => void,
  delay = 0,
  ...args: Args
): number {
  checkBigInt(delay);
  // @ts-ignore
  checkThis(this);
  const timeout = new Timeout(delay, cb, args);
  timeoutMap.set(timeout.id, timeout);
  // ignore promise so it's run in the background
  timeout.poll();
  return timeout.id;
}

class Timeout {
  public id: number;
  callback: () => void;
  duration: number;
  cancelled: boolean;
  rid: number;

  constructor(duration: number, cb: () => void, args: Args) {
    // Bind `args` to the callback and bind `this` to window(global).
    const callback: () => void = cb.bind(window, ...args);
    // In the browser, the delay value must be coercible to an integer between 0
    // and INT32_MAX. Any other value will cause the timer to fire immediately.
    // We emulate this behavior.
    if (duration > TIMEOUT_MAX) {
      console.warn(
        `${duration} does not fit into` +
          " a 32-bit signed integer." +
          "\nTimeout duration was set to 1."
      );
      duration = 1;
    }
    duration = Math.max(0, duration | 0);

    this.id = nextTimerId++;
    this.callback = callback;
    this.duration = duration;
    this.cancelled = false;
    this.rid = sendSync(dispatch.OP_SET_TIMEOUT, { duration });
  }

  async poll(): Promise<void> {
    await sendAsync(dispatch.OP_POLL_TIMEOUT, {
      rid: this.rid
    });

    if (this.cancelled) {
      return;
    }

    // Call the user callback. Intermediate assignment is to avoid leaking `this`
    // to it, while also keeping the stack trace neat when it shows up in there.
    const callback = this.callback;
    callback();
  }

  cancel(): void {
    this.cancelled = true;
    sendSync(dispatch.OP_CLEAR_TIMEOUT, { rid: this.rid });
  }
}

const timeoutMap: Map<number, Timeout> = new Map();

class Interval {
  public id: number;
  callback: () => void;
  duration: number;
  cancelled: boolean;
  rid: number;

  constructor(duration: number, cb: () => void, args: Args) {
    // Bind `args` to the callback and bind `this` to window(global).
    const callback: () => void = cb.bind(window, ...args);
    // In the browser, the delay value must be coercible to an integer between 0
    // and INT32_MAX. Any other value will cause the timer to fire immediately.
    // We emulate this behavior.
    if (duration > TIMEOUT_MAX) {
      console.warn(
        `${duration} does not fit into` +
          " a 32-bit signed integer." +
          "\nTimeout duration was set to 1."
      );
      duration = 1;
    }
    duration = Math.max(0, duration | 0);

    this.id = nextTimerId++;
    this.callback = callback;
    this.duration = duration;
    this.cancelled = false;
    this.rid = sendSync(dispatch.OP_SET_INTERVAL, { duration });
  }

  async poll(): Promise<void> {
    for await (const cancelled of this) {
      if (cancelled) {
        return;
      }
      // Call the user callback. Intermediate assignment is to avoid leaking `this`
      // to it, while also keeping the stack trace neat when it shows up in there.
      const callback = this.callback;
      callback();
    }
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<boolean> {
    return this;
  }

  async next(): Promise<IteratorResult<boolean>> {
    if (this.cancelled) {
      return { value: true, done: true };
    }

    this.cancelled = await sendAsync(dispatch.OP_POLL_INTERVAL, {
      rid: this.rid
    });

    return { value: this.cancelled, done: this.cancelled };
  }

  cancel(): void {
    this.cancelled = true;
    sendSync(dispatch.OP_CLEAR_INTERVAL, { rid: this.rid });
  }
}

const intervalMap: Map<number, Interval> = new Map();

/** Repeatedly calls a function , with a fixed time delay between each call. */
export function setInterval(
  cb: (...args: Args) => void,
  delay = 0,
  ...args: Args
): number {
  checkBigInt(delay);
  // @ts-ignore
  checkThis(this);
  const interval = new Interval(delay, cb, args);
  intervalMap.set(interval.id, interval);
  // ignore promise so it's run in the background
  interval.poll();
  return interval.id;
}

export function clearInterval(id = 0): void {
  checkBigInt(id);
  if (id === 0) {
    return;
  }

  const interval = intervalMap.get(id);
  if (!interval) {
    return;
  }

  interval.cancel();
  intervalMap.delete(id);
}
