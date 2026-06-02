// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

(function () {
const { core } = __bootstrap;
const { ERR_TRACE_EVENTS_CATEGORY_REQUIRED } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { validateObject, validateStringArray } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const lazyBindingMod = core.createLazyLoader(
  "ext:deno_node/internal_binding/mod.ts",
);

function getProc() {
  // deno-lint-ignore no-process-global
  return typeof process !== "undefined" ? process : undefined;
}

// Each isolate (main + each worker thread) writes its own slice of trace
// events. The main thread, on exit, aggregates any worker slices left in cwd
// into a single `node_trace.${rotation}.log` so consumers see one combined
// file (matching Node's process-wide TracingController behavior).
// Resolved on first use rather than at module-load time so that worker_threads
// bootstrap (which sets up threadId/isMainThread) has finished by the time we
// read those fields.
let _wtExports = null;
function getWorkerThreadsExports() {
  if (_wtExports !== null) return _wtExports;
  try {
    _wtExports = core.loadExtScript("ext:deno_node/worker_threads.ts");
  } catch {
    _wtExports = {};
  }
  return _wtExports;
}

function getThreadId() {
  const wt = getWorkerThreadsExports();
  const tid = wt?.threadId ?? wt?.default?.threadId;
  return typeof tid === "number" ? tid : 0;
}

function isMainThreadProc() {
  const wt = getWorkerThreadsExports();
  const isMain = wt?.isMainThread ?? wt?.default?.isMainThread;
  if (typeof isMain === "boolean") return isMain;
  return getThreadId() === 0;
}

function workerSliceFilename(pid, tid) {
  return `.deno_trace_events_${pid}_t${tid}.json`;
}

const kCategories = Symbol("categories");
const kEnabled = Symbol("enabled");

const kMaxTracingCount = 10;

// Phase codes per V8/Chrome trace format.
const PHASE_NESTABLE_ASYNC_BEGIN = 98; // 'b'
const PHASE_NESTABLE_ASYNC_END = 101; // 'e'

const enabledTracingObjects = new Set();
const categoryBuffers = new Map();
const categoryRefCounts = new Map();
const recordedEvents = [];
let asyncHooksRefcount = 0;
let exitHandlerRegistered = false;
let originalSetTimeout = null;
let originalSetInterval = null;
let originalSetImmediate = null;
let traceIdCounter = 0;

function getCategoryEnabledBuffer(category) {
  let buf = categoryBuffers.get(category);
  if (buf === undefined) {
    buf = new Uint8Array(1);
    categoryBuffers.set(category, buf);
  }
  return buf;
}

function incrementCategory(category) {
  const prev = categoryRefCounts.get(category) ?? 0;
  const next = prev + 1;
  categoryRefCounts.set(category, next);
  const buf = getCategoryEnabledBuffer(category);
  buf[0] = next > 255 ? 255 : next;
  if (category === "node.async_hooks") {
    asyncHooksRefcount++;
    if (asyncHooksRefcount === 1) installAsyncHooksTimerTracing();
  }
}

function decrementCategory(category) {
  const prev = categoryRefCounts.get(category) ?? 0;
  const next = prev > 0 ? prev - 1 : 0;
  if (next === 0) {
    categoryRefCounts.delete(category);
  } else {
    categoryRefCounts.set(category, next);
  }
  const buf = getCategoryEnabledBuffer(category);
  buf[0] = next > 255 ? 255 : next;
  if (category === "node.async_hooks") {
    if (asyncHooksRefcount > 0) asyncHooksRefcount--;
    if (asyncHooksRefcount === 0) uninstallAsyncHooksTimerTracing();
  }
}

class Tracing {
  constructor(categories) {
    this[kCategories] = categories;
    this[kEnabled] = false;
  }

  enable() {
    if (!this[kEnabled]) {
      this[kEnabled] = true;
      for (const category of this[kCategories]) {
        incrementCategory(category);
      }
      enabledTracingObjects.add(this);
      if (enabledTracingObjects.size > kMaxTracingCount) {
        const p = getProc();
        if (p && p.emitWarning) {
          p.emitWarning(
            "Possible trace_events memory leak detected. There are more than " +
              `${kMaxTracingCount} enabled Tracing objects.`,
          );
        }
      }
      ensureExitHandlerInstalled();
    }
  }

  disable() {
    if (this[kEnabled]) {
      this[kEnabled] = false;
      for (const category of this[kCategories]) {
        decrementCategory(category);
      }
      enabledTracingObjects.delete(this);
    }
  }

  get enabled() {
    return this[kEnabled];
  }

  get categories() {
    return this[kCategories].join(",");
  }
}

function createTracing(options) {
  validateObject(options, "options");
  validateStringArray(options.categories, "options.categories");
  if (options.categories.length <= 0) {
    throw new ERR_TRACE_EVENTS_CATEGORY_REQUIRED();
  }
  return new Tracing(options.categories);
}

function getEnabledCategories() {
  const seen = new Set();
  for (const tracing of enabledTracingObjects) {
    for (const category of tracing[kCategories]) {
      seen.add(category);
    }
  }
  if (seen.size === 0) {
    return undefined;
  }
  return [...seen].join(",");
}

function nowMicros() {
  return Math.trunc(performance.now() * 1000);
}

function trace(phase, category, name, id, scope) {
  const ph = String.fromCharCode(phase);
  const p = getProc();
  const event = {
    pid: p ? p.pid : 0,
    tid: getThreadId(),
    ts: nowMicros(),
    ph,
    cat: category,
    name,
  };
  if (id !== undefined && id !== null) {
    event.id = "0x" + Number(id).toString(16);
  }
  if (scope !== undefined && scope !== null) {
    event.args = { scope };
  } else {
    event.args = {};
  }
  recordedEvents.push(event);
  ensureExitHandlerInstalled();
}

function writeTraceFile() {
  const p = getProc();
  const pid = p ? p.pid : 0;
  if (isMainThreadProc()) {
    writeMainTraceFile(pid);
  } else {
    writeWorkerSliceFile(pid);
  }
}

let _fsExports = null;
function getFs() {
  if (_fsExports !== null) return _fsExports;
  try {
    _fsExports = core.loadExtScript("ext:deno_node/fs.ts");
  } catch {
    _fsExports = {};
  }
  return _fsExports;
}

function writeMainTraceFile(pid) {
  const fs = getFs();
  const allEvents = recordedEvents.slice();
  // Pull in any worker-thread slices written by this process before exit.
  let entries;
  try {
    entries = fs.readdirSync(".");
  } catch {
    entries = [];
  }
  const prefix = `.deno_trace_events_${pid}_t`;
  for (const entryName of entries) {
    if (typeof entryName !== "string") continue;
    if (!entryName.startsWith(prefix) || !entryName.endsWith(".json")) {
      continue;
    }
    try {
      const text = fs.readFileSync(entryName, "utf-8");
      const slice = JSON.parse(text);
      if (slice && Array.isArray(slice.traceEvents)) {
        for (const ev of slice.traceEvents) allEvents.push(ev);
      }
    } catch {
      // Skip unreadable / partial slice files.
    }
    try {
      fs.unlinkSync(entryName);
    } catch {
      // Best-effort cleanup.
    }
  }
  if (allEvents.length === 0) return;
  let rotation = 1;
  let filename = `node_trace.${rotation}.log`;
  while (existsSync(filename) && rotation < 1000) {
    rotation++;
    filename = `node_trace.${rotation}.log`;
  }
  try {
    fs.writeFileSync(filename, JSON.stringify({ traceEvents: allEvents }));
  } catch {
    // Best-effort exit-time write.
  }
}

function writeWorkerSliceFile(pid) {
  if (recordedEvents.length === 0) return;
  const filename = workerSliceFilename(pid, getThreadId());
  try {
    getFs().writeFileSync(
      filename,
      JSON.stringify({ traceEvents: recordedEvents }),
    );
  } catch {
    // Best-effort exit-time write.
  }
}

function existsSync(path) {
  try {
    getFs().statSync(path);
    return true;
  } catch {
    return false;
  }
}

function ensureExitHandlerInstalled() {
  if (exitHandlerRegistered) return;
  const p = getProc();
  if (!p || !p.on) return;
  exitHandlerRegistered = true;
  p.on("exit", writeTraceFile);
}

function installAsyncHooksTimerTracing() {
  if (originalSetTimeout !== null) return;
  originalSetTimeout = globalThis.setTimeout;
  originalSetInterval = globalThis.setInterval;
  originalSetImmediate = globalThis.setImmediate;

  globalThis.setTimeout = function (cb, ms, ...args) {
    if (typeof cb !== "function") {
      return originalSetTimeout(cb, ms, ...args);
    }
    const id = ++traceIdCounter;
    trace(PHASE_NESTABLE_ASYNC_BEGIN, "node,node.async_hooks", "Timeout", id);
    const wrapped = function () {
      try {
        return cb.apply(this, arguments);
      } finally {
        trace(
          PHASE_NESTABLE_ASYNC_END,
          "node,node.async_hooks",
          "Timeout",
          id,
        );
      }
    };
    return originalSetTimeout(wrapped, ms, ...args);
  };

  if (typeof originalSetImmediate === "function") {
    globalThis.setImmediate = function (cb, ...args) {
      if (typeof cb !== "function") {
        return originalSetImmediate(cb, ...args);
      }
      const id = ++traceIdCounter;
      trace(
        PHASE_NESTABLE_ASYNC_BEGIN,
        "node,node.async_hooks",
        "Immediate",
        id,
      );
      const wrapped = function () {
        try {
          return cb.apply(this, arguments);
        } finally {
          trace(
            PHASE_NESTABLE_ASYNC_END,
            "node,node.async_hooks",
            "Immediate",
            id,
          );
        }
      };
      return originalSetImmediate(wrapped, ...args);
    };
  }
}

function uninstallAsyncHooksTimerTracing() {
  if (originalSetTimeout === null) return;
  globalThis.setTimeout = originalSetTimeout;
  globalThis.setInterval = originalSetInterval;
  if (originalSetImmediate !== null) {
    globalThis.setImmediate = originalSetImmediate;
  }
  originalSetTimeout = null;
  originalSetInterval = null;
  originalSetImmediate = null;
}

// Expose trace + getCategoryEnabledBuffer on the internalBinding('trace_events')
// surface so the Node test fixtures that go through `internal/test/binding`
// observe the same state as the public API.
try {
  const binding = lazyBindingMod().getBinding("trace_events");
  if (binding && typeof binding === "object") {
    binding.getCategoryEnabledBuffer = getCategoryEnabledBuffer;
    binding.trace = trace;
  }
} catch {
  // best-effort: binding registry may not be available in all contexts
}

return {
  default: {
    createTracing,
    getEnabledCategories,
  },
  createTracing,
  getEnabledCategories,
};
})();
