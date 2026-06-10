// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayFrom,
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  BigInt,
  BigIntPrototypeToString,
  Error,
  FunctionPrototypeApply,
  MapPrototypeGet,
  MapPrototypeSet,
  MathMax,
  MathRound,
  Number,
  NumberIsInteger,
  NumberMAX_SAFE_INTEGER,
  ObjectDefineProperty,
  ObjectFromEntries,
  ObjectPrototypeIsPrototypeOf,
  queueMicrotask,
  ReflectConstruct,
  SafeArrayIterator,
  SafeMap,
  Symbol,
  SymbolDispose,
} = primordials;
const {
  performance,
  PerformanceEntry,
  PerformanceObserver: WebPerformanceObserver,
  PerformanceObserverEntryList,
} = core.loadExtScript("ext:deno_web/15_performance.js");
const { EldHistogram, BaseHistogram } = core.ops;
const { ERR_ILLEGAL_CONSTRUCTOR, ERR_INVALID_ARG_TYPE, ERR_OUT_OF_RANGE } = core
  .loadExtScript(
    "ext:deno_node/internal/errors.ts",
  );
const { customInspectSymbol } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const { inspect } = core.loadExtScript(
  "ext:deno_node/internal/util/inspect.mjs",
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
      return ArrayPrototypeSlice(entries);
    },
    getEntriesByType(type) {
      return ArrayPrototypeFilter(entries, (e) => e.entryType === type);
    },
    getEntriesByName(name, type) {
      return ArrayPrototypeFilter(
        entries,
        (e) => e.name === name && (type === undefined || e.entryType === type),
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
      ...new SafeArrayIterator(WebPerformanceObserver.supportedEntryTypes),
      ...new SafeArrayIterator(NODE_ENTRY_TYPES),
    ];
  }

  observe(options) {
    if (typeof options !== "object" || options === null) {
      throw new ERR_INVALID_ARG_TYPE("options", "Object", options);
    }
    if (
      options.entryTypes !== undefined && !ArrayIsArray(options.entryTypes)
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

    const webTypes = ArrayPrototypeFilter(
      requestedTypes,
      (t) => !ArrayPrototypeIncludes(NODE_ENTRY_TYPES, t),
    );
    const nodeTypes = ArrayPrototypeFilter(
      requestedTypes,
      (t) => ArrayPrototypeIncludes(NODE_ENTRY_TYPES, t),
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
      if (!ArrayPrototypeIncludes(nodeObservers, this)) {
        ArrayPrototypePush(nodeObservers, this);
      }
    }
  }

  disconnect() {
    super.disconnect();
    const idx = ArrayPrototypeIndexOf(nodeObservers, this);
    if (idx !== -1) ArrayPrototypeSplice(nodeObservers, idx, 1);
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
    if (!ArrayPrototypeIncludes(obs[_nodeTypes], entry.entryType)) continue;
    ArrayPrototypePush(obs[_nodeBuffer], entry);
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

function hasNodeObserverForType(entryType) {
  for (let i = 0; i < nodeObservers.length; i++) {
    if (ArrayPrototypeIncludes(nodeObservers[i][_nodeTypes], entryType)) {
      return true;
    }
  }
  return false;
}

const eventLoopUtilization = () => {
  // TODO(@marvinhagemeister): Return actual non-stubbed values
  return { idle: 0, active: 0, utilization: 0 };
};

performance.eventLoopUtilization = eventLoopUtilization;

performance.nodeTiming = {};

function recordTimerifyHistogram(histogram, start) {
  const durationNs = MathMax(1, MathRound((performance.now() - start) * 1e6));
  histogram.record(durationNs);
}

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
      !ObjectPrototypeIsPrototypeOf(
        RecordableHistogram.prototype,
        options.histogram,
      )
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.histogram",
        "RecordableHistogram",
        options.histogram,
      );
    }
  }

  const histogram = options?.histogram;

  function timerified(...args) {
    // TODO(bartlomieju): emit PerformanceEntry with entryType 'function'
    const start = histogram === undefined ? 0 : performance.now();
    let result;
    try {
      result = new.target
        ? ReflectConstruct(fn, args)
        : FunctionPrototypeApply(fn, this, args);
    } catch (err) {
      if (histogram !== undefined) {
        recordTimerifyHistogram(histogram, start);
      }
      throw err;
    }
    if (
      histogram !== undefined && result !== null &&
      (typeof result === "object" || typeof result === "function") &&
      typeof result.then === "function"
    ) {
      // `result` is an arbitrary user thenable, not necessarily a native
      // Promise, so PromisePrototypeFinally cannot be used here.
      // deno-lint-ignore prefer-primordials
      return result.finally(() => recordTimerifyHistogram(histogram, start));
    }
    if (histogram !== undefined) {
      recordTimerifyHistogram(histogram, start);
    }
    return result;
  }

  ObjectDefineProperty(timerified, "name", {
    __proto__: null,
    value: `timerified ${fn.name}`,
    configurable: true,
  });
  ObjectDefineProperty(timerified, "length", {
    __proto__: null,
    value: fn.length,
    configurable: true,
  });

  return timerified;
};

performance.timerify = timerify;
// TODO(bartlomieju):
performance.markResourceTiming = () => {};

function monitorEventLoopDelay(options = { __proto__: null }) {
  const { resolution = 10 } = options;

  return new EventLoopDelayHistogram(new EldHistogram(resolution));
}

core.registerCloneableResource("EventLoopDelayHistogram", snapshotHistogram);

const NUMBER_MAX_SAFE_INTEGER = NumberMAX_SAFE_INTEGER;
const EMPTY_HISTOGRAM_MIN = 9223372036854776000;
const EMPTY_HISTOGRAM_MIN_BIGINT = 9223372036854775807n;
const BIGINT_MAX = 0x7FFFFFFFFFFFFFFFn;
const _handle = Symbol("[[handle]]");
const _cloneId = Symbol("[[cloneId]]");
let nextHistogramCloneId = 1;
const histogramCloneRegistry = new SafeMap();

function getHistogramCloneId(histogram) {
  if (histogram[_cloneId] === undefined) {
    histogram[_cloneId] = nextHistogramCloneId++;
  }
  MapPrototypeSet(
    histogramCloneRegistry,
    histogram[_cloneId],
    histogram[_handle],
  );
  return histogram[_cloneId];
}

// Public, JS-side Histogram class - `instanceof` works against both
// `monitorEventLoopDelay()` results and `createHistogram()` results.
class Histogram {
  constructor(handle) {
    // Intentionally not user-constructable.
    if (handle === undefined) {
      throw new ERR_ILLEGAL_CONSTRUCTOR();
    }
    this[_handle] = handle;
  }

  get count() {
    return this[_handle].count;
  }
  get countBigInt() {
    return this[_handle].countBigInt;
  }
  get min() {
    if (this.max === 0) return EMPTY_HISTOGRAM_MIN;
    return this[_handle].min;
  }
  get minBigInt() {
    if (this.max === 0) return EMPTY_HISTOGRAM_MIN_BIGINT;
    return BigInt(this[_handle].minBigInt);
  }
  get max() {
    return this[_handle].max;
  }
  get maxBigInt() {
    return BigInt(this[_handle].maxBigInt);
  }
  get mean() {
    if (this.max === 0) return NaN;
    return this[_handle].mean;
  }
  get stddev() {
    if (this.max === 0) return NaN;
    return this[_handle].stddev;
  }
  get exceeds() {
    return this[_handle].exceeds ?? 0;
  }
  get exceedsBigInt() {
    return BigInt(this[_handle].exceedsBigInt ?? 0);
  }
  get percentiles() {
    // Real Map (not SafeMap): returned to userland and must pass
    // `instanceof Map` (SafeMap's prototype chain excludes Map).
    // deno-lint-ignore prefer-primordials
    const out = new Map();
    if (typeof this[_handle].percentiles === "function") {
      const flat = this[_handle].percentiles();
      for (let i = 0; i < flat.length; i += 2) {
        MapPrototypeSet(out, flat[i], flat[i + 1]);
      }
    } else if (this.count === 0) {
      MapPrototypeSet(out, 100, 0);
    } else {
      MapPrototypeSet(out, 0, this.min);
      for (
        let percentile = 50;
        percentile < 100;
        percentile += (100 - percentile) / 2
      ) {
        MapPrototypeSet(out, percentile, this.percentile(percentile));
        if (percentile > 99.999) break;
      }
      MapPrototypeSet(out, 100, this.max);
    }
    return out;
  }
  get percentilesBigInt() {
    // Real Map (not SafeMap): returned to userland and must pass
    // `instanceof Map` (SafeMap's prototype chain excludes Map).
    // deno-lint-ignore prefer-primordials
    const out = new Map();
    if (typeof this[_handle].percentilesBigInt === "function") {
      const flat = this[_handle].percentilesBigInt();
      for (let i = 0; i < flat.length; i += 2) {
        MapPrototypeSet(out, flat[i], BigInt(flat[i + 1]));
      }
    } else if (this.count === 0) {
      MapPrototypeSet(out, 100, 0n);
    } else {
      MapPrototypeSet(out, 0, this.minBigInt);
      for (
        let percentile = 50;
        percentile < 100;
        percentile += (100 - percentile) / 2
      ) {
        MapPrototypeSet(out, percentile, this.percentileBigInt(percentile));
        if (percentile > 99.999) break;
      }
      MapPrototypeSet(out, 100, this.maxBigInt);
    }
    return out;
  }
  percentile(p) {
    if (typeof p !== "number") {
      throw new ERR_INVALID_ARG_TYPE("percentile", "number", p);
    }
    if (!(p > 0 && p <= 100)) {
      throw new ERR_OUT_OF_RANGE("percentile", "> 0 && <= 100", p);
    }
    return this[_handle].percentile(p);
  }
  percentileBigInt(p) {
    if (typeof p !== "number") {
      throw new ERR_INVALID_ARG_TYPE("percentile", "number", p);
    }
    if (!(p > 0 && p <= 100)) {
      throw new ERR_OUT_OF_RANGE("percentile", "> 0 && <= 100", p);
    }
    return BigInt(this[_handle].percentileBigInt(p));
  }
  reset() {
    this[_handle].reset();
  }
  toJSON() {
    return {
      count: this.count,
      min: this.min,
      max: this.max,
      mean: this.mean,
      exceeds: this.exceeds,
      stddev: this.stddev,
      percentiles: ObjectFromEntries(this.percentiles),
    };
  }
  [customInspectSymbol](depth, options) {
    if (depth < 0) {
      return this;
    }

    return `Histogram ${
      inspect({
        min: this.min,
        max: this.max,
        mean: this.mean,
        exceeds: this.exceeds,
        stddev: this.stddev,
        count: this.count,
        percentiles: this.percentiles,
      }, options)
    }`;
  }
}

class RecordableHistogram extends Histogram {
  constructor(handle, cloneId) {
    super(handle);
    this[_cloneId] = cloneId;
  }

  record(val) {
    if (typeof val === "bigint") {
      if (val < 1n || val > BIGINT_MAX) {
        throw new ERR_OUT_OF_RANGE("val", "a positive integer", val);
      }
      this[_handle].record(val);
      return;
    }
    if (typeof val !== "number" || !NumberIsInteger(val)) {
      throw new ERR_INVALID_ARG_TYPE("val", ["integer", "bigint"], val);
    }
    if (val < 1 || val > NUMBER_MAX_SAFE_INTEGER) {
      throw new ERR_OUT_OF_RANGE(
        "val",
        `>= 1 && <= ${NUMBER_MAX_SAFE_INTEGER}`,
        val,
      );
    }
    this[_handle].record(BigInt(val));
  }

  recordDelta() {
    this[_handle].recordDelta();
  }

  add(other) {
    if (!ObjectPrototypeIsPrototypeOf(RecordableHistogram.prototype, other)) {
      throw new ERR_INVALID_ARG_TYPE(
        "other",
        "RecordableHistogram",
        other,
      );
    }
    this[_handle].add(other[_handle]);
  }

  [core.hostObjectBrand]() {
    return {
      type: "RecordableHistogram",
      id: getHistogramCloneId(this),
    };
  }
}

class EventLoopDelayHistogram extends Histogram {
  constructor(handle) {
    super(handle);
  }

  enable() {
    return this[_handle].enable();
  }

  disable() {
    return this[_handle].disable();
  }

  [SymbolDispose]() {
    this.disable();
  }

  [core.hostObjectBrand]() {
    return {
      type: "EventLoopDelayHistogram",
      count: this.count,
      countBigInt: this.countBigInt,
      min: this.min,
      minBigInt: this.minBigInt,
      max: this.max,
      maxBigInt: this.maxBigInt,
      mean: this.mean,
      exceeds: this.exceeds,
      exceedsBigInt: this.exceedsBigInt,
      stddev: this.stddev,
      percentiles: ArrayFrom(this.percentiles),
      percentilesBigInt: ArrayFrom(
        this.percentilesBigInt,
        (entry) => [entry[0], BigIntPrototypeToString(entry[1])],
      ),
    };
  }
}

function snapshotHistogram(data) {
  return new Histogram({
    count: data.count,
    countBigInt: data.countBigInt,
    min: data.min,
    minBigInt: data.minBigInt,
    max: data.max,
    maxBigInt: data.maxBigInt,
    mean: data.mean,
    exceeds: data.exceeds ?? 0,
    exceedsBigInt: data.exceedsBigInt ?? 0n,
    stddev: data.stddev,
    percentiles: () => {
      const out = [];
      const entries = data.percentiles ?? [[100, 0]];
      for (let i = 0; i < entries.length; i++) {
        ArrayPrototypePush(out, entries[i][0], entries[i][1]);
      }
      return out;
    },
    percentilesBigInt: () => {
      const out = [];
      const entries = data.percentilesBigInt ?? [[100, "0"]];
      for (let i = 0; i < entries.length; i++) {
        ArrayPrototypePush(out, entries[i][0], entries[i][1]);
      }
      return out;
    },
    percentile: (p) => {
      const entries = data.percentiles ?? [[100, 0]];
      for (let i = 0; i < entries.length; i++) {
        if (entries[i][0] >= p) return entries[i][1];
      }
      return data.max;
    },
    percentileBigInt: (p) => {
      const entries = data.percentilesBigInt ?? [[100, "0"]];
      for (let i = 0; i < entries.length; i++) {
        if (entries[i][0] >= p) return BigInt(entries[i][1]);
      }
      return BigInt(data.maxBigInt);
    },
    reset() {},
  });
}

core.registerCloneableResource("RecordableHistogram", (data) => {
  const handle = MapPrototypeGet(histogramCloneRegistry, data.id);
  if (handle === undefined) {
    throw new Error("Unable to deserialize RecordableHistogram");
  }
  return new RecordableHistogram(handle, data.id);
});

function validateInteger(value, name, min, max) {
  if (typeof value === "bigint") {
    if (value < BigInt(min) || value > BigInt(max)) {
      throw new ERR_OUT_OF_RANGE(name, `>= ${min} && <= ${max}`, value);
    }
    return Number(value);
  }
  if (typeof value !== "number" || !NumberIsInteger(value)) {
    throw new ERR_INVALID_ARG_TYPE(name, ["integer", "bigint"], value);
  }
  if (value < min || value > max) {
    throw new ERR_OUT_OF_RANGE(name, `>= ${min} && <= ${max}`, value);
  }
  return value;
}

function createHistogram(options = { __proto__: null }) {
  if (options === null || typeof options !== "object") {
    throw new ERR_INVALID_ARG_TYPE("options", "Object", options);
  }
  const {
    lowest = 1,
    highest = NUMBER_MAX_SAFE_INTEGER,
    figures = 3,
  } = options;
  const lo = validateInteger(
    lowest,
    "options.lowest",
    1,
    NUMBER_MAX_SAFE_INTEGER,
  );
  const hi = validateInteger(
    highest,
    "options.highest",
    2 * lo,
    NUMBER_MAX_SAFE_INTEGER,
  );
  if (typeof figures !== "number" || !NumberIsInteger(figures)) {
    throw new ERR_INVALID_ARG_TYPE("options.figures", "integer", figures);
  }
  if (figures < 1 || figures > 5) {
    throw new ERR_OUT_OF_RANGE("options.figures", ">= 1 && <= 5", figures);
  }
  const handle = new BaseHistogram(BigInt(lo), BigInt(hi), figures);
  return new RecordableHistogram(handle);
}

return {
  default: {
    performance,
    PerformanceObserver,
    PerformanceObserverEntryList,
    PerformanceEntry,
    monitorEventLoopDelay,
    createHistogram,
    eventLoopUtilization,
    timerify,
    constants,
  },
  constants,
  createHistogram,
  enqueueNodePerformanceEntry,
  eventLoopUtilization,
  hasNodeObserverForType,
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
  PerformanceObserverEntryList,
  timerify,
};
})();
