// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  performance,
  PerformanceEntry,
  PerformanceObserver as WebPerformanceObserver,
  PerformanceObserverEntryList,
} from "ext:deno_web/15_performance.js";
import { EldHistogram, op_node_event_loop_metrics } from "ext:core/ops";
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

const eluBuf = new Float64Array(2);
const eluU8 = new Uint8Array(eluBuf.buffer);

function eventLoopUtilization(util1, util2) {
  op_node_event_loop_metrics(eluU8);
  const loopStart = eluBuf[0];
  const idleTime = eluBuf[1];

  if (loopStart <= 0) {
    return { idle: 0, active: 0, utilization: 0 };
  }

  if (util2) {
    const idle = util1.idle - util2.idle;
    const active = util1.active - util2.active;
    return { idle, active, utilization: active / (idle + active) };
  }

  const now = performance.now();
  const active = now - loopStart - idleTime;

  if (!util1) {
    return {
      idle: idleTime,
      active,
      utilization: active / (idleTime + active),
    };
  }

  const idleDelta = idleTime - util1.idle;
  const activeDelta = active - util1.active;
  return {
    idle: idleDelta,
    active: activeDelta,
    utilization: activeDelta / (idleDelta + activeDelta),
  };
}

performance.eventLoopUtilization = eventLoopUtilization;

performance.nodeTiming = {
  get name() {
    return "node";
  },
  get entryType() {
    return "node";
  },
  get startTime() {
    return 0;
  },
  get duration() {
    return performance.now();
  },
  get nodeStart() {
    return 0;
  },
  get v8Start() {
    return 0;
  },
  get environment() {
    return 0;
  },
  get loopStart() {
    op_node_event_loop_metrics(eluU8);
    return eluBuf[0];
  },
  get loopExit() {
    return -1;
  },
  get bootstrapComplete() {
    return 0;
  },
  get idleTime() {
    op_node_event_loop_metrics(eluU8);
    return eluBuf[1];
  },
  toJSON() {
    return {
      name: this.name,
      entryType: this.entryType,
      startTime: this.startTime,
      duration: this.duration,
      nodeStart: this.nodeStart,
      v8Start: this.v8Start,
      environment: this.environment,
      loopStart: this.loopStart,
      loopExit: this.loopExit,
      bootstrapComplete: this.bootstrapComplete,
      idleTime: this.idleTime,
    };
  },
};

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
  eventLoopUtilization,
  constants,
};

export {
  constants,
  eventLoopUtilization,
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
  PerformanceObserverEntryList,
};
