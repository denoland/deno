System.register(
  "$deno$/web/timers.ts",
  ["$deno$/util.ts", "$deno$/ops/timers.ts", "$deno$/rbtree.ts"],
  function (exports_20, context_20) {
    "use strict";
    let util_ts_4,
      timers_ts_1,
      rbtree_ts_1,
      console,
      TIMEOUT_MAX,
      globalTimeoutDue,
      nextTimerId,
      idMap,
      dueTree,
      pendingEvents,
      pendingFireTimers;
    const __moduleName = context_20 && context_20.id;
    function clearGlobalTimeout() {
      globalTimeoutDue = null;
      timers_ts_1.stopGlobalTimer();
    }
    /** Process and run a single ready timer macrotask.
     * This function should be registered through Deno.core.setMacrotaskCallback.
     * Returns true when all ready macrotasks have been processed, false if more
     * ready ones are available. The Isolate future would rely on the return value
     * to repeatedly invoke this function until depletion. Multiple invocations
     * of this function one at a time ensures newly ready microtasks are processed
     * before next macrotask timer callback is invoked. */
    function handleTimerMacrotask() {
      if (pendingFireTimers.length > 0) {
        fire(pendingFireTimers.shift());
        return pendingFireTimers.length === 0;
      }
      return true;
    }
    exports_20("handleTimerMacrotask", handleTimerMacrotask);
    async function setGlobalTimeout(due, now) {
      // Since JS and Rust don't use the same clock, pass the time to rust as a
      // relative time value. On the Rust side we'll turn that into an absolute
      // value again.
      const timeout = due - now;
      util_ts_4.assert(timeout >= 0);
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
      await timers_ts_1.startGlobalTimer(timeout);
      pendingEvents--;
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      prepareReadyTimers();
    }
    function prepareReadyTimers() {
      const now = Date.now();
      // Bail out if we're not expecting the global timer to fire.
      if (globalTimeoutDue === null || pendingEvents > 0) {
        return;
      }
      // After firing the timers that are due now, this will hold the first timer
      // list that hasn't fired yet.
      let nextDueNode;
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
    function setOrClearGlobalTimeout(due, now) {
      if (due == null) {
        clearGlobalTimeout();
      } else {
        setGlobalTimeout(due, now);
      }
    }
    function schedule(timer, now) {
      util_ts_4.assert(!timer.scheduled);
      util_ts_4.assert(now <= timer.due);
      // Find or create the list of timers that will fire at point-in-time `due`.
      const maybeNewDueNode = { due: timer.due, timers: [] };
      let dueNode = dueTree.find(maybeNewDueNode);
      if (dueNode === null) {
        dueTree.insert(maybeNewDueNode);
        dueNode = maybeNewDueNode;
      }
      // Append the newly scheduled timer to the list and mark it as scheduled.
      dueNode.timers.push(timer);
      timer.scheduled = true;
      // If the new timer is scheduled to fire before any timer that existed before,
      // update the global timeout to reflect this.
      if (globalTimeoutDue === null || globalTimeoutDue > timer.due) {
        setOrClearGlobalTimeout(timer.due, now);
      }
    }
    function unschedule(timer) {
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
      const list = dueTree.find(searchKey).timers;
      if (list.length === 1) {
        // Time timer is the only one in the list. Remove the entire list.
        util_ts_4.assert(list[0] === timer);
        dueTree.remove(searchKey);
        // If the unscheduled timer was 'next up', find when the next timer that
        // still exists is due, and update the global alarm accordingly.
        if (timer.due === globalTimeoutDue) {
          const nextDueNode = dueTree.min();
          setOrClearGlobalTimeout(nextDueNode && nextDueNode.due, Date.now());
        }
      } else {
        // Multiple timers that are due at the same point in time.
        // Remove this timer from the list.
        const index = list.indexOf(timer);
        util_ts_4.assert(index > -1);
        list.splice(index, 1);
      }
    }
    function fire(timer) {
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
    function checkThis(thisArg) {
      if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
        throw new TypeError("Illegal invocation");
      }
    }
    function checkBigInt(n) {
      if (typeof n === "bigint") {
        throw new TypeError("Cannot convert a BigInt value to a number");
      }
    }
    function setTimer(cb, delay, args, repeat) {
      // Bind `args` to the callback and bind `this` to globalThis(global).
      const callback = cb.bind(globalThis, ...args);
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
    function setTimeout(cb, delay = 0, ...args) {
      checkBigInt(delay);
      // @ts-ignore
      checkThis(this);
      return setTimer(cb, delay, args, false);
    }
    exports_20("setTimeout", setTimeout);
    function setInterval(cb, delay = 0, ...args) {
      checkBigInt(delay);
      // @ts-ignore
      checkThis(this);
      return setTimer(cb, delay, args, true);
    }
    exports_20("setInterval", setInterval);
    function clearTimer(id) {
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
    function clearTimeout(id = 0) {
      checkBigInt(id);
      if (id === 0) {
        return;
      }
      clearTimer(id);
    }
    exports_20("clearTimeout", clearTimeout);
    function clearInterval(id = 0) {
      checkBigInt(id);
      if (id === 0) {
        return;
      }
      clearTimer(id);
    }
    exports_20("clearInterval", clearInterval);
    return {
      setters: [
        function (util_ts_4_1) {
          util_ts_4 = util_ts_4_1;
        },
        function (timers_ts_1_1) {
          timers_ts_1 = timers_ts_1_1;
        },
        function (rbtree_ts_1_1) {
          rbtree_ts_1 = rbtree_ts_1_1;
        },
      ],
      execute: function () {
        console = globalThis.console;
        // Timeout values > TIMEOUT_MAX are set to 1.
        TIMEOUT_MAX = 2 ** 31 - 1;
        globalTimeoutDue = null;
        nextTimerId = 1;
        idMap = new Map();
        dueTree = new rbtree_ts_1.RBTree((a, b) => a.due - b.due);
        pendingEvents = 0;
        pendingFireTimers = [];
      },
    };
  }
);
