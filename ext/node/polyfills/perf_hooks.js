// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  performance,
  PerformanceEntry,
  PerformanceObserver as WebPerformanceObserver,
  PerformanceObserverEntryList,
  registerExtraEntryType,
} from "ext:deno_web/15_performance.js";
import { EldHistogram } from "ext:core/ops";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";

// Make Node-only entry types observable through the shared web
// `PerformanceObserver`. The actual entry production lives in the
// individual subsystems (e.g. `node:http2`).
registerExtraEntryType("http2");

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

const eventLoopUtilization = () => {
  // TODO(@marvinhagemeister): Return actual non-stubbed values
  return { idle: 0, active: 0, utilization: 0 };
};

performance.eventLoopUtilization = eventLoopUtilization;

performance.nodeTiming = {};

const timerify = (fn, options = {}) => {
  if (typeof fn !== "function") {
    throw new ERR_INVALID_ARG_TYPE("fn", "function", fn);
  }

  if (
    options !== undefined && (typeof options !== "object" || options === null)
  ) {
    throw new ERR_INVALID_ARG_TYPE("options", "Object", options);
  }

  if (options?.histogram !== undefined) {
    if (
      typeof options.histogram !== "object" ||
      options.histogram === null ||
      typeof options.histogram.record !== "function"
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.histogram",
        "RecordableHistogram",
        options.histogram,
      );
    }
  }

  function timerified(...args) {
    // TODO(bartlomieju): emit PerformanceEntry with entryType 'function'
    return new.target ? new fn(...args) : fn.apply(this, args);
  }

  Object.defineProperty(timerified, "name", {
    value: `timerified ${fn.name}`,
    configurable: true,
  });
  Object.defineProperty(timerified, "length", {
    value: fn.length,
    configurable: true,
  });

  return timerified;
};

performance.timerify = timerify;
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
  timerify,
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
  timerify,
};
