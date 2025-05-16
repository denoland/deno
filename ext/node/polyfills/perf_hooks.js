// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  performance as shimPerformance,
  PerformanceEntry,
} from "ext:deno_web/15_performance.js";
import { EldHistogram } from "ext:core/ops";

class PerformanceObserver {
  static supportedEntryTypes = [];
  observe() {
    // todo(lucacasonato): actually implement this
  }
  disconnect() {
    // todo(lucacasonato): actually implement this
  }
}

const constants = {};

const performance = {
  clearMarks: (markName) => shimPerformance.clearMarks(markName),
  eventLoopUtilization: () => {
    // TODO(@marvinhagemeister): Return actual non-stubbed values
    return { idle: 0, active: 0, utilization: 0 };
  },
  mark: (markName) => shimPerformance.mark(markName),
  measure: (
    measureName,
    startMark,
    endMark,
  ) => {
    if (endMark) {
      return shimPerformance.measure(
        measureName,
        startMark,
        endMark,
      );
    } else {
      return shimPerformance.measure(
        measureName,
        startMark,
      );
    }
  },
  nodeTiming: {},
  now: () => shimPerformance.now(),
  timerify: () => notImplemented("timerify from performance"),
  get timeOrigin() {
    return shimPerformance.timeOrigin;
  },
  getEntriesByName: (name, type) =>
    shimPerformance.getEntriesByName(name, type),
  getEntriesByType: (type) => shimPerformance.getEntriesByType(type),
  markResourceTiming: () => {},
  toJSON: () => shimPerformance.toJSON(),
  addEventListener: (...args) => shimPerformance.addEventListener(...args),
  removeEventListener: (...args) =>
    shimPerformance.removeEventListener(...args),
  dispatchEvent: (...args) => shimPerformance.dispatchEvent(...args),
};

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
