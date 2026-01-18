// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  performance,
  PerformanceEntry,
  registerPerformanceObserver,
  unregisterPerformanceObserver,
} from "ext:deno_web/15_performance.js";
import { EldHistogram } from "ext:core/ops";

class PerformanceObserverEntryList {
  #entries;

  constructor(entries) {
    this.#entries = entries;
  }

  getEntries() {
    return this.#entries;
  }

  getEntriesByType(type) {
    return this.#entries.filter((entry) => entry.entryType === type);
  }

  getEntriesByName(name, type) {
    return this.#entries.filter(
      (entry) =>
        entry.name === name && (type === undefined || entry.entryType === type),
    );
  }
}

class PerformanceObserver {
  static supportedEntryTypes = ["mark", "measure"];

  #callback;
  #entryTypes = [];
  #buffer = [];
  #handler = null;

  constructor(callback) {
    if (typeof callback !== "function") {
      throw new TypeError("callback must be a function");
    }
    this.#callback = callback;
  }

  observe(options = {}) {
    if (options.entryTypes) {
      this.#entryTypes = options.entryTypes;
    } else if (options.type) {
      this.#entryTypes = [options.type];
    } else {
      throw new TypeError(
        "observe requires either 'entryTypes' or 'type' option",
      );
    }

    // Create handler that filters by entry type and buffers entries
    this.#handler = (entry) => {
      if (this.#entryTypes.includes(entry.entryType)) {
        this.#buffer.push(entry);
        // Use queueMicrotask to batch entries and call callback asynchronously
        queueMicrotask(() => {
          if (this.#buffer.length > 0) {
            const entries = this.#buffer;
            this.#buffer = [];
            const entryList = new PerformanceObserverEntryList(entries);
            this.#callback(entryList, this);
          }
        });
      }
    };

    registerPerformanceObserver(this.#handler);
  }

  disconnect() {
    if (this.#handler) {
      unregisterPerformanceObserver(this.#handler);
      this.#handler = null;
    }
    this.#buffer = [];
  }

  takeRecords() {
    const entries = this.#buffer;
    this.#buffer = [];
    return entries;
  }
}

const constants = {};

performance.eventLoopUtilization = () => {
  // TODO(@marvinhagemeister): Return actual non-stubbed values
  return { idle: 0, active: 0, utilization: 0 };
};

performance.nodeTiming = {};

performance.timerify = (fn) => {
  if (typeof fn !== "function") {
    throw new TypeError("The 'fn' argument must be of type function");
  }
  const wrapped = (...args) => {
    const start = performance.now();
    const result = fn(...args);
    const end = performance.now();

    performance.measure(`timerify(${fn.name || "anonymous"})`, { start, end });

    return result;
  };

  Object.defineProperty(wrapped, "name", {
    value: fn.name || "wrapped",
    configurable: true,
  });

  return wrapped;
};
// TODO(bartlomieju):
performance.markResourceTiming = () => {};

function monitorEventLoopDelay(options = {}) {
  const { resolution = 10 } = options;

  return new EldHistogram(resolution);
}

export default {
  performance,
  PerformanceObserver,
  PerformanceEntry,
  monitorEventLoopDelay,
  constants,
};

export {
  constants,
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
};
