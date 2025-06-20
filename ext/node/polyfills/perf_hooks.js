// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";
import { performance, PerformanceEntry } from "ext:deno_web/15_performance.js";
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

performance.eventLoopUtilization = () => {
  // TODO(@marvinhagemeister): Return actual non-stubbed values
  return { idle: 0, active: 0, utilization: 0 };
};

// TODO(bartlomieju):
performance.nodeTiming = {};

// TODO(bartlomieju):
performance.timerify = () => notImplemented("timerify from performance");

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
