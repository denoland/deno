// Copyright 2018-2026 the Deno authors. MIT license.
"use strict";

((window) => {
  const {
    MathMax,
    MathTrunc,
    NumberMIN_SAFE_INTEGER,
  } = window.__bootstrap.primordials;
  const {
    op_timer_schedule,
    op_timer_track,
    op_timer_untrack,
    op_timer_now,
    op_leak_tracing_submit,
  } = window.Deno.core.ops;
  const {
    ErrorCaptureStackTrace,
    StringPrototypeSlice,
  } = window.__bootstrap.primordials;
  const { __isLeakTracingEnabled } = window.__infra;

  // ---------------------------------------------------------------------------
  // Linked list helpers (matches Node.js lib/internal/linkedlist.js)
  // Circular doubly-linked list where the sentinel's _idleNext points to the
  // tail (newest) and _idlePrev points to the head (oldest/most-idle).
  // ---------------------------------------------------------------------------
  function L_init(list) {
    list._idleNext = list;
    list._idlePrev = list;
  }

  function L_peek(list) {
    if (list._idlePrev === list) return null;
    return list._idlePrev;
  }

  function L_remove(item) {
    if (item._idleNext) {
      item._idleNext._idlePrev = item._idlePrev;
    }
    if (item._idlePrev) {
      item._idlePrev._idleNext = item._idleNext;
    }
    item._idleNext = null;
    item._idlePrev = null;
  }

  function L_append(list, item) {
    if (item._idleNext || item._idlePrev) {
      L_remove(item);
    }
    item._idleNext = list._idleNext;
    item._idlePrev = list;
    list._idleNext._idlePrev = item;
    list._idleNext = item;
  }

  function L_isEmpty(list) {
    return list._idleNext === list;
  }

  // ---------------------------------------------------------------------------
  // Priority queue (binary min-heap, matches Node.js lib/internal/priority_queue.js)
  // ---------------------------------------------------------------------------
  class PriorityQueue {
    #compare;
    #setPosition;
    #heap = [undefined, undefined];
    #size = 0;

    constructor(comparator, setPosition) {
      this.#compare = comparator;
      this.#setPosition = setPosition;
    }

    insert(value) {
      const pos = ++this.#size;
      this.#heap[pos] = value;
      this.#percolateUp(pos);
    }

    peek() {
      return this.#heap[1];
    }

    shift() {
      const value = this.#heap[1];
      if (value === undefined) return;
      this.#removeAt(1);
      return value;
    }

    percolateDown(pos) {
      const compare = this.#compare;
      const setPosition = this.#setPosition;
      const heap = this.#heap;
      const size = this.#size;
      const hsize = size >> 1;
      const item = heap[pos];

      while (pos <= hsize) {
        let child = pos << 1;
        const nextChild = child + 1;
        let childItem = heap[child];

        if (nextChild <= size && compare(heap[nextChild], childItem) < 0) {
          child = nextChild;
          childItem = heap[nextChild];
        }

        if (compare(item, childItem) <= 0) break;

        if (setPosition) setPosition(childItem, pos);
        heap[pos] = childItem;
        pos = child;
      }

      heap[pos] = item;
      if (setPosition) setPosition(item, pos);
    }

    #percolateUp(pos) {
      const heap = this.#heap;
      const compare = this.#compare;
      const setPosition = this.#setPosition;
      const item = heap[pos];

      while (pos > 1) {
        const parent = pos >> 1;
        const parentItem = heap[parent];
        if (compare(parentItem, item) <= 0) break;
        heap[pos] = parentItem;
        if (setPosition) setPosition(parentItem, pos);
        pos = parent;
      }

      heap[pos] = item;
      if (setPosition) setPosition(item, pos);
    }

    #removeAt(pos) {
      const heap = this.#heap;
      let size = this.#size;
      heap[pos] = heap[size];
      heap[size] = undefined;
      size = --this.#size;

      if (size > 0 && pos <= size) {
        if (pos > 1 && this.#compare(heap[pos >> 1], heap[pos]) > 0) {
          this.#percolateUp(pos);
        } else {
          this.percolateDown(pos);
        }
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Timer infrastructure (matches Node.js lib/internal/timers.js)
  // ---------------------------------------------------------------------------
  const TIMEOUT_MAX = 2 ** 31 - 1;

  let timerListId = NumberMIN_SAFE_INTEGER;
  let nextExpiry = Infinity;
  let nextTimerId = 1;

  // Shared buffer with Rust. Index 0: refed timer count.
  // Set by Rust during store_js_callbacks via __setTimerInfo.
  let timerInfo;

  function compareTimersLists(a, b) {
    const expiryDiff = a.expiry - b.expiry;
    if (expiryDiff === 0) {
      return a.id - b.id;
    }
    return expiryDiff;
  }

  function setPosition(node, pos) {
    node.priorityQueuePosition = pos;
  }

  const timerListQueue = new PriorityQueue(compareTimersLists, setPosition);
  const timerListMap = { __proto__: null };

  class TimersList {
    constructor(expiry, msecs) {
      this._idleNext = this;
      this._idlePrev = this;
      this.expiry = expiry;
      this.id = timerListId++;
      this.msecs = msecs;
      this.priorityQueuePosition = null;
    }
  }

  function incRefCount() {
    if (timerInfo[0]++ === 0) {
      op_timer_schedule(-1); // ref the timer handle
    }
  }

  function decRefCount() {
    if (--timerInfo[0] === 0) {
      op_timer_schedule(-2); // unref the timer handle
    }
  }

  // Insert a timer item into the appropriate bucket.
  function insert(item, msecs, start) {
    if (start === undefined) start = op_timer_now();
    msecs = MathTrunc(msecs);
    item._idleStart = start;

    let list = timerListMap[msecs];
    if (list === undefined) {
      const expiry = start + msecs;
      timerListMap[msecs] = list = new TimersList(expiry, msecs);
      timerListQueue.insert(list);

      if (nextExpiry > expiry) {
        op_timer_schedule(msecs);
        nextExpiry = expiry;
      }
    }

    L_append(list, item);
  }

  // Called from Rust when the native timer fires.
  // Returns: positive = next expiry (has refed timers),
  //          negative = next expiry (no refed timers, negate for actual value),
  //          0 = no timers remain.
  function processTimers(now) {
    nextExpiry = Infinity;

    let list;
    let ranAtLeastOneList = false;
    while ((list = timerListQueue.peek()) != null) {
      if (list.expiry > now) {
        nextExpiry = list.expiry;
        return timerInfo[0] > 0 ? nextExpiry : -nextExpiry;
      }
      if (ranAtLeastOneList) {
        runNextTicks();
      } else {
        ranAtLeastOneList = true;
      }
      listOnTimeout(list, now);
    }
    return 0;
  }

  // runNextTicks is set by 01_core.js via setRunNextTicks().
  let runNextTicks = () => {};
  // reportException is set by 01_core.js via setReportException().
  let reportException = (e) => {
    throw e;
  };

  function setRunNextTicks(fn) {
    runNextTicks = fn;
  }

  function setReportException(fn) {
    reportException = fn;
  }

  function listOnTimeout(list, now) {
    const msecs = list.msecs;

    let ranAtLeastOneTimer = false;
    let timer;
    while ((timer = L_peek(list)) != null) {
      const diff = now - timer._idleStart;

      if (diff < msecs) {
        list.expiry = MathMax(timer._idleStart + msecs, now + 1);
        list.id = timerListId++;
        timerListQueue.percolateDown(1);
        return;
      }

      if (ranAtLeastOneTimer) {
        runNextTicks();
      } else {
        ranAtLeastOneTimer = true;
      }

      L_remove(timer);

      if (!timer._onTimeout) {
        if (!timer._destroyed) {
          timer._destroyed = true;
          if (timer[kRefed]) {
            decRefCount();
          }
          op_timer_untrack(timer._timerId);
        }
        continue;
      }

      try {
        const args = timer._timerArgs;
        if (args === undefined) {
          timer._onTimeout();
        } else {
          timer._onTimeout(...args);
        }
      } catch (e) {
        reportException(e);
      } finally {
        if (
          timer._repeat && timer._idleTimeout !== -1 &&
          !timer._idlePrev && !timer._idleNext
        ) {
          timer._idleTimeout = timer._repeat;
          insert(timer, timer._idleTimeout, now);
        } else if (!timer._idleNext && !timer._idlePrev && !timer._destroyed) {
          timer._destroyed = true;
          if (timer[kRefed]) {
            decRefCount();
          }
          op_timer_untrack(timer._timerId);
        }
      }
    }

    // List is empty, clean up.
    if (list === timerListMap[msecs]) {
      delete timerListMap[msecs];
      timerListQueue.shift();
    }
  }

  // Timer item constructor (internal). Callers (web timers, node timers)
  // wrap this with their own validation and async context handling.
  function createTimer(callback, after, args, isRepeat, isRefed, isSystem) {
    if (after === undefined) {
      after = 1;
    } else {
      after *= 1;
    }
    if (after < 1 || !(after <= TIMEOUT_MAX)) {
      after = after > TIMEOUT_MAX ? TIMEOUT_MAX : 1;
    }

    const id = nextTimerId++;
    const timer = {
      _idleTimeout: after,
      _idlePrev: null,
      _idleNext: null,
      _idleStart: null,
      _onTimeout: callback,
      _timerArgs: args,
      _repeat: isRepeat ? after : null,
      _destroyed: false,
      [kRefed]: isRefed,
      _timerId: id,
    };

    if (isRefed) incRefCount();
    insert(timer, after);
    op_timer_track(id, !!isRepeat, !!isSystem);
    if (__isLeakTracingEnabled()) {
      const error = new Error();
      ErrorCaptureStackTrace(error, createTimer);
      op_leak_tracing_submit(2, id, StringPrototypeSlice(error.stack, 6));
    }
    return timer;
  }

  const kRefed = Symbol("refed");

  function cancelTimer(timer) {
    if (timer._destroyed) return;
    timer._destroyed = true;
    timer._onTimeout = null;
    if (timer[kRefed]) {
      decRefCount();
    }
    L_remove(timer);
    op_timer_untrack(timer._timerId);
  }

  function refreshTimer(timer) {
    // Remove from current list
    if (timer._idlePrev || timer._idleNext) {
      L_remove(timer);
    }
    // Re-insert with current time
    insert(timer, timer._idleTimeout);
  }

  function refTimer(timer) {
    if (!timer[kRefed]) {
      timer[kRefed] = true;
      if (!timer._destroyed) incRefCount();
    }
  }

  function unrefTimer(timer) {
    if (timer[kRefed]) {
      timer[kRefed] = false;
      if (!timer._destroyed) decRefCount();
    }
  }

  // Exported on window.__timers for 01_core.js to pick up.
  window.__timers = {
    processTimers,
    createTimer,
    cancelTimer,
    refreshTimer,
    refTimer,
    unrefTimer,
    insert,
    incRefCount,
    decRefCount,
    kRefed,
    setRunNextTicks,
    setReportException,
    TIMEOUT_MAX,
    L_init,
    L_peek,
    L_remove,
    L_append,
    L_isEmpty,
    __setTimerInfo(buf) {
      timerInfo = buf;
    },
  };
})(globalThis);
