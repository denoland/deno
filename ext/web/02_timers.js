// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    ArrayPrototypePush,
    ArrayPrototypeShift,
    FunctionPrototypeCall,
    Map,
    MapPrototypeDelete,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeSet,
    Uint8Array,
    Uint32Array,
    // deno-lint-ignore camelcase
    NumberPOSITIVE_INFINITY,
    PromisePrototypeThen,
    ObjectPrototypeIsPrototypeOf,
    SafeArrayIterator,
    SymbolFor,
    TypeError,
    indirectEval,
  } = window.__bootstrap.primordials;
  const { webidl } = window.__bootstrap;
  const { reportException } = window.__bootstrap.event;
  const { assert } = window.__bootstrap.infra;

  const hrU8 = new Uint8Array(8);
  const hr = new Uint32Array(hrU8.buffer);
  function opNow() {
    ops.op_now.fast(hrU8);
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
    if (timerTasks.length === 0) {
      return true;
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
   * @type {Map<number, { cancelRid: number, isRef: boolean, promiseId: number }>}
   */
  const activeTimers = new Map();

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
      timerInfo = { cancelRid, isRef: true, promiseId: -1 };

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
    if (timerNestingLevel > 5 && timeout < 4) timeout = 4;

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
      () => ArrayPrototypePush(timerTasks, task),
      timeout,
      timerInfo,
    );

    return id;
  }

  // ---------------------------------------------------------------------------

  /**
   * @typedef ScheduledTimer
   * @property {number} millis
   * @property {() => void} cb
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
   * @param {() => void} cb Will be run after the timeout, if it hasn't been
   * cancelled.
   * @param {number} millis
   * @param {{ cancelRid: number, isRef: boolean, promiseId: number }} timerInfo
   */
  function runAfterTimeout(cb, millis, timerInfo) {
    const cancelRid = timerInfo.cancelRid;
    const sleepPromise = core.opAsync("op_sleep", millis, cancelRid);
    timerInfo.promiseId =
      sleepPromise[SymbolFor("Deno.core.internalPromiseId")];
    if (!timerInfo.isRef) {
      core.unrefOp(timerInfo.promiseId);
    }

    /** @type {ScheduledTimer} */
    const timerObject = {
      millis,
      cb,
      resolved: false,
      prev: scheduledTimers.tail,
      next: null,
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
      () => {
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

        timerObject.resolved = true;

        let lowestUnresolvedTimeout = NumberPOSITIVE_INFINITY;

        let currentEntry = scheduledTimers.head;
        while (currentEntry !== null) {
          if (currentEntry.millis < lowestUnresolvedTimeout) {
            if (currentEntry.resolved) {
              currentEntry.cb();
              removeFromScheduledTimers(currentEntry);
            } else {
              lowestUnresolvedTimeout = currentEntry.millis;
            }
          }

          currentEntry = currentEntry.next;
        }
      },
      (err) => {
        if (ObjectPrototypeIsPrototypeOf(core.InterruptedPrototype, err)) {
          // The timer was cancelled.
          removeFromScheduledTimers(timerObject);
        } else {
          throw err;
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
    core.refOp(timerInfo.promiseId);
  }

  function unrefTimer(id) {
    const timerInfo = MapPrototypeGet(activeTimers, id);
    if (timerInfo === undefined || !timerInfo.isRef) {
      return;
    }
    timerInfo.isRef = false;
    core.unrefOp(timerInfo.promiseId);
  }

  window.__bootstrap.timers = {
    setTimeout,
    setInterval,
    clearTimeout,
    clearInterval,
    handleTimerMacrotask,
    opNow,
    refTimer,
    unrefTimer,
  };
})(this);
