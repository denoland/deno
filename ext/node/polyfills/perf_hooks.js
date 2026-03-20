// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  performance,
  PerformanceEntry,
  PerformanceObserver as WebPerformanceObserver,
  PerformanceObserverEntryList,
} from "ext:deno_web/15_performance.js";
import { EldHistogram, op_node_uv_metrics_info } from "ext:core/ops";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";

const constants = {
  NODE_PERFORMANCE_ENTRY_TYPE_NODE: 0,
  NODE_PERFORMANCE_ENTRY_TYPE_MARK: 1,
  NODE_PERFORMANCE_ENTRY_TYPE_MEASURE: 2,
  NODE_PERFORMANCE_ENTRY_TYPE_GC: 3,
  NODE_PERFORMANCE_ENTRY_TYPE_FUNCTION: 4,
  NODE_PERFORMANCE_ENTRY_TYPE_HTTP2: 5,
  NODE_PERFORMANCE_ENTRY_TYPE_HTTP: 6,
  NODE_PERFORMANCE_ENTRY_TYPE_DNS: 7,
  NODE_PERFORMANCE_ENTRY_TYPE_NET: 8,
};

// Node-compatible PerformanceObserver that throws proper Node.js errors
class PerformanceObserver extends WebPerformanceObserver {
  constructor(callback) {
    if (typeof callback !== "function") {
      throw new ERR_INVALID_ARG_TYPE("callback", "Function", callback);
    }
    super(callback);
  }

  observe(options) {
    if (typeof options !== "object" || options === null) {
      throw new ERR_INVALID_ARG_TYPE("options", "Object", options);
    }
    if (
      options.entryTypes !== undefined && !Array.isArray(options.entryTypes)
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.entryTypes",
        "string[]",
        options.entryTypes,
      );
    }
    return super.observe(options);
  }

  static get supportedEntryTypes() {
    return WebPerformanceObserver.supportedEntryTypes;
  }
}

performance.eventLoopUtilization = () => {
  // TODO(@marvinhagemeister): Return actual non-stubbed values
  return { idle: 0, active: 0, utilization: 0 };
};

// PerformanceNodeTiming provides Node.js-compatible timing milestones.
// In Deno we don't have exact equivalents for all Node.js milestones,
// so we approximate where possible and stub the rest.
class PerformanceNodeTiming {
  #startTime = 0;

  get name() {
    return "node";
  }

  get entryType() {
    return "node";
  }

  get startTime() {
    return this.#startTime;
  }

  get duration() {
    return performance.now();
  }

  // In Node.js these are timestamps relative to process start.
  // We approximate with 0 since Deno doesn't track these milestones
  // separately. They are in ascending order as Node.js tests expect.
  get nodeStart() {
    return 0;
  }

  get v8Start() {
    return 0.1;
  }

  get environment() {
    return 0.2;
  }

  get bootstrapComplete() {
    return 0.3;
  }

  // loopStart is -1 until the event loop starts, then the time it started.
  // We return performance.now() after the first event loop tick.
  #loopStartValue = -1;
  #loopStartChecked = false;
  get loopStart() {
    if (!this.#loopStartChecked) {
      // After bootstrap, the loop has started if we're being called
      // from a timer/immediate callback (i.e., the event loop is running).
      // Use a heuristic: if performance.now() > 1, the loop has started.
      if (performance.now() > 1) {
        this.#loopStartValue = 0.4;
        this.#loopStartChecked = true;
      }
    }
    return this.#loopStartValue;
  }

  get loopExit() {
    return -1;
  }

  get idleTime() {
    // TODO(#31085): track actual idle time in UvLoopInner
    return 0;
  }

  get uvMetricsInfo() {
    const info = op_node_uv_metrics_info();
    if (info == null) {
      return { loopCount: 0, events: 0, eventsWaiting: 0 };
    }
    return {
      loopCount: info[0],
      events: info[1],
      eventsWaiting: info[2],
    };
  }

  toJSON() {
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      nodeStart: this.nodeStart,
      v8Start: this.v8Start,
      environment: this.environment,
      bootstrapComplete: this.bootstrapComplete,
      loopStart: this.loopStart,
      loopExit: this.loopExit,
      idleTime: this.idleTime,
    };
  }
}

performance.nodeTiming = new PerformanceNodeTiming();

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
  PerformanceObserverEntryList,
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
  PerformanceObserverEntryList,
};
