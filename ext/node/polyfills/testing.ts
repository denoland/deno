// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file prefer-primordials

(function () {
"use strict";
const { core, primordials } = __bootstrap;
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  DatePrototypeToString,
  Error,
  ErrorPrototype,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  NumberIsFinite,
  NumberIsInteger,
  ObjectDefineProperty,
  ObjectPrototypeHasOwnProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetPrototypeOf,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  ReflectApply,
  ReflectConstruct,
  SafeArrayIterator,
  SafeMap,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeHas,
  String,
  Symbol,
  SymbolDispose,
  SymbolFor,
  SymbolToPrimitive,
  TypeError,
} = primordials;

let errorHandlersInstalled = false;

let activeNodeTests = 0;

let pendingCallbackReject = null;

function sanitizeThrowValue(err) {
  if (err === null || err === undefined || typeof err !== "object") {
    return err;
  }
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
    return err;
  }
  const inspectSymbol = SymbolFor("nodejs.util.inspect.custom");
  if (typeof err[inspectSymbol] !== "function") {
    return err;
  }
  try {
    Deno.inspect(err);
    return err;
  } catch {
    return new Error(
      "test threw a non-Error object with a throwing custom inspect",
    );
  }
}

function installErrorHandlers() {
  if (errorHandlersInstalled) return;
  errorHandlersInstalled = true;

  globalThis.addEventListener("unhandledrejection", (event) => {
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
  });

  globalThis.addEventListener("error", (event) => {
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const {
  validateFunction,
  validateInteger,
  validateNumber,
  validateObject,
  validateStringArray,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_STATE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { default: assert } = core.loadExtScript("ext:deno_node/assert.ts");
const {
  tapEscape,
  tapIndent,
} = core.loadExtScript("ext:deno_node/internal/test/reporters.ts");

const methodsToCopy = [
  "deepEqual",
  "deepStrictEqual",
  "doesNotMatch",
  "doesNotReject",
  "doesNotThrow",
  "equal",
  "fail",
  "ifError",
  "match",
  "notDeepEqual",
  "notDeepStrictEqual",
  "notEqual",
  "notStrictEqual",
  "partialDeepStrictEqual",
  "rejects",
  "strictEqual",
  "throws",
  "ok",
];

let assertObject = undefined;
function getAssertObject() {
  if (assertObject === undefined) {
    assertObject = { __proto__: null };
    ArrayPrototypeForEach(methodsToCopy, (method) => {
      assertObject[method] = assert[method];
    });
  }
  return assertObject;
}

// Lazy access to other node polyfills; loading these eagerly at module
// init causes circular initialization issues during snapshotting.
let _Readable = null;
function getReadable() {
  if (_Readable === null) {
    _Readable = core.loadExtScript(
      "ext:deno_node/internal/streams/readable.js",
    ).Readable;
  }
  return _Readable;
}
let _fsWatch = null;
function getFsWatch() {
  if (_fsWatch === null) {
    _fsWatch = core.loadExtScript("ext:deno_node/fs.ts").watch;
  }
  return _fsWatch;
}
const lazyProcess = core.createLazyLoader("node:process");

// node:test `run()` implementation.
//
// Returns a `TestsStream`-compatible Readable that emits structured events
// describing the test run lifecycle. We currently support the watch-mode
// event stream (`test:watch:drained`, `test:watch:restarted`) which is the
// minimum required for the Node.js `test-runner/test-run-watch-*` fixtures
// that drive watch behavior through the programmatic API. Actual test file
// discovery / execution remains TODO and is gated behind separate work; the
// stream emits a single empty run cycle, then either ends (watch:false) or
// waits for filesystem changes to trigger restarts (watch:true).
//
// See test-runner/test-run-watch-*.mjs in the Node compat suite for the
// behavior this implements.
function run(options) {
  options = options ?? {};
  const watch = options.watch === true;
  const signal = options.signal;
  let cwd = options.cwd;
  if (cwd === undefined) {
    cwd = lazyProcess().default.cwd();
  }

  const Readable = getReadable();
  const stream = new Readable({
    __proto__: null,
    objectMode: true,
    // We push events imperatively; the consumer just needs a no-op `_read`.
    read() {},
  });

  let watcher = null;
  let finished = false;
  let pendingRestartTimer = null;

  function finish() {
    if (finished) return;
    finished = true;
    if (pendingRestartTimer !== null) {
      clearTimeout(pendingRestartTimer);
      pendingRestartTimer = null;
    }
    if (watcher !== null) {
      try {
        watcher.close();
      } catch { /* ignore */ }
      watcher = null;
    }
    stream.push(null);
  }

  function emit(type) {
    if (finished) return;
    const data = { __proto__: null };
    // Node's TestsStream emits each lifecycle entry both as a data chunk
    // (consumed via async iteration / `'data'` listeners) and as a named
    // event so callers can attach `.on('test:watch:drained', ...)` directly.
    stream.push({ __proto__: null, type, data });
    stream.emit(type, data);
  }

  function drained() {
    emit("test:watch:drained");
  }

  function scheduleRestart() {
    if (finished) return;
    // Debounce bursts of fs events so a single user-visible change produces
    // exactly one restart cycle (Node's watcher coalesces likewise).
    if (pendingRestartTimer !== null) {
      clearTimeout(pendingRestartTimer);
    }
    pendingRestartTimer = setTimeout(() => {
      pendingRestartTimer = null;
      if (finished) return;
      emit("test:watch:restarted");
      drained();
    }, 50);
  }

  if (signal) {
    if (signal.aborted) {
      // Resolve the initial drained on next tick to keep callers that
      // `await once(stream, 'test:watch:drained')` working.
      queueMicrotask(() => {
        drained();
        finish();
      });
      return stream;
    }
    signal.addEventListener("abort", finish, { once: true });
  }

  // Emit the initial "drained" event after the current microtask completes
  // so that consumers attaching `.on('data')` synchronously after `run(...)`
  // returns still observe the event.
  queueMicrotask(() => {
    drained();
    if (!watch) {
      finish();
      return;
    }
    try {
      const fsWatch = getFsWatch();
      watcher = fsWatch(cwd, { recursive: true }, () => {
        scheduleRestart();
      });
      watcher.on("error", () => {
        finish();
      });
    } catch {
      // If we can't watch (e.g. cwd doesn't exist), end the stream gracefully.
      finish();
    }
  });

  return stream;
}

function noop() {}

const skippedSymbol = Symbol("skipped");

// Detect Node.js-compatible `--test-reporter=...` selection so the polyfill
// can emit reporter output matching Node's snapshot fixtures. Deno's CLI does
// not consume `--test-reporter`; instead, our `child_process` polyfill (via
// node_shim) propagates the value through NODE_OPTIONS when one Deno process
// spawns another using Node-style flags. Reading the env var here keeps the
// detection self-contained and avoids new Rust plumbing.
function detectNodeTestReporter() {
  const env = globalThis.Deno?.env;
  if (!env) return null;
  let value = null;
  try {
    value = env.get("NODE_TEST_REPORTER");
  } catch { /* permission denied */ }
  if (value) return value;
  let nodeOptions = "";
  try {
    nodeOptions = env.get("NODE_OPTIONS") || "";
  } catch { /* permission denied */ }
  if (!nodeOptions) return null;
  // Match the first `--test-reporter` occurrence; support both `=value` and
  // space-separated forms. We intentionally do not handle multiple reporters
  // (Node lets you stack reporters with destinations); the snapshot tests use
  // a single reporter and that is what we target.
  const match = nodeOptions.match(/--test-reporter(?:=|\s+)(\S+)/);
  return match ? match[1] : null;
}

// Resolve lazily: testing.ts ships inside Deno's startup snapshot, so any env
// lookups performed at module-evaluation time observe the build environment,
// not the running process. Memoize on the first call so we still only read the
// env once.
let nodeTestReporterCache;
function getNodeTestReporter() {
  if (nodeTestReporterCache !== undefined) return nodeTestReporterCache;
  nodeTestReporterCache = detectNodeTestReporter();
  return nodeTestReporterCache;
}
function isTapMode() {
  return getNodeTestReporter() === "tap";
}

function getTapSuiteALS() {
  if (tapSuiteALS !== null) return tapSuiteALS;
  const mod = core.loadExtScript("ext:deno_node/async_hooks.ts");
  const ALS = mod.AsyncLocalStorage;
  tapSuiteALS = new ALS();
  return tapSuiteALS;
}

function getTapCurrentSuite() {
  if (tapSuiteALS !== null) {
    const fromAls = tapSuiteALS.getStore();
    if (fromAls !== undefined) return fromAls;
  }
  return tapCurrentSuiteSync;
}

// Parse `--test-skip-pattern` from NODE_OPTIONS so the TAP-mode polyfill can
// filter tests Node-style. A bare string is interpreted as a regex source;
// `/.../flags` is a regex literal.
function parsePatternFlag(flag) {
  const env = globalThis.Deno?.env;
  if (!env) return null;
  let nodeOptions = "";
  try {
    nodeOptions = env.get("NODE_OPTIONS") || "";
  } catch { /* permission denied */ }
  if (!nodeOptions) return null;
  const out = [];
  const re = new RegExp(`${flag}(?:=|\\s+)(\\S+)`, "g");
  let m;
  while ((m = re.exec(nodeOptions)) !== null) {
    const value = m[1];
    let pattern;
    const litMatch = value.match(/^\/(.*)\/([a-z]*)$/);
    if (litMatch) {
      try {
        pattern = new RegExp(litMatch[1], litMatch[2]);
      } catch {
        continue;
      }
    } else {
      try {
        pattern = new RegExp(value);
      } catch {
        continue;
      }
    }
    ArrayPrototypePush(out, pattern);
  }
  return out.length > 0 ? out : null;
}

let testSkipPatternCache;
function getTestSkipPatterns() {
  if (testSkipPatternCache !== undefined) return testSkipPatternCache;
  testSkipPatternCache = parsePatternFlag("--test-skip-pattern");
  return testSkipPatternCache;
}

let testOnlyFlagCache;
function isTestOnlyFlagSet() {
  if (testOnlyFlagCache !== undefined) return testOnlyFlagCache;
  const env = globalThis.Deno?.env;
  if (!env) {
    testOnlyFlagCache = false;
    return false;
  }
  let nodeOptions = "";
  try {
    nodeOptions = env.get("NODE_OPTIONS") || "";
  } catch { /* permission denied */ }
  testOnlyFlagCache = /(^|\s)--test-only(\s|=|$)/.test(nodeOptions);
  return testOnlyFlagCache;
}

const TEST_ONLY_WARNING =
  "# 'only' and 'runOnly' require the --test-only command-line option.";

function matchesAnyPattern(name, patterns) {
  for (const p of new SafeArrayIterator(patterns)) {
    if (p.test(name)) return true;
  }
  return false;
}

// Returns true if the given test/suite name should be excluded from the run.
function shouldSkipByPattern(name) {
  const skip = getTestSkipPatterns();
  if (skip && matchesAnyPattern(String(name), skip)) return true;
  return false;
}

// Top-level queue of test/suite entries collected synchronously while the
// script body evaluates. Children of describe() blocks live under their parent
// entry's `children` array, populated during the synchronous descend through
// the describe body.
const tapTopEntries = [];
let tapRunScheduled = false;
// AsyncLocalStorage tracking the currently-active describe() so that test()
// and describe() calls made inside an async describe body - even after
// `await` boundaries - still register against the surrounding suite. The
// fallback variable handles synchronous nesting before ALS is loaded.
let tapCurrentSuiteSync = null;
let tapSuiteALS = null;
const tapStats = {
  tests: 0,
  suites: 0,
  pass: 0,
  fail: 0,
  cancelled: 0,
  skipped: 0,
  todo: 0,
};

function tapWrite(line) {
  // deno-lint-ignore no-console
  console.log(line);
}

function tapDirective(options) {
  if (options.skip) {
    const msg = options.skip === true ? "" : tapEscape(String(options.skip));
    return msg ? ` # SKIP ${msg}` : " # SKIP";
  }
  if (options.todo) {
    const msg = options.todo === true ? "" : tapEscape(String(options.todo));
    return msg ? ` # TODO ${msg}` : " # TODO";
  }
  return "";
}

function tapYaml(depth, type) {
  const pad = tapIndent(depth) + "  ";
  tapWrite(`${pad}---`);
  tapWrite(`${pad}duration_ms: 0`);
  tapWrite(`${pad}type: '${type}'`);
  tapWrite(`${pad}...`);
}

class TapContext {
  #diagnostics = [];
  #name;
  #depth;
  #subtestTail = PromiseResolve();
  #subtestCount = 0;
  #parentChildren;
  // Per-context "warning printed" flag for the `--test-only` diagnostic.
  // Mutated by `runTapEntry` when a child uses `only: true`.
  onlyWarningEmitted = false;

  constructor(name, depth, parentChildren) {
    this.#name = name;
    this.#depth = depth;
    this.#parentChildren = parentChildren;
  }

  get name() {
    return this.#name;
  }

  get fullName() {
    return this.#name;
  }

  get signal() {
    // Provide an AbortSignal so consumers that read t.signal don't crash; the
    // minimal TAP runner does not currently honour aborts.
    return new AbortController().signal;
  }

  get assert() {
    return getAssertObject();
  }

  get mock() {
    return mock;
  }

  diagnostic(message) {
    ArrayPrototypePush(this.#diagnostics, String(message));
  }

  _drainDiagnostics() {
    const out = this.#diagnostics;
    this.#diagnostics = [];
    return out;
  }

  // Subtest registration: `t.test(name, opts?, fn?)` queues a subtest that runs
  // sequentially in the order it was registered. Concurrent calls (Promise.all)
  // are serialized through the parent's subtest tail.
  test(name, options, fn) {
    const prepared = prepareOptions(name, options, fn, {});
    this.#subtestCount++;
    const n = this.#subtestCount;
    const childDepth = this.#depth + 1;
    const entry = {
      name: prepared.name,
      fn: prepared.fn,
      options: prepared.options,
      kind: "test",
      children: [],
      bodyPromise: null,
      bodyError: null,
    };
    if (this.#parentChildren) {
      ArrayPrototypePush(this.#parentChildren, entry);
    }
    // deno-lint-ignore no-this-alias
    const parentState = this;
    const p = PromisePrototypeThen(
      this.#subtestTail,
      () => runTapEntry(entry, childDepth, n, parentState),
    );
    this.#subtestTail = PromisePrototypeThen(p, () => {}, () => {});
    return p;
  }

  _drainSubtests() {
    return this.#subtestTail;
  }

  _subtestCount() {
    return this.#subtestCount;
  }
}

function scheduleTapRun() {
  if (tapRunScheduled) return;
  tapRunScheduled = true;
  // Defer to the macrotask queue so synchronous top-level test() calls finish
  // queueing before we start running.
  setTimeout(() => {
    runTapTop();
  }, 0);
}

async function runTapTop() {
  // Hold the event loop open while tests run so that fixtures using
  // unref'd timers (Node's runner keeps itself alive internally) don't cause
  // Deno to exit before subtests complete.
  const keepAlive = setInterval(() => {}, 1 << 30);
  try {
    // Match Node's ordering: top-level `before()` callbacks fire before the
    // `TAP version 13` line, so any console output they produce appears
    // before the reporter header in the captured stream.
    if (rootBeforeHooks.length > 0) {
      const rootCtx = { name: "<root>", fullName: "<root>" };
      for (const hook of new SafeArrayIterator(rootBeforeHooks)) {
        try {
          const r = ReflectApply(hook, null, [rootCtx]);
          if (isThenable(r)) await r;
        } catch { /* swallow to keep parity with Node's lenient hook errors */ }
      }
    }
    tapWrite("TAP version 13");
    let n = 0;
    const topState = { onlyWarningEmitted: false };
    for (const entry of new SafeArrayIterator(tapTopEntries)) {
      n++;
      await runTapEntry(entry, 0, n, topState);
    }
    // Drain top-level `after()` hooks before printing the plan/summary so
    // their console output appears between the last test and the `1..N` line.
    if (rootAfterHooks.length > 0) {
      const rootCtx = { name: "<root>", fullName: "<root>" };
      const hooks = ArrayPrototypeSplice(
        rootAfterHooks,
        0,
        rootAfterHooks.length,
      );
      for (const hook of new SafeArrayIterator(hooks)) {
        try {
          const r = ReflectApply(hook, null, [rootCtx]);
          if (isThenable(r)) await r;
        } catch { /* swallow */ }
      }
    }
    tapWrite(`1..${n}`);
    tapWrite(`# tests ${tapStats.tests}`);
    tapWrite(`# suites ${tapStats.suites}`);
    tapWrite(`# pass ${tapStats.pass}`);
    tapWrite(`# fail ${tapStats.fail}`);
    tapWrite(`# cancelled ${tapStats.cancelled}`);
    tapWrite(`# skipped ${tapStats.skipped}`);
    tapWrite(`# todo ${tapStats.todo}`);
    tapWrite(`# duration_ms 0`);
  } finally {
    clearInterval(keepAlive);
  }
  if (tapStats.fail > 0 || tapStats.cancelled > 0) {
    try {
      globalThis.Deno?.exit?.(1);
    } catch { /* exit unavailable */ }
  }
}

// Recursively run a test or suite entry, emitting TAP output at the given
// nesting depth. `parentState` is the runtime state of the immediate parent
// (a TapContext for test-bodies, or a `{ onlyWarningEmitted }` object for
// suite/root scopes); it's used to emit the `# 'only' and 'runOnly' require
// the --test-only command-line option.` warning at most once per parent.
async function runTapEntry(entry, depth, n, parentState) {
  const indent = tapIndent(depth);
  const isSuite = entry.kind === "suite";
  tapWrite(`${indent}# Subtest: ${tapEscape(entry.name)}`);
  const directive = tapDirective(entry.options);

  let status = "ok";
  let diagnostics = [];
  let childCount = 0;

  // Each test/suite body gets its own state object so warnings emitted
  // because of an `only: true` child don't leak across siblings.
  const myChildrenState = { onlyWarningEmitted: false };

  if (entry.options.skip) {
    if (isSuite) {
      tapStats.suites++;
    } else {
      tapStats.skipped++;
      tapStats.tests++;
    }
  } else if (entry.options.todo) {
    // Per Node behavior, a TODO test that throws is not counted as a failure.
    // The runner skips invoking the body to match snapshot output.
    if (isSuite) {
      tapStats.suites++;
    } else {
      tapStats.todo++;
      tapStats.tests++;
    }
  } else if (isSuite) {
    try {
      if (entry.bodyError) throw entry.bodyError;
      if (entry.bodyPromise) await entry.bodyPromise;
      let childN = 0;
      for (const child of new SafeArrayIterator(entry.children)) {
        childN++;
        await runTapEntry(child, depth + 1, childN, myChildrenState);
      }
      childCount = childN;
    } catch (_err) {
      status = "not ok";
      tapStats.fail++;
    }
    tapStats.suites++;
  } else {
    // test/it body
    const ctx = new TapContext(entry.name, depth, entry.children);
    try {
      const ret = ReflectApply(entry.fn, ctx, [ctx]);
      if (isThenable(ret)) await ret;
      // Wait for any concurrent t.test() calls (e.g. Promise.all([...])).
      await ctx._drainSubtests();
      childCount = ctx._subtestCount();
      tapStats.pass++;
    } catch (_err) {
      status = "not ok";
      tapStats.fail++;
      // Even on failure, drain any in-flight subtests so their output is
      // emitted before the parent's "not ok" line.
      try {
        await ctx._drainSubtests();
      } catch { /* ignore */ }
      childCount = ctx._subtestCount();
    }
    diagnostics = ctx._drainDiagnostics();
    tapStats.tests++;
  }

  if (childCount > 0) {
    tapWrite(`${tapIndent(depth + 1)}1..${childCount}`);
  }

  tapWrite(`${indent}${status} ${n} - ${tapEscape(entry.name)}${directive}`);
  tapYaml(depth, isSuite ? "suite" : "test");
  for (const d of new SafeArrayIterator(diagnostics)) {
    tapWrite(`${indent}# ${tapEscape(d)}`);
  }
  // If this entry was registered with `only: true` and the `--test-only` flag
  // wasn't supplied, Node prints a one-off warning in the parent's scope
  // immediately after the entry's yaml/diagnostics. Emit it at the same depth
  // and only once per parent.
  if (
    entry.options.only &&
    !isTestOnlyFlagSet() &&
    parentState &&
    !parentState.onlyWarningEmitted
  ) {
    tapWrite(`${indent}${TEST_ONLY_WARNING}`);
    parentState.onlyWarningEmitted = true;
  }
}

function queueTapTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);
  if (shouldSkipByPattern(prepared.name)) {
    // Filtered out entirely: do not register or run.
    scheduleTapRun();
    return PromiseResolve();
  }
  const entry = {
    name: prepared.name,
    fn: prepared.fn,
    options: prepared.options,
    kind: "test",
    children: [],
    bodyPromise: null,
    bodyError: null,
  };
  const parentSuite = getTapCurrentSuite();
  if (parentSuite !== null) {
    ArrayPrototypePush(parentSuite.children, entry);
  } else {
    ArrayPrototypePush(tapTopEntries, entry);
    scheduleTapRun();
  }
  return PromiseResolve();
}

function queueTapSuite(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);
  if (shouldSkipByPattern(prepared.name)) {
    // Filtered out entirely: do not register, but the suite body must not
    // run (it would otherwise add unwanted children to the parent).
    scheduleTapRun();
    return PromiseResolve();
  }
  const entry = {
    name: prepared.name,
    fn: prepared.fn,
    options: prepared.options,
    kind: "suite",
    children: [],
    bodyPromise: null,
    bodyError: null,
  };
  const parentSuite = getTapCurrentSuite();
  if (parentSuite !== null) {
    ArrayPrototypePush(parentSuite.children, entry);
  } else {
    ArrayPrototypePush(tapTopEntries, entry);
    scheduleTapRun();
  }
  // Evaluate the suite body inside an AsyncLocalStorage scope so any nested
  // describe()/test() calls - including those scheduled after `await` inside
  // the body - register against this suite, not the outer (or null) scope.
  // The sync fallback handles environments without ALS available.
  const als = getTapSuiteALS();
  const prev = tapCurrentSuiteSync;
  tapCurrentSuiteSync = entry;
  try {
    als.run(entry, () => {
      try {
        const ret = ReflectApply(prepared.fn, null, []);
        if (isThenable(ret)) entry.bodyPromise = ret;
      } catch (err) {
        entry.bodyError = err;
      }
    });
  } finally {
    tapCurrentSuiteSync = prev;
  }
  return PromiseResolve();
}

function isThenable(value) {
  return value !== null && value !== undefined &&
    typeof value.then === "function";
}

function getExpectFailureMatch(expectFailure) {
  if (
    expectFailure !== null && typeof expectFailure === "object" &&
    ObjectPrototypeHasOwnProperty(expectFailure, "match")
  ) {
    return expectFailure.match;
  }
  if (
    expectFailure === true ||
    typeof expectFailure === "string" ||
    expectFailure === undefined
  ) {
    return undefined;
  }
  return expectFailure;
}

function assertExpectedFailure(err, expectFailure) {
  const match = getExpectFailureMatch(expectFailure);
  if (match !== undefined) {
    assert.throws(() => {
      throw err;
    }, match);
  }
}

async function runNodeTestFunction(fn, nodeTestContext) {
  if (fn.length >= 2) {
    // Node-style callback API: fn(t, done) - wait for `done()` (or promise
    // rejection) before treating the test as complete.
    await new Promise((testResolve, testReject) => {
      pendingCallbackReject = testReject;
      const done = (err) => {
        pendingCallbackReject = null;
        if (err) testReject(err);
        else testResolve(undefined);
      };
      try {
        const result = ReflectApply(fn, nodeTestContext, [
          nodeTestContext,
          done,
        ]);
        if (isThenable(result)) {
          PromisePrototypeThen(result, undefined, (err) => {
            pendingCallbackReject = null;
            testReject(err);
          });
        }
      } catch (err) {
        pendingCallbackReject = null;
        testReject(err);
      }
    });
    return undefined;
  }
  return await ReflectApply(fn, nodeTestContext, [nodeTestContext]);
}

async function runPossiblyExpectingFailure(fn, nodeTestContext, options) {
  if (
    !options.expectFailure ||
    options.skip ||
    options.todo
  ) {
    const result = await runNodeTestFunction(fn, nodeTestContext);
    nodeTestContext._checkPlan();
    return result;
  }

  let failed = false;
  try {
    await runNodeTestFunction(fn, nodeTestContext);
    nodeTestContext._checkPlan();
  } catch (err) {
    failed = true;
    assertExpectedFailure(err, options.expectFailure);
  }

  if (!failed) {
    throw new Error("test was expected to fail but passed");
  }
  return undefined;
}

class TestPlan {
  #expected;
  #actual = 0;

  constructor(count) {
    this.#expected = count;
  }

  increment() {
    this.#actual++;
  }

  check() {
    if (this.#actual !== this.#expected) {
      throw new Error(
        `plan expected ${this.#expected} assertion(s) but received ${this.#actual}`,
      );
    }
  }
}

class NodeTestContext {
  #denoContext;
  #afterHooks = [];
  #beforeHooks = [];
  #parent;
  #skipped = false;
  #name;
  #abortController = new AbortController();
  #plan;
  #planAssert;
  #beforeEachHooks = [];
  #afterEachHooks = [];

  constructor(t, parent, name) {
    this.#denoContext = t;
    this.#parent = parent;
    this.#name = name;
  }

  get [skippedSymbol]() {
    return this.#skipped || (this.#parent?.[skippedSymbol] ?? false);
  }

  get assert() {
    if (this.#plan) {
      if (!this.#planAssert) {
        const plan = this.#plan;
        const base = getAssertObject();
        const wrapped = { __proto__: null };
        ArrayPrototypeForEach(methodsToCopy, (method) => {
          wrapped[method] = function (...args) {
            plan.increment();
            return ReflectApply(base[method], this, args);
          };
        });
        this.#planAssert = wrapped;
      }
      return this.#planAssert;
    }
    return getAssertObject();
  }

  plan(count) {
    validateInteger(count, "count", 1);
    this.#plan = new TestPlan(count);
  }

  _checkPlan() {
    if (this.#plan) this.#plan.check();
  }

  get signal() {
    return this.#abortController.signal;
  }

  get name() {
    return this.#name;
  }

  get fullName() {
    if (this.#parent) {
      return this.#parent.fullName + " > " + this.#name;
    }
    return this.#name;
  }

  diagnostic(message) {
    // deno-lint-ignore no-console
    console.log("DIAGNOSTIC:", message);
  }

  get mock() {
    return mock;
  }

  runOnly() {
    notImplemented("test.TestContext.runOnly");
    return null;
  }

  skip() {
    this.#skipped = true;
    return null;
  }

  todo() {
    this.#skipped = true;
    return null;
  }

  test(name, options, fn) {
    const prepared = prepareOptions(name, options, fn, {});
    if (this.#plan) this.#plan.increment();
    // deno-lint-ignore no-this-alias
    const parentContext = this;
    const after = async () => {
      for (const hook of new SafeArrayIterator(this.#afterHooks)) {
        await hook();
      }
    };
    const before = async () => {
      for (const hook of new SafeArrayIterator(this.#beforeHooks)) {
        await hook();
      }
    };
    return PromisePrototypeThen(
      this.#denoContext.step({
        name: prepared.name,
        fn: async (denoTestContext) => {
          const newNodeTextContext = new NodeTestContext(
            denoTestContext,
            parentContext,
            prepared.name,
          );
          try {
            await before();
            for (
              const hook of new SafeArrayIterator(
                parentContext.#beforeEachHooks,
              )
            ) {
              await hook();
            }
            await runPossiblyExpectingFailure(
              prepared.fn,
              newNodeTextContext,
              prepared.options,
            );
            await after();
          } catch (err) {
            if (!newNodeTextContext[skippedSymbol]) {
              throw err;
            }
            try {
              await after();
            } catch { /* ignore, test is already failing */ }
          } finally {
            for (
              const hook of new SafeArrayIterator(
                parentContext.#afterEachHooks,
              )
            ) {
              await hook();
            }
          }
        },
        ignore: !!prepared.options.todo || !!prepared.options.skip,
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      }),
      () => undefined,
    );
  }

  before(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("before() requires a function");
    }
    ArrayPrototypePush(this.#beforeHooks, fn);
  }

  after(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("after() requires a function");
    }
    ArrayPrototypePush(this.#afterHooks, fn);
  }

  beforeEach(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("beforeEach() requires a function");
    }
    ArrayPrototypePush(this.#beforeEachHooks, fn);
  }

  afterEach(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("afterEach() requires a function");
    }
    ArrayPrototypePush(this.#afterEachHooks, fn);
  }
}

let currentSuite = null;

const rootBeforeHooks = [];
const rootAfterHooks = [];
const rootBeforeEachHooks = [];
const rootAfterEachHooks = [];
let rootBeforeRan = false;

async function runRootBeforeOnce() {
  if (rootBeforeRan) return;
  rootBeforeRan = true;
  if (rootBeforeHooks.length === 0) return;
  const rootCtx = { name: "<root>", fullName: "<root>" };
  for (const hook of new SafeArrayIterator(rootBeforeHooks)) {
    await hook(rootCtx);
  }
}

async function runRootAfterIfDone() {
  if (activeNodeTests !== 0) return;
  if (rootAfterHooks.length === 0) return;
  const rootCtx = { name: "<root>", fullName: "<root>" };
  // Snapshot and clear so we only run once even if more tests get queued.
  const hooks = ArrayPrototypeSplice(rootAfterHooks, 0, rootAfterHooks.length);
  for (const hook of new SafeArrayIterator(hooks)) {
    try {
      await hook(rootCtx);
    } catch { /* ignore */ }
  }
}

class TestSuite {
  #denoTestContext;
  nodeTestContext;
  entries = [];
  beforeAllHooks = [];
  afterAllHooks = [];
  beforeEachHooks = [];
  afterEachHooks = [];

  constructor(t, nodeTestContext) {
    this.#denoTestContext = t;
    this.nodeTestContext = nodeTestContext;
  }

  addTest(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const beforeEach = this.beforeEachHooks;
    const afterEach = this.afterEachHooks;
    const suiteNodeContext = this.nodeTestContext;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(
          denoTestContext,
          suiteNodeContext,
          prepared.name,
        );
        try {
          for (const hook of new SafeArrayIterator(beforeEach)) {
            await hook(newNodeTextContext);
          }
          return await runPossiblyExpectingFailure(
            prepared.fn,
            newNodeTextContext,
            prepared.options,
          );
        } catch (err) {
          if (newNodeTextContext[skippedSymbol]) {
            return undefined;
          } else {
            throw err;
          }
        } finally {
          for (const hook of new SafeArrayIterator(afterEach)) {
            try {
              await hook(newNodeTextContext);
            } catch { /* ignore */ }
          }
        }
      },
      ignore: !!prepared.options.todo || !!prepared.options.skip,
    });
  }

  addSuite(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const { promise, resolve } = Promise.withResolvers();
    const parentSuiteContext = this.nodeTestContext;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: wrapSuiteFn(prepared.fn, resolve, prepared.name, parentSuiteContext),
      ignore: !!prepared.options.todo || !!prepared.options.skip,
    });
    return promise;
  }

  async execute() {
    for (const entry of new SafeArrayIterator(this.entries)) {
      await this.#denoTestContext.step({
        name: entry.name,
        fn: entry.fn,
        ignore: entry.ignore,
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      });
    }
  }
}

function prepareOptions(name, options, fn, overrides) {
  if (typeof name === "function") {
    fn = name;
  } else if (name !== null && typeof name === "object") {
    fn = options;
    options = name;
  } else if (typeof options === "function") {
    fn = options;
  }

  if (options === null || typeof options !== "object") {
    options = {};
  }

  const finalOptions = { ...options, ...overrides };

  if (typeof fn !== "function") {
    fn = noop;
  }

  if (typeof name !== "string" || name === "") {
    name = fn.name || "<anonymous>";
  }

  return { fn, options: finalOptions, name };
}

function wrapTestFn(fn, resolve, name, options) {
  return async function (t) {
    const nodeTestContext = new NodeTestContext(t, undefined, name);
    let beforeEachOk = false;
    try {
      await runRootBeforeOnce();
      for (const hook of new SafeArrayIterator(rootBeforeEachHooks)) {
        await hook(nodeTestContext);
      }
      beforeEachOk = true;
      await runPossiblyExpectingFailure(fn, nodeTestContext, options);
    } catch (err) {
      if (!nodeTestContext[skippedSymbol]) {
        throw sanitizeThrowValue(err);
      }
    } finally {
      if (beforeEachOk) {
        for (const hook of new SafeArrayIterator(rootAfterEachHooks)) {
          try {
            await hook(nodeTestContext);
          } catch { /* swallow to match node behavior on hook error */ }
        }
      }
      activeNodeTests--;
      await runRootAfterIfDone();
      resolve();
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  activeNodeTests++;

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapTestFn(prepared.fn, noop, prepared.name, prepared.options),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  // Node resolves the returned promise on test completion, but the
  // Deno runner only executes registered tests after the module
  // finishes evaluating, so top-level `await test(...)` deadlocks.
  // Resolve immediately to unblock; the test still runs and is
  // reported normally. Trade-off: code that awaits `test()` for
  // sequencing (`await test('a'); await test('b')`) sees them run
  // out of order.
  return PromiseResolve();
}

function wrapSuiteFn(fn, resolve, name, parentNodeContext) {
  return async function (t) {
    const isTopLevel = parentNodeContext === undefined;
    if (isTopLevel) await runRootBeforeOnce();
    const suiteNodeContext = new NodeTestContext(t, parentNodeContext, name);
    const prevSuite = currentSuite;
    const suite = currentSuite = new TestSuite(t, suiteNodeContext);
    try {
      fn(suiteNodeContext);
    } finally {
      currentSuite = prevSuite;
    }
    try {
      for (const hook of new SafeArrayIterator(suite.beforeAllHooks)) {
        await hook();
      }
      await suite.execute();
    } finally {
      try {
        for (const hook of new SafeArrayIterator(suite.afterAllHooks)) {
          await hook();
        }
      } finally {
        if (isTopLevel) {
          activeNodeTests--;
          await runRootAfterIfDone();
        }
        resolve();
      }
    }
  };
}

function prepareDenoTestForSuite(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  activeNodeTests++;

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapSuiteFn(prepared.fn, noop, prepared.name, undefined),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  // See `prepareDenoTest` for the Node-divergence trade-off; top-level
  // `await suite(...)` would deadlock if we waited for completion.
  return PromiseResolve();
}

function test(name, options, fn, overrides) {
  installErrorHandlers();
  if (isTapMode()) {
    return queueTapTest(name, options, fn, overrides);
  }
  if (currentSuite) {
    return currentSuite.addTest(name, options, fn, overrides);
  }
  return prepareDenoTest(name, options, fn, overrides);
}

test.skip = function skip(name, options, fn) {
  return test(name, options, fn, { skip: true });
};

test.todo = function todo(name, options, fn) {
  return test(name, options, fn, { todo: true });
};

test.only = function only(name, options, fn) {
  return test(name, options, fn, { only: true });
};

test.expectFailure = function expectFailure(name, options, fn) {
  return test(name, options, fn, { expectFailure: true });
};

function suite(name, options, fn, overrides) {
  installErrorHandlers();
  if (isTapMode()) {
    return queueTapSuite(name, options, fn, overrides);
  }
  if (currentSuite) {
    return currentSuite.addSuite(name, options, fn, overrides);
  }
  return prepareDenoTestForSuite(name, options, fn, overrides);
}

suite.skip = function skip(name, options, fn) {
  return suite(name, options, fn, { skip: true });
};
suite.todo = function todo(name, options, fn) {
  return suite(name, options, fn, { todo: true });
};
suite.only = function only(name, options, fn) {
  return suite(name, options, fn, { only: true });
};

const it = test;
const describe = suite;

function before(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("before() requires a function argument");
  }
  if (isTapMode()) {
    const tapSuite = getTapCurrentSuite();
    if (tapSuite !== null) {
      ArrayPrototypePush(tapSuite.beforeAllHooks ??= [], fn);
      return;
    }
    ArrayPrototypePush(rootBeforeHooks, fn);
    // A bare top-level `before()` with no tests must still produce TAP
    // output (`before` runs, then `TAP version 13`, then `1..0`).
    scheduleTapRun();
    return;
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeAllHooks, fn);
    return;
  }
  ArrayPrototypePush(rootBeforeHooks, fn);
}

function after(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("after() requires a function argument");
  }
  if (isTapMode()) {
    const tapSuite = getTapCurrentSuite();
    if (tapSuite !== null) {
      ArrayPrototypePush(tapSuite.afterAllHooks ??= [], fn);
      return;
    }
    ArrayPrototypePush(rootAfterHooks, fn);
    scheduleTapRun();
    return;
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterAllHooks, fn);
    return;
  }
  ArrayPrototypePush(rootAfterHooks, fn);
}

function beforeEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("beforeEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeEachHooks, fn);
    return;
  }
  ArrayPrototypePush(rootBeforeEachHooks, fn);
}

function afterEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("afterEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterEachHooks, fn);
    return;
  }
  ArrayPrototypePush(rootAfterEachHooks, fn);
}

test.it = test;
test.describe = suite;
test.suite = suite;
test.before = before;
test.after = after;
test.beforeEach = beforeEach;
test.afterEach = afterEach;

const activeMocks = [];

class MockFunctionContext {
  #calls = [];
  #implementation;
  #restore;
  #times;
  #onceImplementations = new SafeMap();

  constructor(implementation, restore, times) {
    this.#implementation = implementation;
    this.#restore = restore;
    this.#times = times;
  }

  get calls() {
    return this.#calls;
  }

  callCount() {
    return this.#calls.length;
  }

  mockImplementation(implementation) {
    validateFunction(implementation, "implementation");
    this.#implementation = implementation;
  }

  mockImplementationOnce(implementation, onCall) {
    validateFunction(implementation, "implementation");
    if (onCall !== undefined) {
      validateInteger(onCall, "onCall", 0);
    }
    const call = onCall ?? this.#calls.length;
    MapPrototypeSet(this.#onceImplementations, call, implementation);
  }

  resetCalls() {
    ArrayPrototypeSplice(this.#calls, 0, this.#calls.length);
  }

  restore() {
    if (this.#restore) {
      this.#restore();
      this.#restore = undefined;
    }
    this._restored = true;
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

  _recordCall(thisArg, args, result, error, target) {
    ArrayPrototypePush(this.#calls, {
      arguments: args,
      error,
      result,
      stack: new Error(),
      target,
      this: thisArg,
    });
  }

  _shouldMock() {
    if (this._restored) return false;
    if (this.#times === undefined) return true;
    return this.#calls.length < this.#times;
  }

  _getImplementation() {
    return this.#implementation;
  }

  _nextImpl() {
    const nextCall = this.#calls.length;
    const onceImpl = MapPrototypeGet(this.#onceImplementations, nextCall);
    if (onceImpl) {
      MapPrototypeDelete(this.#onceImplementations, nextCall);
      return onceImpl;
    }
    return this.#implementation;
  }
}

function createMockFunction(original, implementation, ctx) {
  const mockFn = function (...args) {
    const newTarget = new.target;
    const isCtor = newTarget !== undefined;
    // The IIFE wrapping this module is sloppy, so a plain call leaks
    // globalThis as `this`. Match strict-mode/Node semantics.
    const thisArg = !isCtor && this === globalThis ? undefined : this;
    const impl = ctx._shouldMock()
      ? (ctx._nextImpl() ?? implementation ?? original)
      : original;

    let result;
    let error;

    // If called directly (not via subclass), use the original constructor
    // so the produced instance has its prototype, and so call.target reports
    // the user's class (not the mock wrapper).
    const ctorTarget = isCtor && newTarget === mockFn ? impl : newTarget;
    try {
      if (isCtor) {
        result = impl ? ReflectConstruct(impl, args, ctorTarget) : undefined;
      } else {
        result = impl ? ReflectApply(impl, thisArg, args) : undefined;
      }
    } catch (e) {
      error = e;
      ctx._recordCall(
        isCtor ? thisArg : thisArg,
        args,
        undefined,
        error,
        ctorTarget,
      );
      throw e;
    }

    ctx._recordCall(
      isCtor ? result : thisArg,
      args,
      result,
      undefined,
      ctorTarget,
    );
    return result;
  };

  ObjectDefineProperty(mockFn, "mock", {
    __proto__: null,
    value: ctx,
    writable: false,
    enumerable: false,
    configurable: false,
  });

  return mockFn;
}

function findPropertyDescriptor(obj, name) {
  let current = obj;
  while (current !== null && current !== undefined) {
    const desc = ObjectGetOwnPropertyDescriptor(current, name);
    if (desc) return desc;
    current = ObjectGetPrototypeOf(current);
  }
  return undefined;
}

function mockMethodImpl(object, methodName, implementation, options) {
  if (
    implementation !== null && typeof implementation === "object" &&
    typeof implementation !== "function"
  ) {
    options = implementation;
    implementation = undefined;
  }

  const descriptor = findPropertyDescriptor(object, methodName);
  if (!descriptor) {
    throw new TypeError(
      `Cannot mock property '${String(methodName)}' because it does not exist`,
    );
  }

  const isGetter = options?.getter ?? false;
  const isSetter = options?.setter ?? false;

  let original;
  if (isGetter) {
    original = descriptor.get;
  } else if (isSetter) {
    original = descriptor.set;
  } else {
    original = descriptor.value;
  }

  if (typeof original !== "function") {
    throw new TypeError(
      `Cannot mock property '${
        String(methodName)
      }' because it is not a function`,
    );
  }

  const restore = () => {
    ObjectDefineProperty(object, methodName, descriptor);
  };

  const impl = implementation === undefined ? original : implementation;
  const ctx = new MockFunctionContext(impl, restore, options?.times);
  ArrayPrototypePush(activeMocks, ctx);

  const mockFn = createMockFunction(original, impl, ctx);

  const mockDescriptor = {
    configurable: descriptor.configurable,
    enumerable: descriptor.enumerable,
  };

  if (isGetter) {
    mockDescriptor.get = mockFn;
    mockDescriptor.set = descriptor.set;
  } else if (isSetter) {
    mockDescriptor.get = descriptor.get;
    mockDescriptor.set = mockFn;
  } else {
    mockDescriptor.writable = descriptor.writable;
    mockDescriptor.value = mockFn;
  }

  ObjectDefineProperty(object, methodName, mockDescriptor);

  return mockFn;
}

const SUPPORTED_APIS = [
  "setTimeout",
  "setInterval",
  "setImmediate",
  "Date",
];

class MockTimersHandle {
  #id;
  #timer;
  #timers;
  #refed;
  constructor(timer, timers, refed) {
    this.#id = timer.id;
    this.#timer = timer;
    this.#timers = timers;
    this.#refed = refed;
  }
  ref() {
    this.#refed = true;
    return this;
  }
  unref() {
    this.#refed = false;
    return this;
  }
  hasRef() {
    return this.#refed;
  }
  refresh() {
    if (this.#timer && !this.#timer.interval) {
      this.#timer.fireAt = this.#timers._now + this.#timer.delay;
      MapPrototypeSet(this.#timers._timers, this.#id, this.#timer);
    }
    return this;
  }
  [SymbolToPrimitive]() {
    return this.#id;
  }
  [SymbolFor("Deno.customInspect")]() {
    return `MockTimer { id: ${this.#id} }`;
  }
  get _id() {
    return this.#id;
  }
}

class MockTimers {
  _enabled = false;
  _now = 0;
  _timers = new SafeMap();
  _nextId = 1;
  #originals = new SafeMap();

  #mockGlobal(name, value) {
    if (!MapPrototypeHas(this.#originals, name)) {
      MapPrototypeSet(this.#originals, name, globalThis[name]);
    }
    globalThis[name] = value;
  }

  enable(options = { __proto__: null }) {
    if (this._enabled) {
      throw new ERR_INVALID_STATE(
        "MockTimers is already enabled. Reset it first to enable it again",
      );
    }
    validateObject(options, "options");
    let { apis, now } = options;
    if (apis === undefined) {
      apis = SUPPORTED_APIS;
    } else {
      validateStringArray(apis, "options.apis");
      for (let i = 0; i < apis.length; i++) {
        if (!ArrayPrototypeIncludes(SUPPORTED_APIS, apis[i])) {
          throw new ERR_INVALID_ARG_VALUE(
            `options.apis[${i}]`,
            apis[i],
            `must be one of ${ArrayPrototypeJoin(SUPPORTED_APIS, ", ")}`,
          );
        }
      }
    }
    if (now === undefined) {
      now = 0;
    } else if (ObjectPrototypeIsPrototypeOf(originalDatePrototype, now)) {
      now = originalDateGetTime.call(now);
    }
    validateNumber(now, "options.now", 0);
    if (!NumberIsFinite(now) || !NumberIsInteger(now)) {
      throw new ERR_INVALID_ARG_VALUE(
        "options.now",
        now,
        "must be a finite, non-negative integer or a Date object",
      );
    }

    this._enabled = true;
    this._now = now;

    for (let i = 0; i < apis.length; i++) {
      const api = apis[i];
      if (api === "Date") {
        this.#mockGlobal("Date", createMockDate(this));
      } else if (api === "setTimeout") {
        this.#mockGlobal(
          "setTimeout",
          (callback, delay, ...args) =>
            this._setTimeout(callback, delay, args, false),
        );
        this.#mockGlobal("clearTimeout", (handle) => this._clearTimer(handle));
      } else if (api === "setInterval") {
        this.#mockGlobal(
          "setInterval",
          (callback, delay, ...args) =>
            this._setInterval(callback, delay, args),
        );
        this.#mockGlobal("clearInterval", (handle) => this._clearTimer(handle));
      } else if (api === "setImmediate") {
        this.#mockGlobal(
          "setImmediate",
          (callback, ...args) => this._setTimeout(callback, 0, args, true),
        );
        this.#mockGlobal(
          "clearImmediate",
          (handle) => this._clearTimer(handle),
        );
      }
    }
  }

  reset() {
    if (!this._enabled) return;
    for (const [name, original] of this.#originals.entries()) {
      if (name === "Date") {
        globalThis.Date = original;
      } else {
        globalThis[name] = original;
      }
    }
    this.#originals.clear();
    this._timers.clear();
    this._now = 0;
    this._nextId = 1;
    this._enabled = false;
  }

  tick(milliseconds = 0) {
    if (!this._enabled) {
      throw new ERR_INVALID_STATE(
        "You should enable MockTimers first by calling the .enable function",
      );
    }
    validateNumber(milliseconds, "milliseconds", 0);
    if (!NumberIsFinite(milliseconds)) {
      throw new ERR_INVALID_ARG_VALUE(
        "milliseconds",
        milliseconds,
        "must be a finite number",
      );
    }
    const target = this._now + milliseconds;
    while (true) {
      const next = this.#findNextTimer();
      if (next === null || next.fireAt > target) break;
      this._now = next.fireAt;
      this.#fireTimer(next);
    }
    this._now = target;
  }

  runAll() {
    if (!this._enabled) {
      throw new ERR_INVALID_STATE(
        "You should enable MockTimers first by calling the .enable function",
      );
    }
    // Intervals re-arm in `_timers` after firing (their `fireAt` is bumped)
    // instead of being deleted, so without bookkeeping `runAll()` with an
    // active interval loops forever. Match Node and fire each registered
    // timer at most once: track ids that have already fired and stop when
    // `#findNextTimer()` returns one of them.
    const fired = new SafeSet();
    while (true) {
      const next = this.#findNextTimer();
      if (next === null || SetPrototypeHas(fired, next.id)) break;
      SetPrototypeAdd(fired, next.id);
      this._now = next.fireAt;
      this.#fireTimer(next);
    }
  }

  setTime(milliseconds) {
    if (!this._enabled) {
      throw new ERR_INVALID_STATE(
        "You should enable MockTimers first by calling the .enable function",
      );
    }
    validateNumber(milliseconds, "milliseconds", 0);
    if (!NumberIsFinite(milliseconds)) {
      throw new ERR_INVALID_ARG_VALUE(
        "milliseconds",
        milliseconds,
        "must be a finite number",
      );
    }
    this._now = milliseconds;
  }

  [SymbolDispose]() {
    this.reset();
  }

  _setTimeout(callback, delay, args, immediate) {
    validateFunction(callback, "callback");
    if (delay === undefined || delay === null) delay = 1;
    if (typeof delay !== "number") delay = +delay;
    if (!NumberIsFinite(delay) || delay < 0) delay = 1;
    if (delay > 2147483647) delay = 1;
    const id = this._nextId++;
    const timer = {
      id,
      callback,
      args,
      delay,
      fireAt: this._now + delay,
      interval: null,
      immediate,
    };
    MapPrototypeSet(this._timers, id, timer);
    return new MockTimersHandle(timer, this, true);
  }

  _setInterval(callback, delay, args) {
    validateFunction(callback, "callback");
    if (delay === undefined || delay === null) delay = 1;
    if (typeof delay !== "number") delay = +delay;
    if (!NumberIsFinite(delay) || delay < 1) delay = 1;
    if (delay > 2147483647) delay = 1;
    const id = this._nextId++;
    const timer = {
      id,
      callback,
      args,
      delay,
      fireAt: this._now + delay,
      interval: delay,
      immediate: false,
    };
    MapPrototypeSet(this._timers, id, timer);
    return new MockTimersHandle(timer, this, true);
  }

  _clearTimer(handle) {
    if (handle === null || handle === undefined) return;
    let id;
    if (typeof handle === "number") {
      id = handle;
    } else if (typeof handle === "object" && typeof handle._id === "number") {
      id = handle._id;
    } else {
      return;
    }
    MapPrototypeDelete(this._timers, id);
  }

  #findNextTimer() {
    let next = null;
    for (const t of this._timers.values()) {
      if (
        next === null ||
        t.fireAt < next.fireAt ||
        (t.fireAt === next.fireAt &&
          (t.immediate !== next.immediate ? t.immediate : t.id < next.id))
      ) {
        next = t;
      }
    }
    return next;
  }

  #fireTimer(timer) {
    if (timer.interval !== null) {
      timer.fireAt += timer.interval;
    } else {
      MapPrototypeDelete(this._timers, timer.id);
    }
    try {
      ReflectApply(timer.callback, undefined, timer.args);
    } catch (err) {
      // Surface the error asynchronously via the original setTimeout so a
      // single bad callback doesn't abort tick().
      const originalSetTimeout = MapPrototypeGet(this.#originals, "setTimeout");
      const fallback = originalSetTimeout ?? globalThis.setTimeout;
      fallback(() => {
        throw err;
      }, 0);
    }
  }
}

const originalDate = globalThis.Date;
const originalDatePrototype = originalDate.prototype;
const originalDateGetTime = originalDate.prototype.getTime;

function createMockDate(mockTimers) {
  function MockDate(...args) {
    if (!new.target) {
      return DatePrototypeToString(
        ReflectConstruct(originalDate, [mockTimers._now], MockDate),
      );
    }
    if (args.length === 0) {
      return ReflectConstruct(originalDate, [mockTimers._now], MockDate);
    }
    return ReflectConstruct(originalDate, args, MockDate);
  }
  ObjectDefineProperty(MockDate, "prototype", {
    value: originalDate.prototype,
    writable: false,
  });
  ObjectDefineProperty(MockDate, "name", {
    value: "Date",
    configurable: true,
  });
  MockDate.now = () => mockTimers._now;
  MockDate.parse = originalDate.parse;
  MockDate.UTC = originalDate.UTC;
  MockDate.isMock = true;
  MockDate.toString = () => "function Date() { [native code] }";
  return MockDate;
}

const mockTimers = new MockTimers();

const mock = {
  fn: (original, implementation, options) => {
    if (original !== null && typeof original === "object") {
      options = original;
      original = undefined;
      implementation = undefined;
    } else if (implementation !== null && typeof implementation === "object") {
      options = implementation;
      implementation = original;
    }

    const ctx = new MockFunctionContext(
      implementation ?? original,
      undefined,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(
      original,
      implementation ?? original,
      ctx,
    );
    return mockFn;
  },

  getter: (object, methodName, implementation, options) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation;
      implementation = undefined;
    }
    return mockMethodImpl(object, methodName, implementation, {
      ...options,
      getter: true,
    });
  },

  method: (object, methodName, implementation, options) => {
    return mockMethodImpl(object, methodName, implementation, options);
  },

  reset: () => {
    ArrayPrototypeForEach(activeMocks, (ctx) => {
      ctx.resetCalls();
    });
  },

  restoreAll: () => {
    while (activeMocks.length > 0) {
      const ctx = activeMocks[activeMocks.length - 1];
      ctx.restore();
    }
  },

  setter: (object, methodName, implementation, options) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation;
      implementation = undefined;
    }
    return mockMethodImpl(object, methodName, implementation, {
      ...options,
      setter: true,
    });
  },

  timers: {
    enable: (options) => mockTimers.enable(options),
    reset: () => mockTimers.reset(),
    tick: (ms) => mockTimers.tick(ms),
    runAll: () => mockTimers.runAll(),
    setTime: (ms) => mockTimers.setTime(ms),
    [SymbolDispose]: () => mockTimers.reset(),
  },
};

test.test = test;
test.mock = mock;
test.before = before;
test.after = after;
test.beforeEach = beforeEach;
test.afterEach = afterEach;
test.run = run;

return {
  run,
  test,
  suite,
  it,
  describe,
  before,
  after,
  beforeEach,
  afterEach,
  mock,
  default: test,
};
})();
