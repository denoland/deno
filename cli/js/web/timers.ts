// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../util.ts";
import { startGlobalTimer, stopGlobalTimer } from "../ops/timers.ts";
import { RBTree } from "../rbtree.ts";

const { console } = globalThis;

interface Timer {
  id: number;
  callback: () => void;
  delay: number;
  due: number;
  repeat: boolean;
  scheduled: boolean;
}

// Timeout values > TIMEOUT_MAX are set to 1.
const TIMEOUT_MAX = 2 ** 31 - 1;

let globalTimeoutDue: number | null = null;

let nextTimerId = 1;
const idMap = new Map<number, Timer>();
type DueNode = { due: number; timers: Timer[] };
const dueTree = new RBTree<DueNode>((a, b) => a.due - b.due);

function clearGlobalTimeout(): void {
  globalTimeoutDue = null;
  stopGlobalTimer();
}

let pendingEvents = 0;
const pendingFireTimers: Timer[] = [];

/** Process and run a single ready timer macrotask.
 * This function should be registered through Deno.core.setMacrotaskCallback.
 * Returns true when all ready macrotasks have been processed, false if more
 * ready ones are available. The Isolate future would rely on the return value
 * to repeatedly invoke this function until depletion. Multiple invocations
 * of this function one at a time ensures newly ready microtasks are processed
 * before next macrotask timer callback is invoked. */
export function handleTimerMacrotask(): boolean {
  if (pendingFireTimers.length > 0) {
    fire(pendingFireTimers.shift()!);
    return pendingFireTimers.length === 0;
  }
  return true;
}

async function setGlobalTimeout(due: number, now: number): Promise<void> {
  // Since JS and Rust don't use the same clock, pass the time to rust as a
  // relative time value. On the Rust side we'll turn that into an absolute
  // value again.
  const timeout = due - now;
  assert(timeout >= 0);
  // Send message to the backend.
  globalTimeoutDue = due;
  pendingEvents++;
  // FIXME(bartlomieju): this is problematic, because `clearGlobalTimeout`
  // is synchronous. That means that timer is cancelled, but this promise is still pending
  // until next turn of event loop. This leads to "leaking of async ops" in tests;
  // because `clearTimeout/clearInterval` might be the last statement in test function
  // `opSanitizer` will immediately complain that there is pending op going on, unless
  // some timeout/defer is put in place to allow promise resolution.
  // Ideally `clearGlobalTimeout` doesn't return until this op is resolved, but
  // I'm not if that's possible.
  await startGlobalTimer(timeout);
  pendingEvents--;
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  prepareReadyTimers();
}

function prepareReadyTimers(): void {
  const now = Date.now();
  // Bail out if we're not expecting the global timer to fire.
  if (globalTimeoutDue === null || pendingEvents > 0) {
    return;
  }
  // After firing the timers that are due now, this will hold the first timer
  // list that hasn't fired yet.
  let nextDueNode: DueNode | null;
  while ((nextDueNode = dueTree.min()) !== null && nextDueNode.due <= now) {
    dueTree.remove(nextDueNode);
    // Fire all the timers in the list.
    for (const timer of nextDueNode.timers) {
      // With the list dropped, the timer is no longer scheduled.
      timer.scheduled = false;
      // Place the callback to pending timers to fire.
      pendingFireTimers.push(timer);
    }
  }
  setOrClearGlobalTimeout(nextDueNode && nextDueNode.due, now);
}

function setOrClearGlobalTimeout(due: number | null, now: number): void {
  if (due == null) {
    clearGlobalTimeout();
  } else {
    setGlobalTimeout(due, now);
  }
}

function schedule(timer: Timer, now: number): void {
  assert(!timer.scheduled);
  assert(now <= timer.due);
  // Find or create the list of timers that will fire at point-in-time `due`.
  const maybeNewDueNode = { due: timer.due, timers: [] };
  let dueNode = dueTree.find(maybeNewDueNode);
  if (dueNode === null) {
    dueTree.insert(maybeNewDueNode);
    dueNode = maybeNewDueNode;
  }
  // Append the newly scheduled timer to the list and mark it as scheduled.
  dueNode!.timers.push(timer);
  timer.scheduled = true;
  // If the new timer is scheduled to fire before any timer that existed before,
  // update the global timeout to reflect this.
  if (globalTimeoutDue === null || globalTimeoutDue > timer.due) {
    setOrClearGlobalTimeout(timer.due, now);
  }
}

function unschedule(timer: Timer): void {
  // Check if our timer is pending scheduling or pending firing.
  // If either is true, they are not in tree, and their idMap entry
  // will be deleted soon. Remove it from queue.
  let index = -1;
  if ((index = pendingFireTimers.indexOf(timer)) >= 0) {
    pendingFireTimers.splice(index);
    return;
  }
  // If timer is not in the 2 pending queues and is unscheduled,
  // it is not in the tree.
  if (!timer.scheduled) {
    return;
  }
  const searchKey = { due: timer.due, timers: [] };
  // Find the list of timers that will fire at point-in-time `due`.
  const list = dueTree.find(searchKey)!.timers;
  if (list.length === 1) {
    // Time timer is the only one in the list. Remove the entire list.
    assert(list[0] === timer);
    dueTree.remove(searchKey);
    // If the unscheduled timer was 'next up', find when the next timer that
    // still exists is due, and update the global alarm accordingly.
    if (timer.due === globalTimeoutDue) {
      const nextDueNode: DueNode | null = dueTree.min();
      setOrClearGlobalTimeout(nextDueNode && nextDueNode.due, Date.now());
    }
  } else {
    // Multiple timers that are due at the same point in time.
    // Remove this timer from the list.
    const index = list.indexOf(timer);
    assert(index > -1);
    list.splice(index, 1);
  }
}

function fire(timer: Timer): void {
  // If the timer isn't found in the ID map, that means it has been cancelled
  // between the timer firing and the promise callback (this function).
  if (!idMap.has(timer.id)) {
    return;
  }
  // Reschedule the timer if it is a repeating one, otherwise drop it.
  if (!timer.repeat) {
    // One-shot timer: remove the timer from this id-to-timer map.
    idMap.delete(timer.id);
  } else {
    // Interval timer: compute when timer was supposed to fire next.
    // However make sure to never schedule the next interval in the past.
    const now = Date.now();
    timer.due = Math.max(now, timer.due + timer.delay);
    schedule(timer, now);
  }
  // Call the user callback. Intermediate assignment is to avoid leaking `this`
  // to it, while also keeping the stack trace neat when it shows up in there.
  const callback = timer.callback;
  callback();
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Args = any[];

function checkThis(thisArg: unknown): void {
  if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
    throw new TypeError("Illegal invocation");
  }
}

function checkBigInt(n: unknown): void {
  if (typeof n === "bigint") {
    throw new TypeError("Cannot convert a BigInt value to a number");
  }
}

function setTimer(
  cb: (...args: Args) => void,
  delay: number,
  args: Args,
  repeat: boolean
): number {
  // Bind `args` to the callback and bind `this` to globalThis(global).
  const callback: () => void = cb.bind(globalThis, ...args);
  // In the browser, the delay value must be coercible to an integer between 0
  // and INT32_MAX. Any other value will cause the timer to fire immediately.
  // We emulate this behavior.
  const now = Date.now();
  if (delay > TIMEOUT_MAX) {
    console.warn(
      `${delay} does not fit into` +
        " a 32-bit signed integer." +
        "\nTimeout duration was set to 1."
    );
    delay = 1;
  }
  delay = Math.max(0, delay | 0);

  // Create a new, unscheduled timer object.
  const timer = {
    id: nextTimerId++,
    callback,
    args,
    delay,
    due: now + delay,
    repeat,
    scheduled: false,
  };
  // Register the timer's existence in the id-to-timer map.
  idMap.set(timer.id, timer);
  // Schedule the timer in the due table.
  schedule(timer, now);
  return timer.id;
}

export function setTimeout(
  this: unknown,
  cb: (...args: Args) => void,
  delay = 0,
  ...args: Args
): number {
  checkBigInt(delay);
  checkThis(this);
  return setTimer(cb, delay, args, false);
}

export function setInterval(
  this: unknown,
  cb: (...args: Args) => void,
  delay = 0,
  ...args: Args
): number {
  checkBigInt(delay);
  checkThis(this);
  return setTimer(cb, delay, args, true);
}

function clearTimer(id: number): void {
  id = Number(id);
  const timer = idMap.get(id);
  if (timer === undefined) {
    // Timer doesn't exist any more or never existed. This is not an error.
    return;
  }
  // Unschedule the timer if it is currently scheduled, and forget about it.
  unschedule(timer);
  idMap.delete(timer.id);
}

export function clearTimeout(id = 0): void {
  checkBigInt(id);
  if (id === 0) {
    return;
  }
  clearTimer(id);
}

export function clearInterval(id = 0): void {
  checkBigInt(id);
  if (id === 0) {
    return;
  }
  clearTimer(id);
}
