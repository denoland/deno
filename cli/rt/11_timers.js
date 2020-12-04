// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const assert = window.__bootstrap.util.assert;
  const core = window.Deno.core;
  const { sendSync } = window.__bootstrap.dispatchMinimal;

  function opStopGlobalTimer() {
    core.jsonOpSync("op_global_timer_stop");
  }

  function opStartGlobalTimer(timeout) {
    return core.jsonOpSync("op_global_timer_start", { timeout });
  }

  async function opWaitGlobalTimer() {
    await core.jsonOpAsync("op_global_timer");
  }

  const nowBytes = new Uint8Array(8);
  function opNow() {
    sendSync("op_now", 0, nowBytes);
    return new DataView(
      Uint8Array.from(nowBytes).buffer,
    )
      .getFloat64();
  }

  function sleepSync(millis = 0) {
    return core.jsonOpSync("op_sleep_sync", { millis });
  }

  // Derived from https://github.com/vadimg/js_bintrees. MIT Licensed.

  class RBNode {
    constructor(data) {
      this.data = data;
      this.left = null;
      this.right = null;
      this.red = true;
    }

    getChild(dir) {
      return dir ? this.right : this.left;
    }

    setChild(dir, val) {
      if (dir) {
        this.right = val;
      } else {
        this.left = val;
      }
    }
  }

  class RBTree {
    #comparator = null;
    #root = null;

    constructor(comparator) {
      this.#comparator = comparator;
      this.#root = null;
    }

    /** Returns `null` if tree is empty. */
    min() {
      let res = this.#root;
      if (res === null) {
        return null;
      }
      while (res.left !== null) {
        res = res.left;
      }
      return res.data;
    }

    /** Returns node `data` if found, `null` otherwise. */
    find(data) {
      let res = this.#root;
      while (res !== null) {
        const c = this.#comparator(data, res.data);
        if (c === 0) {
          return res.data;
        } else {
          res = res.getChild(c > 0);
        }
      }
      return null;
    }

    /** returns `true` if inserted, `false` if duplicate. */
    insert(data) {
      let ret = false;

      if (this.#root === null) {
        // empty tree
        this.#root = new RBNode(data);
        ret = true;
      } else {
        const head = new RBNode(null); // fake tree root

        let dir = 0;
        let last = 0;

        // setup
        let gp = null; // grandparent
        let ggp = head; // grand-grand-parent
        let p = null; // parent
        let node = this.#root;
        ggp.right = this.#root;

        // search down
        while (true) {
          if (node === null) {
            // insert new node at the bottom
            node = new RBNode(data);
            p.setChild(dir, node);
            ret = true;
          } else if (isRed(node.left) && isRed(node.right)) {
            // color flip
            node.red = true;
            node.left.red = false;
            node.right.red = false;
          }

          // fix red violation
          if (isRed(node) && isRed(p)) {
            const dir2 = ggp.right === gp;

            assert(gp);
            if (node === p.getChild(last)) {
              ggp.setChild(dir2, singleRotate(gp, !last));
            } else {
              ggp.setChild(dir2, doubleRotate(gp, !last));
            }
          }

          const cmp = this.#comparator(node.data, data);

          // stop if found
          if (cmp === 0) {
            break;
          }

          last = dir;
          dir = Number(cmp < 0); // Fix type

          // update helpers
          if (gp !== null) {
            ggp = gp;
          }
          gp = p;
          p = node;
          node = node.getChild(dir);
        }

        // update root
        this.#root = head.right;
      }

      // make root black
      this.#root.red = false;

      return ret;
    }

    /** Returns `true` if removed, `false` if not found. */
    remove(data) {
      if (this.#root === null) {
        return false;
      }

      const head = new RBNode(null); // fake tree root
      let node = head;
      node.right = this.#root;
      let p = null; // parent
      let gp = null; // grand parent
      let found = null; // found item
      let dir = 1;

      while (node.getChild(dir) !== null) {
        const last = dir;

        // update helpers
        gp = p;
        p = node;
        node = node.getChild(dir);

        const cmp = this.#comparator(data, node.data);

        dir = cmp > 0;

        // save found node
        if (cmp === 0) {
          found = node;
        }

        // push the red node down
        if (!isRed(node) && !isRed(node.getChild(dir))) {
          if (isRed(node.getChild(!dir))) {
            const sr = singleRotate(node, dir);
            p.setChild(last, sr);
            p = sr;
          } else if (!isRed(node.getChild(!dir))) {
            const sibling = p.getChild(!last);
            if (sibling !== null) {
              if (
                !isRed(sibling.getChild(!last)) &&
                !isRed(sibling.getChild(last))
              ) {
                // color flip
                p.red = false;
                sibling.red = true;
                node.red = true;
              } else {
                assert(gp);
                const dir2 = gp.right === p;

                if (isRed(sibling.getChild(last))) {
                  gp.setChild(dir2, doubleRotate(p, last));
                } else if (isRed(sibling.getChild(!last))) {
                  gp.setChild(dir2, singleRotate(p, last));
                }

                // ensure correct coloring
                const gpc = gp.getChild(dir2);
                assert(gpc);
                gpc.red = true;
                node.red = true;
                assert(gpc.left);
                gpc.left.red = false;
                assert(gpc.right);
                gpc.right.red = false;
              }
            }
          }
        }
      }

      // replace and remove if found
      if (found !== null) {
        found.data = node.data;
        assert(p);
        p.setChild(p.right === node, node.getChild(node.left === null));
      }

      // update root and make it black
      this.#root = head.right;
      if (this.#root !== null) {
        this.#root.red = false;
      }

      return found !== null;
    }
  }

  function isRed(node) {
    return node !== null && node.red;
  }

  function singleRotate(root, dir) {
    const save = root.getChild(!dir);
    assert(save);

    root.setChild(!dir, save.getChild(dir));
    save.setChild(dir, root);

    root.red = true;
    save.red = false;

    return save;
  }

  function doubleRotate(root, dir) {
    root.setChild(!dir, singleRotate(root.getChild(!dir), !dir));
    return singleRotate(root, dir);
  }

  const { console } = globalThis;
  const OriginalDate = Date;

  // Timeout values > TIMEOUT_MAX are set to 1.
  const TIMEOUT_MAX = 2 ** 31 - 1;

  let globalTimeoutDue = null;

  let nextTimerId = 1;
  const idMap = new Map();
  const dueTree = new RBTree((a, b) => a.due - b.due);

  function clearGlobalTimeout() {
    globalTimeoutDue = null;
    opStopGlobalTimer();
  }

  let pendingEvents = 0;
  const pendingFireTimers = [];

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

  async function setGlobalTimeout(due, now) {
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
    opStartGlobalTimer(timeout);
    await opWaitGlobalTimer();
    pendingEvents--;
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    prepareReadyTimers();
  }

  function prepareReadyTimers() {
    const now = OriginalDate.now();
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
      assert(list[0] === timer);
      dueTree.remove(searchKey);
      // If the unscheduled timer was 'next up', find when the next timer that
      // still exists is due, and update the global alarm accordingly.
      if (timer.due === globalTimeoutDue) {
        const nextDueNode = dueTree.min();
        setOrClearGlobalTimeout(
          nextDueNode && nextDueNode.due,
          OriginalDate.now(),
        );
      }
    } else {
      // Multiple timers that are due at the same point in time.
      // Remove this timer from the list.
      const index = list.indexOf(timer);
      assert(index > -1);
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
      const now = OriginalDate.now();
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

  function setTimer(
    cb,
    delay,
    args,
    repeat,
  ) {
    // Bind `args` to the callback and bind `this` to globalThis(global).
    const callback = cb.bind(globalThis, ...args);
    // In the browser, the delay value must be coercible to an integer between 0
    // and INT32_MAX. Any other value will cause the timer to fire immediately.
    // We emulate this behavior.
    const now = OriginalDate.now();
    if (delay > TIMEOUT_MAX) {
      console.warn(
        `${delay} does not fit into` +
          " a 32-bit signed integer." +
          "\nTimeout duration was set to 1.",
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

  function setTimeout(
    cb,
    delay = 0,
    ...args
  ) {
    checkBigInt(delay);
    checkThis(this);
    return setTimer(cb, delay, args, false);
  }

  function setInterval(
    cb,
    delay = 0,
    ...args
  ) {
    checkBigInt(delay);
    checkThis(this);
    return setTimer(cb, delay, args, true);
  }

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

  function clearInterval(id = 0) {
    checkBigInt(id);
    if (id === 0) {
      return;
    }
    clearTimer(id);
  }

  window.__bootstrap.timers = {
    clearInterval,
    setInterval,
    clearTimeout,
    setTimeout,
    handleTimerMacrotask,
    opStopGlobalTimer,
    opStartGlobalTimer,
    opNow,
    sleepSync,
  };
})(this);
