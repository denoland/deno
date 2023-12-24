// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypePush,
  ArrayPrototypeShift,
  FunctionPrototypeCall,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  Uint8Array,
  Uint32Array,
  PromisePrototypeThen,
  SafeArrayIterator,
  SafeMap,
  TypedArrayPrototypeGetBuffer,
  TypeError,
  indirectEval,
} = primordials;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { reportException } from "ext:deno_web/02_event.js";
import { assert } from "ext:deno_web/00_infra.js";
const { op_sleep, op_void_async_deferred } = core.ensureFastOps();

const hrU8 = new Uint8Array(8);
const hr = new Uint32Array(TypedArrayPrototypeGetBuffer(hrU8));
function opNow() {
  ops.op_now(hrU8);
  return (hr[0] * 1000 + hr[1] / 1e6);
}

// ---------------------------------------------------------------------------

/**
 * The task queue corresponding to the timer task source.
 *
 * @type { {action: () => void, nestingLevel: number}[] }
 */
const timerTasks = [];

/**
 * The current task's timer nesting level, or zero if we're not currently
 * running a timer task (since the minimum nesting level is 1).
 *
 * @type {number}
 */
let timerNestingLevel = 0;

function handleTimerMacrotask() {
  // We have no work to do, tell the runtime that we don't
  // need to perform microtask checkpoint.
  if (timerTasks.length === 0) {
    return undefined;
  }

  const task = ArrayPrototypeShift(timerTasks);

  timerNestingLevel = task.nestingLevel;

  try {
    task.action();
  } finally {
    timerNestingLevel = 0;
  }
  return timerTasks.length === 0;
}

// ---------------------------------------------------------------------------

/**
 * The keys in this map correspond to the key ID's in the spec's map of active
 * timers. The values are the timeout's cancel rid.
 *
 * @type {Map<number, { cancelRid: number, isRef: boolean, promise: Promise<void> }>}
 */
const activeTimers = new SafeMap();

let nextId = 1;

/**
 * @param {Function | string} callback
 * @param {number} timeout
 * @param {Array<any>} args
 * @param {boolean} repeat
 * @param {number | undefined} prevId
 * @returns {number} The timer ID
 */
function initializeTimer(
  callback,
  timeout,
  args,
  repeat,
  prevId,
  // TODO(bartlomieju): remove this option, once `nextTick` and `setImmediate`
  // in Node compat are cleaned up
  respectNesting = true,
) {
  // 2. If previousId was given, let id be previousId; otherwise, let
  // previousId be an implementation-defined integer than is greater than zero
  // and does not already exist in global's map of active timers.
  let id;
  let timerInfo;
  if (prevId !== undefined) {
    // `prevId` is only passed for follow-up calls on intervals
    assert(repeat);
    id = prevId;
    timerInfo = MapPrototypeGet(activeTimers, id);
  } else {
    // TODO(@andreubotella): Deal with overflow.
    // https://github.com/whatwg/html/issues/7358
    id = nextId++;
    const cancelRid = ops.op_timer_handle();
    timerInfo = { cancelRid, isRef: true, promise: null };

    // Step 4 in "run steps after a timeout".
    MapPrototypeSet(activeTimers, id, timerInfo);
  }

  // 3. If the surrounding agent's event loop's currently running task is a
  // task that was created by this algorithm, then let nesting level be the
  // task's timer nesting level. Otherwise, let nesting level be zero.
  // 4. If timeout is less than 0, then set timeout to 0.
  // 5. If nesting level is greater than 5, and timeout is less than 4, then
  // set timeout to 4.
  //
  // The nesting level of 5 and minimum of 4 ms are spec-mandated magic
  // constants.
  if (timeout < 0) timeout = 0;
  if (timerNestingLevel > 5 && timeout < 4 && respectNesting) timeout = 4;

  // 9. Let task be a task that runs the following steps:
  const task = {
    action: () => {
      // 1. If id does not exist in global's map of active timers, then abort
      // these steps.
      //
      // This is relevant if the timer has been canceled after the sleep op
      // resolves but before this task runs.
      if (!MapPrototypeHas(activeTimers, id)) {
        return;
      }

      // 2.
      // 3.
      if (typeof callback === "function") {
        try {
          FunctionPrototypeCall(
            callback,
            globalThis,
            ...new SafeArrayIterator(args),
          );
        } catch (error) {
          reportException(error);
        }
      } else {
        indirectEval(callback);
      }

      if (repeat) {
        if (MapPrototypeHas(activeTimers, id)) {
          // 4. If id does not exist in global's map of active timers, then
          // abort these steps.
          // NOTE: If might have been removed via the author code in handler
          // calling clearTimeout() or clearInterval().
          // 5. If repeat is true, then perform the timer initialization steps
          // again, given global, handler, timeout, arguments, true, and id.
          initializeTimer(callback, timeout, args, true, id);
        }
      } else {
        // 6. Otherwise, remove global's map of active timers[id].
        core.tryClose(timerInfo.cancelRid);
        MapPrototypeDelete(activeTimers, id);
      }
    },

    // 10. Increment nesting level by one.
    // 11. Set task's timer nesting level to nesting level.
    nestingLevel: timerNestingLevel + 1,
  };

  // 12. Let completionStep be an algorithm step which queues a global task on
  // the timer task source given global to run task.
  // 13. Run steps after a timeout given global, "setTimeout/setInterval",
  // timeout, completionStep, and id.
  runAfterTimeout(
    task,
    timeout,
    timerInfo,
  );

  return id;
}

// ---------------------------------------------------------------------------

/**
 * @typedef ScheduledTimer
 * @property {number} millis
 * @property { {action: () => void, nestingLevel: number}[] } task
 * @property {boolean} resolved
 * @property {ScheduledTimer | null} prev
 * @property {ScheduledTimer | null} next
 */

/**
 * A doubly linked list of timers.
 * @type { { head: ScheduledTimer | null, tail: ScheduledTimer | null } }
 */
const scheduledTimers = { head: null, tail: null };

/**
 * @param { {action: () => void, nestingLevel: number}[] } task Will be run
 * after the timeout, if it hasn't been cancelled.
 * @param {number} millis
 * @param {{ cancelRid: number, isRef: boolean, promise: Promise<void> }} timerInfo
 */
function runAfterTimeout(task, millis, timerInfo) {
  const cancelRid = timerInfo.cancelRid;
  let sleepPromise;
  // If this timeout is scheduled for 0ms it means we want it to run at the
  // end of the event loop turn. There's no point in setting up a Tokio timer,
  // since its lowest resolution is 1ms. Firing of a "void async" op is better
  // in this case, because the timer will take closer to 0ms instead of >1ms.
  if (millis === 0) {
    sleepPromise = op_void_async_deferred();
  } else {
    sleepPromise = op_sleep(millis, cancelRid);
  }
  timerInfo.promise = sleepPromise;
  if (!timerInfo.isRef) {
    core.unrefOpPromise(timerInfo.promise);
  }

  /** @type {ScheduledTimer} */
  const timerObject = {
    millis,
    resolved: false,
    prev: scheduledTimers.tail,
    next: null,
    task,
  };

  // Add timerObject to the end of the list.
  if (scheduledTimers.tail === null) {
    assert(scheduledTimers.head === null);
    scheduledTimers.head = scheduledTimers.tail = timerObject;
  } else {
    scheduledTimers.tail.next = timerObject;
    scheduledTimers.tail = timerObject;
  }

  // 1.
  PromisePrototypeThen(
    sleepPromise,
    (cancelled) => {
      if (timerObject.resolved) {
        return;
      }

      // "op_void_async_deferred" returns null
      if (cancelled !== null && !cancelled) {
        // The timer was cancelled.
        removeFromScheduledTimers(timerObject);
        return;
      }
      // 2. Wait until any invocations of this algorithm that had the same
      // global and orderingIdentifier, that started before this one, and
      // whose milliseconds is equal to or less than this one's, have
      // completed.
      // 4. Perform completionSteps.

      // IMPORTANT: Since the sleep ops aren't guaranteed to resolve in the
      // right order, whenever one resolves, we run through the scheduled
      // timers list (which is in the order in which they were scheduled), and
      // we call the callback for every timer which both:
      //   a) has resolved, and
      //   b) its timeout is lower than the lowest unresolved timeout found so
      //      far in the list.

      let currentEntry = scheduledTimers.head;
      while (currentEntry !== null) {
        if (currentEntry.millis <= timerObject.millis) {
          currentEntry.resolved = true;
          ArrayPrototypePush(timerTasks, currentEntry.task);
          removeFromScheduledTimers(currentEntry);

          if (currentEntry === timerObject) {
            break;
          }
        }

        currentEntry = currentEntry.next;
      }
    },
  );
}

/** @param {ScheduledTimer} timerObj */
function removeFromScheduledTimers(timerObj) {
  if (timerObj.prev !== null) {
    timerObj.prev.next = timerObj.next;
  } else {
    assert(scheduledTimers.head === timerObj);
    scheduledTimers.head = timerObj.next;
  }
  if (timerObj.next !== null) {
    timerObj.next.prev = timerObj.prev;
  } else {
    assert(scheduledTimers.tail === timerObj);
    scheduledTimers.tail = timerObj.prev;
  }
}

// ---------------------------------------------------------------------------

function checkThis(thisArg) {
  if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
    throw new TypeError("Illegal invocation");
  }
}

function setTimeout(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    callback = webidl.converters.DOMString(callback);
  }
  timeout = webidl.converters.long(timeout);

  return initializeTimer(callback, timeout, args, false);
}

function setInterval(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    callback = webidl.converters.DOMString(callback);
  }
  timeout = webidl.converters.long(timeout);

  return initializeTimer(callback, timeout, args, true);
}

// TODO(bartlomieju): remove this option, once `nextTick` and `setImmediate`
// in Node compat are cleaned up
function setTimeoutUnclamped(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    callback = webidl.converters.DOMString(callback);
  }
  timeout = webidl.converters.long(timeout);

  return initializeTimer(callback, timeout, args, false, undefined, false);
}

function clearTimeout(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const timerInfo = MapPrototypeGet(activeTimers, id);
  if (timerInfo !== undefined) {
    core.tryClose(timerInfo.cancelRid);
    MapPrototypeDelete(activeTimers, id);
  }
}

function clearInterval(id = 0) {
  checkThis(this);
  clearTimeout(id);
}

function refTimer(id) {
  const timerInfo = MapPrototypeGet(activeTimers, id);
  if (timerInfo === undefined || timerInfo.isRef) {
    return;
  }
  timerInfo.isRef = true;
  core.refOpPromise(timerInfo.promise);
}

function unrefTimer(id) {
  const timerInfo = MapPrototypeGet(activeTimers, id);
  if (timerInfo === undefined || !timerInfo.isRef) {
    return;
  }
  timerInfo.isRef = false;
  core.unrefOpPromise(timerInfo.promise);
}

// Defer to avoid starving the event loop. Not using queueMicrotask()
// for that reason: it lets promises make forward progress but can
// still starve other parts of the event loop.
function defer(go) {
  PromisePrototypeThen(op_void_async_deferred(), () => go());
}

export {
  clearInterval,
  clearTimeout,
  defer,
  handleTimerMacrotask,
  opNow,
  refTimer,
  setInterval,
  setTimeout,
  setTimeoutUnclamped,
  unrefTimer,
};
