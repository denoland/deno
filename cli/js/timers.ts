// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util.ts";
import { window } from "./window.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { RBTree } from "./rbtree.ts";

const { console } = window;

/**
 * This module implements timeouts and intervals.
 * 
 * Both of them are internally defined as `Timer` interface
 * seen below. Timers are stored in red-black tree, grouped
 * by `due` field (representing time instant in which they should fire).
 * Intervals are represented as timeouts that are scheduled after
 * previous timeout if fired.
 * 
 * For each node of RB tree there's a single "timeout resource" created
 * in Rust - meaning that multiple timeouts scheduled for same instant
 * are represented by single resource in Rust.
 * 
 * Whole situation with RBTree is not optimal and we'd like
 * to move to solution where we have single Rust resource corresponding
 * to single JS timeout/interval. However, this is not yet possible
 * because Tokio doesn't support ordering of delays (meaning if you schedule
 * a few timeouts for single instant they will fire in random order).
 * 
 * This modules should be revamped once Tokio preserves ordering of delays.
 */

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
let nextTimerId = 1;
const idMap = new Map<number, Timer>();
type DueNode = { due: number; timers: Timer[]; rid: number };
const dueTree = new RBTree<DueNode>((a, b) => a.due - b.due);

async function pollTimeout(rid: number, due: number): Promise<void> {
  try {
    await sendAsync(dispatch.OP_POLL_TIMEOUT, { rid });
  } catch (e) {
    // Above op will throw if timeout was removed from resource
    // table before op was sent.
  }
  fireTimers(due);
}

function schedule(timer: Timer, now: number): void {
  assert(!timer.scheduled);
  assert(now <= timer.due);
  // Since JS and Rust don't use the same clock, pass the time to rust as a
  // relative time value. On the Rust side we'll turn that into an absolute
  // value again.
  const timeout = timer.due - now;
  // Find or create the list of timers that will fire at point-in-time `due`.
  const maybeNewDueNode = { due: timer.due, rid: null, timers: [] };
  let dueNode = dueTree.find(maybeNewDueNode);
  if (dueNode === null) {
    dueTree.insert(maybeNewDueNode);
    dueNode = maybeNewDueNode;
  }
  // Append the newly scheduled timer to the list and mark it as scheduled.
  dueNode!.timers.push(timer);
  timer.scheduled = true;

  // If tree node has no rid that means that timeout wasn't yet scheduled in Rust - do it now
  if (!dueNode.rid) {
    dueNode.rid = sendSync(dispatch.OP_SET_TIMEOUT, { timeout });
    pollTimeout(dueNode.rid, dueNode.due);
  }
}

function unschedule(timer: Timer): void {
  if (!timer.scheduled) {
    return;
  }
  const searchKey = { due: timer.due, rid: 0, timers: [] };
  // Find the list of timers that will fire at point-in-time `due`.
  const dueNode = dueTree.find(searchKey)!;
  const timers = dueNode.timers;
  if (timers.length === 1) {
    // Time timer is the only one in the list. Remove the entire list and send op to cancel timeout.
    assert(timers[0] === timer);
    dueTree.remove(searchKey);
    try {
      sendSync(dispatch.OP_CLEAR_TIMEOUT, { rid: dueNode.rid });
    } catch (e) {
      // Above op will throw if timeout fired before op was sent.
    }
  } else {
    // Multiple timers that are due at the same point in time.
    // Remove this timer from the list.
    const index = timers.indexOf(timer);
    assert(index > -1);
    timers.splice(index, 1);
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

function fireTimers(due: number): void {
  const dueNode = dueTree.find({ due, rid: 0, timers: [] });
  // All timeouts removed before this function fired
  if (!dueNode) {
    return;
  }

  dueTree.remove(dueNode);
  // Fire all the timers in the list.
  for (const timer of dueNode.timers) {
    // With the list dropped, the timer is no longer scheduled.
    timer.scheduled = false;
    // Place the callback on the microtask queue.
    Promise.resolve(timer).then(fire);
  }
}

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

function setTimer(
  cb: (...args: Args) => void,
  delay: number,
  args: Args,
  repeat: boolean
): number {
  // Bind `args` to the callback and bind `this` to window(global).
  const callback: () => void = cb.bind(window, ...args);
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
    scheduled: false
  };
  // Register the timer's existence in the id-to-timer map.
  idMap.set(timer.id, timer);
  // Schedule the timer in the due table.
  schedule(timer, now);
  return timer.id;
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
  return setTimer(cb, delay, args, false);
}

/** Repeatedly calls a function , with a fixed time delay between each call. */
export function setInterval(
  cb: (...args: Args) => void,
  delay = 0,
  ...args: Args
): number {
  checkBigInt(delay);
  // @ts-ignore
  checkThis(this);
  return setTimer(cb, delay, args, true);
}

/** Clears a previously set timer by id. AKA clearTimeout and clearInterval. */
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
