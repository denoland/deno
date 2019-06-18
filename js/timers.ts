// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { sendAsync, sendSync } from "./dispatch";
import { window } from "./window";

interface Timer {
  id: number;
  callback: () => void;
  delay: number;
  due: number;
  repeat: boolean;
  scheduled: boolean;
}

// We'll subtract EPOCH every time we retrieve the time with Date.now(). This
// ensures that absolute time values stay below UINT32_MAX - 2, which is the
// maximum object key that EcmaScript considers "numerical". After running for
// about a month, this is no longer true, and Deno explodes.
// TODO(piscisaureus): fix that ^.
const EPOCH = Date.now();
const APOCALYPSE = 2 ** 32 - 2;

// Timeout values > TIMEOUT_MAX are set to 1.
const TIMEOUT_MAX = 2 ** 31 - 1;

let globalTimeoutDue: number | null = null;

let nextTimerId = 1;
const idMap = new Map<number, Timer>();
const dueMap: { [due: number]: Timer[] } = Object.create(null);

function getTime(): number {
  // TODO: use a monotonic clock.
  const now = Date.now() - EPOCH;
  assert(now >= 0 && now < APOCALYPSE);
  return now;
}

function clearGlobalTimeout(): void {
  const builder = flatbuffers.createBuilder();
  const inner = msg.GlobalTimerStop.createGlobalTimerStop(builder);
  globalTimeoutDue = null;
  let res = sendSync(builder, msg.Any.GlobalTimerStop, inner);
  assert(res == null);
}

async function setGlobalTimeout(due: number, now: number): Promise<void> {
  // Since JS and Rust don't use the same clock, pass the time to rust as a
  // relative time value. On the Rust side we'll turn that into an absolute
  // value again.
  let timeout = due - now;
  assert(timeout >= 0);

  // Send message to the backend.
  const builder = flatbuffers.createBuilder();
  msg.GlobalTimer.startGlobalTimer(builder);
  msg.GlobalTimer.addTimeout(builder, timeout);
  const inner = msg.GlobalTimer.endGlobalTimer(builder);
  globalTimeoutDue = due;
  await sendAsync(builder, msg.Any.GlobalTimer, inner);
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  fireTimers();
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
  let list = dueMap[timer.due];
  if (list === undefined) {
    list = dueMap[timer.due] = [];
  }
  // Append the newly scheduled timer to the list and mark it as scheduled.
  list.push(timer);
  timer.scheduled = true;
  // If the new timer is scheduled to fire before any timer that existed before,
  // update the global timeout to reflect this.
  if (globalTimeoutDue === null || globalTimeoutDue > timer.due) {
    setOrClearGlobalTimeout(timer.due, now);
  }
}

function unschedule(timer: Timer): void {
  if (!timer.scheduled) {
    return;
  }
  // Find the list of timers that will fire at point-in-time `due`.
  const list = dueMap[timer.due];
  if (list.length === 1) {
    // Time timer is the only one in the list. Remove the entire list.
    assert(list[0] === timer);
    delete dueMap[timer.due];
    // If the unscheduled timer was 'next up', find when the next timer that
    // still exists is due, and update the global alarm accordingly.
    if (timer.due === globalTimeoutDue) {
      let nextTimerDue: number | null = null;
      for (const key in dueMap) {
        nextTimerDue = Number(key);
        break;
      }
      setOrClearGlobalTimeout(nextTimerDue, getTime());
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
    const now = getTime();
    timer.due = Math.max(now, timer.due + timer.delay);
    schedule(timer, now);
  }
  // Call the user callback. Intermediate assignment is to avoid leaking `this`
  // to it, while also keeping the stack trace neat when it shows up in there.
  const callback = timer.callback;
  callback();
}

function fireTimers(): void {
  const now = getTime();
  // Bail out if we're not expecting the global timer to fire (yet).
  if (globalTimeoutDue === null || now < globalTimeoutDue) {
    return;
  }
  // After firing the timers that are due now, this will hold the due time of
  // the first timer that hasn't fired yet.
  let nextTimerDue: number | null = null;
  // Walk over the keys of the 'due' map. Since dueMap is actually a regular
  // object and its keys are numerical and smaller than UINT32_MAX - 2,
  // keys are iterated in ascending order.
  for (const key in dueMap) {
    // Convert the object key (a string) to a number.
    const due = Number(key);
    // Break out of the loop if the next timer isn't due to fire yet.
    if (Number(due) > now) {
      nextTimerDue = due;
      break;
    }
    // Get the list of timers that have this due time, then drop it.
    const list = dueMap[key];
    delete dueMap[key];
    // Fire all the timers in the list.
    for (const timer of list) {
      // With the list dropped, the timer is no longer scheduled.
      timer.scheduled = false;
      // Place the callback on the microtask queue.
      Promise.resolve(timer).then(fire);
    }
  }

  // Update the global alarm to go off when the first-up timer that hasn't fired
  // yet is due.
  setOrClearGlobalTimeout(nextTimerDue, now);
}

export type Args = unknown[];

function checkThis(thisArg: unknown): void {
  if (thisArg !== null && thisArg !== undefined && thisArg !== window) {
    throw new TypeError("Illegal invocation");
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
  const now = getTime();
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
  delay: number,
  ...args: Args
): number {
  // @ts-ignore
  checkThis(this);
  return setTimer(cb, delay, args, false);
}

/** Repeatedly calls a function , with a fixed time delay between each call. */
export function setInterval(
  cb: (...args: Args) => void,
  delay: number,
  ...args: Args
): number {
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

export function clearTimeout(id: number): void {
  clearTimer(id);
}

export function clearInterval(id: number): void {
  clearTimer(id);
}
