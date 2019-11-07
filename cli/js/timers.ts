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
  id = Number(id);
  if (id === 0) {
    return;
  }

  const timeout = timeoutMap.get(id);
  // Timeout already fired or just bad id
  if (!timeout) {
    return;
  }

  // Mark as cancelled to prevent firing timeout if promise is resolved
  timeout.cancelled = true;
  timeoutMap.delete(id);

  try {
    sendSync(dispatch.OP_CLEAR_TIMEOUT, { rid: timeout.rid });
  } catch {
    // Might return bad resource id error if timeout already fired
  }
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
  // Bind `args` to the callback and bind `this` to window(global).
  const callback: () => void = cb.bind(window, ...args);
  // In the browser, the delay value must be coercible to an integer between 0
  // and INT32_MAX. Any other value will cause the timer to fire immediately.
  // We emulate this behavior.
  if (delay > TIMEOUT_MAX) {
    console.warn(
      `${delay} does not fit into` +
        " a 32-bit signed integer." +
        "\nTimeout delay was set to 1."
    );
    delay = 1;
  }
  delay = Math.max(0, delay | 0);

  const timeout = new Timeout(delay, callback);
  timeoutMap.set(timeout.id, timeout);
  // Run promise in the background
  timeout.poll();
  return timeout.id;
}

class Timeout {
  public id: number;
  callback: () => void;
  rid: number;
  cancelled: boolean;

  constructor(duration: number, cb: () => void) {
    this.id = nextTimerId++;
    this.cancelled = false;
    this.callback = cb;
    this.rid = sendSync(dispatch.OP_SET_TIMEOUT, { duration });
  }

  async poll(): Promise<void> {
    try {
      await sendAsync(dispatch.OP_POLL_TIMEOUT, {
        rid: this.rid
      });
    } catch {
      // Might return bad resource id error if timeout was cleared
    }

    Promise.resolve().then(() => {
      // If timeout was cleared before op resolved don't fire
      if (this.cancelled) {
        return;
      }

      // If we're this far mark this timeout as cancelled and remove from map
      // so it cannot be cleared anymore.
      this.cancelled = true;
      timeoutMap.delete(this.id);
      // Call the user callback. Intermediate assignment is to avoid leaking `this`
      // to it, while also keeping the stack trace neat when it shows up in there.
      const callback = this.callback;
      callback();
    });
  }
}

const timeoutMap: Map<number, Timeout> = new Map();

class Interval {
  public id: number;
  callback: () => void;
  duration: number;
  cancelled: boolean;
  rid: number;

  constructor(duration: number, cb: () => void) {
    this.id = nextTimerId++;
    this.callback = cb;
    this.duration = duration;
    this.cancelled = false;
    this.rid = sendSync(dispatch.OP_SET_INTERVAL, { duration });
  }

  async poll(): Promise<void> {
    // Run async iterator. If
    for await (const _cancelled of this) {
      Promise.resolve().then(() => {
        if (this.cancelled) {
          return;
        }
        // Call the user callback. Intermediate assignment is to avoid leaking `this`
        // to it, while also keeping the stack trace neat when it shows up in there.
        const callback = this.callback;
        callback();
      });
    }
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<boolean> {
    return this;
  }

  async next(): Promise<IteratorResult<boolean>> {
    // If interval was cleared don't start another promise
    if (this.cancelled) {
      return { value: true, done: true };
    }

    let cancelled;
    try {
      cancelled = await sendAsync(dispatch.OP_POLL_INTERVAL, {
        rid: this.rid
      });
    } catch {
      // Might throw bad resource id if interval was cleared
      cancelled = true;
    }

    this.cancelled = cancelled;
    return { value: cancelled, done: cancelled };
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
  // Bind `args` to the callback and bind `this` to window(global).
  const callback: () => void = cb.bind(window, ...args);
  // In the browser, the delay value must be coercible to an integer between 0
  // and INT32_MAX. Any other value will cause the timer to fire immediately.
  // We emulate this behavior.
  if (delay > TIMEOUT_MAX) {
    console.warn(
      `${delay} does not fit into` +
        " a 32-bit signed integer." +
        "\nInterval delay was set to 1."
    );
    delay = 1;
  }
  delay = Math.max(0, delay | 0);

  const interval = new Interval(delay, callback);
  intervalMap.set(interval.id, interval);
  // Run async iterator promise in the background
  interval.poll();
  return interval.id;
}

export function clearInterval(id = 0): void {
  checkBigInt(id);
  id = Number(id);
  if (id === 0) {
    return;
  }

  const interval = intervalMap.get(id);
  if (!interval) {
    return;
  }

  // Mark interval as cancelled to prevent firing on when current promise
  // resolves
  interval.cancelled = true;
  intervalMap.delete(id);

  try {
    sendSync(dispatch.OP_CLEAR_INTERVAL, { rid: interval.rid });
  } catch {}
}
