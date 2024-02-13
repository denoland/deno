// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/**
 * Utilities for mocking time while testing.
 *
 * @module
 */

import { RedBlackTree } from "../data_structures/red_black_tree.ts";
import { ascend } from "../data_structures/comparators.ts";
import type { DelayOptions } from "../async/delay.ts";
import { _internals } from "./_time.ts";

/** An error related to faking time. */
export class TimeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TimeError";
  }
}

function FakeTimeNow() {
  return time?.now ?? _internals.Date.now();
}

const FakeDate = new Proxy(Date, {
  construct(_target, args) {
    if (args.length === 0) args.push(FakeDate.now());
    // @ts-expect-error this is a passthrough
    return new _internals.Date(...args);
  },
  apply(_target, _thisArg, args) {
    if (args.length === 0) args.push(FakeDate.now());
    // @ts-expect-error this is a passthrough
    return _internals.Date(...args);
  },
  get(target, prop, receiver) {
    if (prop === "now") {
      return FakeTimeNow;
    }
    return Reflect.get(target, prop, receiver);
  },
});

interface Timer {
  id: number;
  // deno-lint-ignore no-explicit-any
  callback: (...args: any[]) => void;
  delay: number;
  args: unknown[];
  due: number;
  repeat: boolean;
}

export interface FakeTimeOptions {
  /**
   * The rate relative to real time at which fake time is updated.
   * By default time only moves forward through calling tick or setting now.
   * Set to 1 to have the fake time automatically tick forward at the same rate in milliseconds as real time.
   */
  advanceRate: number;
  /**
   * The frequency in milliseconds at which fake time is updated.
   * If advanceRate is set, we will update the time every 10 milliseconds by default.
   */
  advanceFrequency?: number;
}

interface DueNode {
  due: number;
  timers: Timer[];
}

let time: FakeTime | undefined = undefined;

function fakeSetTimeout(
  // deno-lint-ignore no-explicit-any
  callback: (...args: any[]) => void,
  delay = 0,
  // deno-lint-ignore no-explicit-any
  ...args: any[]
): number {
  if (!time) throw new TimeError("no fake time");
  return setTimer(callback, delay, args, false);
}

function fakeClearTimeout(id?: number) {
  if (!time) throw new TimeError("no fake time");
  if (typeof id === "number" && dueNodes.has(id)) {
    dueNodes.delete(id);
  }
}

function fakeSetInterval(
  // deno-lint-ignore no-explicit-any
  callback: (...args: any[]) => unknown,
  delay = 0,
  // deno-lint-ignore no-explicit-any
  ...args: any[]
): number {
  if (!time) throw new TimeError("no fake time");
  return setTimer(callback, delay, args, true);
}

function fakeClearInterval(id?: number) {
  if (!time) throw new TimeError("no fake time");
  if (typeof id === "number" && dueNodes.has(id)) {
    dueNodes.delete(id);
  }
}

function setTimer(
  // deno-lint-ignore no-explicit-any
  callback: (...args: any[]) => void,
  delay = 0,
  args: unknown[],
  repeat = false,
): number {
  const id: number = timerId.next().value;
  delay = Math.max(repeat ? 1 : 0, Math.floor(delay));
  const due: number = now + delay;
  let dueNode: DueNode | null = dueTree.find({ due } as DueNode);
  if (dueNode === null) {
    dueNode = { due, timers: [] };
    dueTree.insert(dueNode);
  }
  dueNode.timers.push({
    id,
    callback,
    args,
    delay,
    due,
    repeat,
  });
  dueNodes.set(id, dueNode);
  return id;
}

function overrideGlobals() {
  globalThis.Date = FakeDate;
  globalThis.setTimeout = fakeSetTimeout;
  globalThis.clearTimeout = fakeClearTimeout;
  globalThis.setInterval = fakeSetInterval;
  globalThis.clearInterval = fakeClearInterval;
}

function restoreGlobals() {
  globalThis.Date = _internals.Date;
  globalThis.setTimeout = _internals.setTimeout;
  globalThis.clearTimeout = _internals.clearTimeout;
  globalThis.setInterval = _internals.setInterval;
  globalThis.clearInterval = _internals.clearInterval;
}

function* timerIdGen() {
  let i = 1;
  while (true) yield i++;
}

function nextDueNode(): DueNode | null {
  for (;;) {
    const dueNode = dueTree.min();
    if (!dueNode) return null;
    const hasTimer = dueNode.timers.some((timer) => dueNodes.has(timer.id));
    if (hasTimer) return dueNode;
    dueTree.remove(dueNode);
  }
}

let startedAt: number;
let now: number;
let initializedAt: number;
let advanceRate: number;
let advanceFrequency: number;
let advanceIntervalId: number | undefined;
let timerId: Generator<number>;
let dueNodes: Map<number, DueNode>;
let dueTree: RedBlackTree<DueNode>;

/**
 * Overrides the real Date object and timer functions with fake ones that can be
 * controlled through the fake time instance.
 *
 * ```ts
 * import {
 *   assertSpyCalls,
 *   spy,
 * } from "https://deno.land/std@$STD_VERSION/testing/mock.ts";
 * import { FakeTime } from "https://deno.land/std@$STD_VERSION/testing/time.ts";
 *
 * function secondInterval(cb: () => void): number {
 *   return setInterval(cb, 1000);
 * }
 *
 * Deno.test("secondInterval calls callback every second and stops after being cleared", () => {
 *   const time = new FakeTime();
 *
 *   try {
 *     const cb = spy();
 *     const intervalId = secondInterval(cb);
 *     assertSpyCalls(cb, 0);
 *     time.tick(500);
 *     assertSpyCalls(cb, 0);
 *     time.tick(500);
 *     assertSpyCalls(cb, 1);
 *     time.tick(3500);
 *     assertSpyCalls(cb, 4);
 *
 *     clearInterval(intervalId);
 *     time.tick(1000);
 *     assertSpyCalls(cb, 4);
 *   } finally {
 *     time.restore();
 *   }
 * });
 * ```
 */
export class FakeTime {
  constructor(
    start?: number | string | Date | null,
    options?: FakeTimeOptions,
  ) {
    if (time) time.restore();
    initializedAt = _internals.Date.now();
    startedAt = start instanceof Date
      ? start.valueOf()
      : typeof start === "number"
      ? Math.floor(start)
      : typeof start === "string"
      ? (new Date(start)).valueOf()
      : initializedAt;
    if (Number.isNaN(startedAt)) throw new TimeError("invalid start");
    now = startedAt;

    timerId = timerIdGen();
    dueNodes = new Map();
    dueTree = new RedBlackTree(
      (a: DueNode, b: DueNode) => ascend(a.due, b.due),
    );

    overrideGlobals();
    time = this;

    advanceRate = Math.max(
      0,
      options?.advanceRate ? options.advanceRate : 0,
    );
    advanceFrequency = Math.max(
      0,
      options?.advanceFrequency ? options.advanceFrequency : 10,
    );
    advanceIntervalId = advanceRate > 0
      ? _internals.setInterval.call(null, () => {
        this.tick(advanceRate * advanceFrequency);
      }, advanceFrequency)
      : undefined;
  }

  /** Restores real time. */
  static restore() {
    if (!time) throw new TimeError("time already restored");
    time.restore();
  }

  /**
   * Restores real time temporarily until callback returns and resolves.
   */
  static restoreFor<T>(
    // deno-lint-ignore no-explicit-any
    callback: (...args: any[]) => Promise<T> | T,
    // deno-lint-ignore no-explicit-any
    ...args: any[]
  ): Promise<T> {
    if (!time) return Promise.reject(new TimeError("no fake time"));
    restoreGlobals();
    try {
      const result = callback.apply(null, args);
      if (result instanceof Promise) {
        return result.finally(() => overrideGlobals());
      } else {
        overrideGlobals();
        return Promise.resolve(result);
      }
    } catch (e) {
      overrideGlobals();
      return Promise.reject(e);
    }
  }

  /**
   * The amount of milliseconds elapsed since January 1, 1970 00:00:00 UTC for the fake time.
   * When set, it will call any functions waiting to be called between the current and new fake time.
   * If the timer callback throws, time will stop advancing forward beyond that timer.
   */
  get now(): number {
    return now;
  }
  set now(value: number) {
    if (value < now) throw new Error("time cannot go backwards");
    let dueNode: DueNode | null = dueTree.min();
    while (dueNode && dueNode.due <= value) {
      const timer: Timer | undefined = dueNode.timers.shift();
      if (timer && dueNodes.has(timer.id)) {
        now = timer.due;
        if (timer.repeat) {
          const due: number = timer.due + timer.delay;
          let dueNode: DueNode | null = dueTree.find({ due } as DueNode);
          if (dueNode === null) {
            dueNode = { due, timers: [] };
            dueTree.insert(dueNode);
          }
          dueNode.timers.push({ ...timer, due });
          dueNodes.set(timer.id, dueNode);
        } else {
          dueNodes.delete(timer.id);
        }
        timer.callback.apply(null, timer.args);
      } else if (!timer) {
        dueTree.remove(dueNode);
        dueNode = dueTree.min();
      }
    }
    now = value;
  }

  /** The initial amount of milliseconds elapsed since January 1, 1970 00:00:00 UTC for the fake time. */
  get start(): number {
    return startedAt;
  }
  set start(value: number) {
    throw new Error("cannot change start time after initialization");
  }

  /** Resolves after the given number of milliseconds using real time. */
  async delay(ms: number, options: DelayOptions = {}): Promise<void> {
    const { signal } = options;
    if (signal?.aborted) {
      return Promise.reject(
        new DOMException("Delay was aborted.", "AbortError"),
      );
    }
    return await new Promise((resolve, reject) => {
      let timer: number | null = null;
      const abort = () =>
        FakeTime
          .restoreFor(() => {
            if (timer) clearTimeout(timer);
          })
          .then(() =>
            reject(new DOMException("Delay was aborted.", "AbortError"))
          );
      const done = () => {
        signal?.removeEventListener("abort", abort);
        resolve();
      };
      FakeTime.restoreFor(() => setTimeout(done, ms))
        .then((id) => timer = id);
      signal?.addEventListener("abort", abort, { once: true });
    });
  }

  /** Runs all pending microtasks. */
  async runMicrotasks() {
    await this.delay(0);
  }

  /**
   * Adds the specified number of milliseconds to the fake time.
   * This will call any functions waiting to be called between the current and new fake time.
   */
  tick(ms = 0) {
    this.now += ms;
  }

  /**
   * Runs all pending microtasks then adds the specified number of milliseconds to the fake time.
   * This will call any functions waiting to be called between the current and new fake time.
   */
  async tickAsync(ms = 0) {
    await this.runMicrotasks();
    this.now += ms;
  }

  /**
   * Advances time to when the next scheduled timer is due.
   * If there are no pending timers, time will not be changed.
   * Returns true when there is a scheduled timer and false when there is not.
   */
  next(): boolean {
    const next = nextDueNode();
    if (next) this.now = next.due;
    return !!next;
  }

  /**
   * Runs all pending microtasks then advances time to when the next scheduled timer is due.
   * If there are no pending timers, time will not be changed.
   */
  async nextAsync(): Promise<boolean> {
    await this.runMicrotasks();
    return this.next();
  }

  /**
   * Advances time forward to the next due timer until there are no pending timers remaining.
   * If the timers create additional timers, they will be run too. If there is an interval,
   * time will keep advancing forward until the interval is cleared.
   */
  runAll() {
    while (!dueTree.isEmpty()) {
      this.next();
    }
  }

  /**
   * Advances time forward to the next due timer until there are no pending timers remaining.
   * If the timers create additional timers, they will be run too. If there is an interval,
   * time will keep advancing forward until the interval is cleared.
   * Runs all pending microtasks before each timer.
   */
  async runAllAsync() {
    while (!dueTree.isEmpty()) {
      await this.nextAsync();
    }
  }

  /** Restores time related global functions to their original state. */
  restore() {
    if (!time) throw new TimeError("time already restored");
    time = undefined;
    restoreGlobals();
    if (advanceIntervalId) clearInterval(advanceIntervalId);
  }
}
