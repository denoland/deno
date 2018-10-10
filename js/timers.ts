// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import { setFireTimersCallback } from "./dispatch";

// Tell the dispatcher which function it should call to fire timers that are
// due. This is done using a callback because circular imports are disallowed.
setFireTimersCallback(fireTimers);

interface Timer {
  id: number;
  callback: () => void;
  delay: number;
  due: number;
  repeat: boolean;
}

// We'll subtract EPOCH every time we retrieve the time with Date.now(). This
// ensures that absolute time values stay below UINT32_MAX - 2, which is the
// maximum object key that EcmaScript considers "numerical". After running for
// about a month, this is no longer true, and Deno explodes.
// TODO(piscisaureus): fix that ^.
const EPOCH = Date.now();
const APOCALYPS = 2 ** 32 - 2;

let nextTimerId = 1;
const idMap = new Map<number, Timer>();
const dueMap: { [due: number]: Timer[] } = Object.create(null);

function getTime() {
  // TODO: use a monotonic clock.
  const now = Date.now() - EPOCH;
  assert(now >= 0 && now < APOCALYPS);
  return now;
}

function schedule(timer: Timer) {
  // Find or create the list of timers that will fire at point-in-time `due`.
  let list = dueMap[timer.due];
  if (list === undefined) {
    list = dueMap[timer.due] = [];
  }
  // Append the newly scheduled timer to the list and mark it as scheduled.
  list.push(timer);
}

function unschedule(timer: Timer) {
  idMap.delete(timer.id);
  // Find the list of timers that will fire at point-in-time `due`.
  const list = dueMap[timer.due];
  if (list == null) {
    return;
  }
  if (list.length === 1) {
    // Time timer is the only one in the list. Remove the entire list.
    assert(list[0] === timer);
    delete dueMap[timer.due];
  } else {
    // Multiple timers that are due at the same point in time.
    // Remove this timer from the list.
    const index = list.indexOf(timer);
    assert(index > -1);
    list.splice(index, 1);
  }
}

function fire(timer: Timer) {
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
    timer.due = timer.due + timer.delay;
    schedule(timer);
  }
  // Call the user callback. Intermediate assignment is to avoid leaking `this`
  // to it, while also keeping the stack trace neat when it shows up in there.
  const callback = timer.callback;
  callback();
}

// Returns negative number  if there are no pending timers.
// Returns positive number indicating the number of milliseconds that must
// be waited until the next timer will fire.
function fireTimers(): number {
  const now = getTime();
  // Walk over the keys of the 'due' map. Since dueMap is actually a regular
  // object and its keys are numerical and smaller than UINT32_MAX - 2,
  // keys are iterated in ascending order.
  for (const key in dueMap) {
    // Convert the object key (a string) to a number.
    const due = Number(key);
    // Break out of the loop if the next timer isn't due to fire yet.
    if (due > now) {
      return due - now;
    }
    // Get the list of timers that have this due time, then drop it.
    const list = dueMap[key];
    delete dueMap[key];
    // Fire all the timers in the list.
    for (const timer of list) {
      fire(timer);
    }
  }
  return -1; // Wait forever.
}

function setTimer<Args extends Array<unknown>>(
  cb: (...args: Args) => void,
  delay: number,
  args: Args,
  repeat: boolean
): number {
  // If any `args` were provided (which is uncommon), bind them to the callback.
  const callback: () => void = args.length === 0 ? cb : cb.bind(null, ...args);
  // In the browser, the delay value must be coercable to an integer between 0
  // and INT32_MAX. Any other value will cause the timer to fire immediately.
  // We emulate this behavior.
  const now = getTime();
  delay = Math.max(0, delay | 0);
  // Create a new, unscheduled timer object.
  const timer = {
    id: nextTimerId++,
    callback,
    args,
    delay,
    due: now + delay,
    repeat
  };
  // Register the timer's existence in the id-to-timer map.
  idMap.set(timer.id, timer);
  // Schedule the timer in the due table.
  schedule(timer);
  return timer.id;
}

/** Sets a timer which executes a function once after the timer expires. */
export function setTimeout<Args extends Array<unknown>>(
  cb: (...args: Args) => void,
  delay: number,
  ...args: Args
): number {
  return setTimer(cb, delay, args, false);
}

/** Repeatedly calls a function , with a fixed time delay between each call. */
export function setInterval<Args extends Array<unknown>>(
  cb: (...args: Args) => void,
  delay: number,
  ...args: Args
): number {
  return setTimer(cb, delay, args, true);
}

/** Clears a previously set timer by id. */
export function clearTimer(id: number): void {
  const timer = idMap.get(id);
  if (timer === undefined) {
    // Timer doesn't exist any more or never existed. This is not an error.
    return;
  }
  // Unschedule the timer if it is currently scheduled, and forget about it.
  unschedule(timer);
}
