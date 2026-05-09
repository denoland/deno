// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

(function () {
const { core } = globalThis.__bootstrap;
const {
  performance,
  PerformanceEntry,
  PerformanceObserver: WebPerformanceObserver,
  PerformanceObserverEntryList,
} = core.loadExtScript("ext:deno_web/15_performance.js");
const { EldHistogram, op_node_event_loop_metrics } = core.ops;
const { ERR_INVALID_ARG_TYPE } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

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

// Entry types Node.js's PerformanceObserver supports beyond the web spec's
// "mark"/"measure". The web layer's PerformanceObserver filters these out via
// supportedEntryTypes, so this subclass tracks them in a parallel registry.
const NODE_ENTRY_TYPES = ["http2", "function", "gc", "http", "dns", "net"];

const nodeObservers = [];
const _nodeTypes = Symbol("[[nodeTypes]]");
const _nodeBuffer = Symbol("[[nodeBuffer]]");
const _nodeScheduled = Symbol("[[nodeScheduled]]");
const _nodeCallback = Symbol("[[nodeCallback]]");

function createNodeEntryList(entries) {
  return {
    getEntries() {
      return entries.slice();
    },
    getEntriesByType(type) {
      return entries.filter((e) => e.entryType === type);
    },
    getEntriesByName(name, type) {
      return entries.filter((e) =>
        e.name === name && (type === undefined || e.entryType === type)
      );
    },
  };
}

// Node-compatible PerformanceObserver that throws proper Node.js errors
class PerformanceObserver extends WebPerformanceObserver {
  [_nodeTypes] = [];
  [_nodeBuffer] = [];
  [_nodeScheduled] = false;
  [_nodeCallback] = null;

  constructor(callback) {
    if (typeof callback !== "function") {
      throw new ERR_INVALID_ARG_TYPE("callback", "Function", callback);
    }
    super(callback);
    this[_nodeCallback] = callback;
  }

  static get supportedEntryTypes() {
    return [
      ...WebPerformanceObserver.supportedEntryTypes,
      ...NODE_ENTRY_TYPES,
    ];
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

    const requestedTypes = options.entryTypes !== undefined
      ? options.entryTypes
      : (options.type !== undefined ? [options.type] : []);

    const webTypes = requestedTypes.filter(
      (t) => !NODE_ENTRY_TYPES.includes(t),
    );
    const nodeTypes = requestedTypes.filter(
      (t) => NODE_ENTRY_TYPES.includes(t),
    );

    if (webTypes.length > 0) {
      if (options.entryTypes !== undefined) {
        super.observe({ entryTypes: webTypes, buffered: options.buffered });
      } else if (webTypes.length === 1) {
        super.observe({ type: webTypes[0], buffered: options.buffered });
      }
    }

    if (nodeTypes.length > 0) {
      this[_nodeTypes] = nodeTypes;
      this[_nodeBuffer] = [];
      if (!nodeObservers.includes(this)) {
        nodeObservers.push(this);
      }
    }
  }

  disconnect() {
    super.disconnect();
    const idx = nodeObservers.indexOf(this);
    if (idx !== -1) nodeObservers.splice(idx, 1);
    this[_nodeTypes] = [];
    this[_nodeBuffer] = [];
  }
}

// Internal helper used by node:http2 and other modules to dispatch
// Node-only PerformanceObserver entries (e.g. `Http2Session`) that the web
// PerformanceObserver does not understand.
function enqueueNodePerformanceEntry(entry) {
  for (let i = 0; i < nodeObservers.length; i++) {
    const obs = nodeObservers[i];
    if (!obs[_nodeTypes].includes(entry.entryType)) continue;
    obs[_nodeBuffer].push(entry);
    if (obs[_nodeScheduled]) continue;
    obs[_nodeScheduled] = true;
    queueMicrotask(() => {
      obs[_nodeScheduled] = false;
      const entries = obs[_nodeBuffer];
      obs[_nodeBuffer] = [];
      if (entries.length === 0) return;
      const list = createNodeEntryList(entries);
      try {
        obs[_nodeCallback](list, obs);
      } catch (_e) {
        // Match web observer: callback errors should not crash dispatch.
      }
    });
  }
}

const eluBuf = new Float64Array(3);
const eluU8 = new Uint8Array(eluBuf.buffer);

function eventLoopUtilization(util1, util2) {
  if (util2) {
    const idle = util1.idle - util2.idle;
    const active = util1.active - util2.active;
    return { idle, active, utilization: active / (idle + active) };
  }

  op_node_event_loop_metrics(eluU8);
  const idle = eluBuf[1];
  const active = eluBuf[2];

  if (!util1) {
    return { idle, active, utilization: active / (idle + active) };
  }

  const idleDelta = idle - util1.idle;
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

return {
  default: {
    performance,
    PerformanceObserver,
    PerformanceObserverEntryList,
    PerformanceEntry,
    monitorEventLoopDelay,
    eventLoopUtilization,
    timerify,
    constants,
  },
  constants,
  enqueueNodePerformanceEntry,
  eventLoopUtilization,
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
  PerformanceObserverEntryList,
  timerify,
};
})();
