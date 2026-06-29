// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
"use strict";
const { core, primordials } = __bootstrap;
const {
  ArrayIsArray,
  ArrayPrototypeForEach,
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypeJoin,
  ArrayPrototypeLastIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  DatePrototypeGetTime,
  DatePrototypeToString,
  Error,
  ErrorPrototype,
  JSONStringify,
  MapPrototypeClear,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  NumberIsFinite,
  NumberIsInteger,
  ObjectDefineProperty,
  ObjectKeys,
  ObjectPrototypeHasOwnProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetPrototypeOf,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  PromiseWithResolvers,
  Proxy,
  ReflectApply,
  ReflectConstruct,
  ReflectGet,
  RegExpPrototypeExec,
  RegExpPrototypeTest,
  SafeArrayIterator,
  SafeMap,
  SafeMapIterator,
  SafeRegExp,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeMatch,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  Symbol,
  SymbolDispose,
  SymbolFor,
  SymbolToPrimitive,
  TypeError,
  queueMicrotask,
} = primordials;

// The genuine timer functions, captured once at runtime so that an enabled mock
// clock (node:test's mock.timers) cannot stop a test's timeout from firing in
// real time. We cannot read these at module-init time: this polyfill is baked
// into the startup snapshot, where the module body runs before the timer APIs
// exist on `globalThis`. Instead we capture lazily on the first `test()` /
// `suite()` registration (see installErrorHandlers), which always runs after
// the runtime is booted and before any test body can call mock.timers.enable().
let realSetTimeout = null;
let realClearTimeout = null;
function ensureRealTimers() {
  if (realSetTimeout === null) {
    realSetTimeout = globalThis.setTimeout;
    realClearTimeout = globalThis.clearTimeout;
  }
}

let errorHandlersInstalled = false;

let activeNodeTests = 0;

let pendingCallbackReject = null;

// Stack of failure "sinks" for the tests whose bodies are currently executing.
// A test pushes a sink for the duration of its body (across await boundaries)
// and pops it when the body settles. An unhandled rejection or uncaught
// exception that fires while a test body is running is attributed to the
// innermost still-running test, matching Node's behavior of failing the
// currently-active test (see https://github.com/denoland/deno/issues/34818).
const activeTestSinks = [];

function pushTestSink(sink) {
  ArrayPrototypePush(activeTestSinks, sink);
}

function popTestSink(sink) {
  const idx = ArrayPrototypeLastIndexOf(activeTestSinks, sink);
  if (idx !== -1) {
    ArrayPrototypeSplice(activeTestSinks, idx, 1);
  }
}

// The innermost test whose body is still running, or null when no test body is
// currently on the stack (e.g. between registration and execution).
function currentTestSink() {
  for (let i = activeTestSinks.length - 1; i >= 0; i--) {
    const sink = activeTestSinks[i];
    if (!sink.settled) return sink;
  }
  return null;
}

function buildTimeoutError(timeout) {
  // Node fails a timed-out test with a `test timed out after Nms` cause and an
  // ERR_TEST_FAILURE wrapper. We surface the same message and tag the error so
  // reporters that look at code/failureType behave like Node.
  const err = new Error(`test timed out after ${timeout}ms`);
  err.code = "ERR_TEST_FAILURE";
  err.failureType = "testTimeoutFailure";
  return err;
}

function buildAbortError(signal) {
  // When the test is aborted via a caller-supplied signal, Node fails the test
  // with the signal's reason (or a generic abort error when none was given).
  if (signal && signal.reason !== undefined) {
    return signal.reason;
  }
  const err = new Error("The test was aborted");
  err.code = "ABORT_ERR";
  err.failureType = "testAborted";
  return err;
}

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
  // Capture the genuine timers now, before any test body runs and before any
  // mock clock can replace the globals.
  ensureRealTimers();
  if (errorHandlersInstalled) return;
  errorHandlersInstalled = true;

  globalThis.addEventListener("unhandledrejection", (event) => {
    // Attribute the rejection to the currently-running test so it fails, the
    // way Node does. This is gated on an active sink rather than on
    // `activeNodeTests` so it also works in TAP mode, where tests are run by
    // our own runner and the counter is not used.
    const sink = currentTestSink();
    if (sink !== null) {
      event.preventDefault();
      sink.fail(event.reason);
      return;
    }
    // Preserve the prior behavior of swallowing rejections while Deno.test
    // backed node tests are pending but no body is actively running.
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
  });

  globalThis.addEventListener("error", (event) => {
    // Uncaught exceptions are not routed through the test sink: a synchronous
    // throw in a test body is already caught by the body wrapper, and Node does
    // not treat every uncaught `error` event (for example warnings surfaced as
    // errors) as a test failure. We keep the original behavior of swallowing
    // the error while node tests are pending and forwarding to a pending
    // callback-style `done` reject.
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}
const {
  validateBoolean,
  validateFunction,
  validateInteger,
  validateNumber,
  validateObject,
  validateString,
  validateStringArray,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const nodeErrors = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} = nodeErrors.codes;
// `ERR_INVALID_STATE` is a hand-written class exported at the top level of the
// errors module; unlike the generated codes it is not registered on `.codes`.
const { ERR_INVALID_STATE } = nodeErrors;
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
    assertObject.fileSnapshot = fileSnapshot;
  }
  return assertObject;
}

// Lazy access so `node:fs` and `node:path` polyfills are pulled in only on the
// first `fileSnapshot()` call, mirroring the other lazy loaders in this file
// and avoiding circular init during snapshot build.
let _fsForSnapshot = null;
function getFsForSnapshot() {
  if (_fsForSnapshot === null) {
    _fsForSnapshot = core.loadExtScript("ext:deno_node/fs.ts");
  }
  return _fsForSnapshot;
}
let _pathForSnapshot = null;
function getPathForSnapshot() {
  if (_pathForSnapshot === null) {
    _pathForSnapshot = core.loadExtScript("ext:deno_node/path/mod.ts");
  }
  return _pathForSnapshot;
}

// Resolve update-snapshot mode lazily so the env / Rust op is read at most
// once. We accept either Deno's own `--update-snapshots` flag (when running
// under `deno test`) or Node's `--test-update-snapshots` propagated via
// NODE_OPTIONS, so the polyfill behaves the same way the rest of the Node
// compat surface does for reporter detection above.
//
// The deno_test extension's ops are registered at runtime - after this
// polyfill's snapshot is built - so they are not on the captured `core.ops`.
// They are however reachable via `Deno[Deno.internal].core.ops`, which the
// test runner exposes for cli/js code; we look them up lazily through there.
let _fileSnapshotUpdateMode = undefined;
function isFileSnapshotUpdateMode() {
  if (_fileSnapshotUpdateMode !== undefined) return _fileSnapshotUpdateMode;
  const denoInternal = globalThis.Deno?.[globalThis.Deno.internal];
  const op = denoInternal?.core?.ops?.op_test_snapshot_in_update_mode;
  if (typeof op === "function") {
    try {
      if (op()) {
        _fileSnapshotUpdateMode = true;
        return true;
      }
    } catch { /* op not wired up; not running under `deno test` */ }
  }
  let nodeOptions = "";
  try {
    nodeOptions = globalThis.Deno?.env?.get("NODE_OPTIONS") || "";
  } catch { /* permission denied */ }
  if (
    nodeOptions &&
    RegExpPrototypeTest(
      new SafeRegExp(/(?:^|\s)--test-update-snapshots(?:\s|=|$)/),
      nodeOptions,
    )
  ) {
    _fileSnapshotUpdateMode = true;
    return true;
  }
  _fileSnapshotUpdateMode = false;
  return false;
}

// Default serializer pipeline: matches Node's `t.assert.fileSnapshot` default
// (`JSON.stringify(value, null, 2)`).
function defaultFileSnapshotSerializer(value) {
  return JSONStringify(value, null, 2);
}

// `t.assert.fileSnapshot(value, path[, options])`.
//
// Without `--test-update-snapshots`, serializes `value` and compares against
// the contents of the file at `path` using `assert.strictEqual`. With the
// flag, writes the serialized value to `path` (creating parent directories
// as needed). Both file paths are CWD-relative, matching Node.
function fileSnapshot(actual, path, options) {
  validateString(path, "path");
  if (options === undefined) {
    options = { __proto__: null };
  } else {
    validateObject(options, "options");
  }
  let serializers;
  if (options.serializers === undefined) {
    serializers = [defaultFileSnapshotSerializer];
  } else {
    if (!ArrayIsArray(options.serializers)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.serializers",
        "Array",
        options.serializers,
      );
    }
    serializers = options.serializers;
    for (let i = 0; i < serializers.length; i++) {
      if (typeof serializers[i] !== "function") {
        throw new ERR_INVALID_ARG_TYPE(
          `options.serializers[${i}]`,
          "function",
          serializers[i],
        );
      }
    }
  }
  let value = actual;
  try {
    for (let i = 0; i < serializers.length; i++) {
      value = serializers[i](value);
    }
  } catch (err) {
    const e = new ERR_INVALID_STATE(
      "The provided serializers did not generate a string.",
    );
    e.cause = err;
    e.input = actual;
    throw e;
  }
  if (typeof value !== "string") {
    const e = new ERR_INVALID_STATE(
      "The provided serializers did not generate a string.",
    );
    e.input = actual;
    throw e;
  }

  const fs = getFsForSnapshot();
  if (isFileSnapshotUpdateMode()) {
    try {
      const parent = getPathForSnapshot().dirname(path);
      fs.mkdirSync(parent, { __proto__: null, recursive: true });
      fs.writeFileSync(path, value, "utf8");
    } catch (err) {
      const e = new ERR_INVALID_STATE(
        `Cannot write snapshot file '${path}'.`,
      );
      e.cause = err;
      e.filename = path;
      throw e;
    }
    return;
  }

  let expected;
  try {
    expected = fs.readFileSync(path, "utf8");
  } catch (err) {
    const isMissing = err && err.code === "ENOENT";
    const message = isMissing
      ? `Cannot read snapshot file '${path}'. Missing snapshots can be ` +
        "generated by rerunning the command with the --test-update-snapshots " +
        "flag."
      : `Cannot read snapshot file '${path}'.`;
    const e = new ERR_INVALID_STATE(message);
    e.cause = err;
    e.filename = path;
    throw e;
  }
  assert.strictEqual(value, expected);
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
    // deno-lint-ignore prefer-primordials -- stream is a Node Readable, not an Array
    stream.push(null);
  }

  function emit(type) {
    if (finished) return;
    const data = { __proto__: null };
    // Node's TestsStream emits each lifecycle entry both as a data chunk
    // (consumed via async iteration / `'data'` listeners) and as a named
    // event so callers can attach `.on('test:watch:drained', ...)` directly.
    // deno-lint-ignore prefer-primordials -- stream is a Node Readable, not an Array
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
  const match = StringPrototypeMatch(
    nodeOptions,
    new SafeRegExp(/--test-reporter(?:=|\s+)(\S+)/),
  );
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
  const re = new SafeRegExp(`${flag}(?:=|\\s+)(\\S+)`, "g");
  let m;
  while ((m = RegExpPrototypeExec(re, nodeOptions)) !== null) {
    const value = m[1];
    let pattern;
    const litMatch = StringPrototypeMatch(
      value,
      new SafeRegExp(/^\/(.*)\/([a-z]*)$/),
    );
    if (litMatch) {
      try {
        pattern = new SafeRegExp(litMatch[1], litMatch[2]);
      } catch {
        continue;
      }
    } else {
      try {
        pattern = new SafeRegExp(value);
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
  testOnlyFlagCache = RegExpPrototypeTest(
    new SafeRegExp(/(^|\s)--test-only(\s|=|$)/),
    nodeOptions,
  );
  return testOnlyFlagCache;
}

const TEST_ONLY_WARNING =
  "# 'only' and 'runOnly' require the --test-only command-line option.";

function matchesAnyPattern(name, patterns) {
  for (const p of new SafeArrayIterator(patterns)) {
    if (RegExpPrototypeTest(p, name)) return true;
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
  #abortController = new AbortController();
  // Per-context "warning printed" flag for the `--test-only` diagnostic.
  // Mutated by `runTapEntry` when a child uses `only: true`.
  onlyWarningEmitted = false;
  // When set via `runOnly(true)`, only direct subtests registered with the
  // `only: true` option are executed; all other subtests are filtered out.
  #runOnly = false;

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
    return this.#abortController.signal;
  }

  // Aborts this test's own AbortSignal (t.signal); see NodeTestContext._abort.
  _abort(reason) {
    if (!this.#abortController.signal.aborted) {
      this.#abortController.abort(reason);
    }
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
  runOnly(value) {
    this.#runOnly = !!value;
    return null;
  }

  test(name, options, fn) {
    const prepared = prepareOptions(name, options, fn, {});
    // In run-only mode, subtests not flagged with `only: true` are filtered
    // out entirely: they are neither registered nor reported, matching Node.
    if (this.#runOnly && !prepared.options.only) {
      return PromiseResolve();
    }
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
        } catch {
          /* swallow to keep parity with Node's lenient hook errors */
        }
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
      await runWithTestGuards(async () => {
        const ret = ReflectApply(entry.fn, ctx, [ctx]);
        if (isThenable(ret)) await ret;
      }, {
        timeout: entry.options.timeout,
        signal: entry.options.signal,
        abort: (reason) => ctx._abort(reason),
      });
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

// Runs `invoke()` (a thunk that executes a single test body) while enforcing
// the `timeout` option, honoring a caller-supplied abort `signal`, and capturing
// any unhandled rejection / uncaught exception that fires during the body. The
// returned promise resolves with the body's value, or rejects with the first
// failure observed: a body error, a timeout, an abort, or a captured rejection.
// `opts.abort(reason)` is invoked on timeout/abort so the test's own
// AbortSignal (`t.signal`) is aborted and user code can react.
async function runWithTestGuards(invoke, opts) {
  ensureRealTimers();
  const timeout = opts?.timeout;
  const signal = opts?.signal;
  const abort = opts?.abort;

  const failure = PromiseWithResolvers();
  const result = PromiseWithResolvers();
  const sink = {
    settled: false,
    fail(reason) {
      if (this.settled) return;
      this.settled = true;
      failure.reject(reason);
    },
  };
  pushTestSink(sink);

  let timeoutId = null;
  let onAbort = null;
  try {
    if (signal !== undefined && signal !== null) {
      if (signal.aborted) {
        if (typeof abort === "function") abort(signal.reason);
        sink.fail(buildAbortError(signal));
      } else {
        onAbort = () => {
          if (typeof abort === "function") abort(signal.reason);
          sink.fail(buildAbortError(signal));
        };
        signal.addEventListener("abort", onAbort, { once: true });
      }
    }

    if (typeof timeout === "number" && timeout > 0) {
      timeoutId = realSetTimeout(() => {
        const err = buildTimeoutError(timeout);
        if (typeof abort === "function") abort(err);
        sink.fail(err);
      }, timeout);
    }

    // Race the body against any externally-signalled failure. We avoid
    // Promise.race so a late body settlement cannot resurface after a failure.
    const body = (async () => {
      const value = await invoke();
      // Stop attributing as soon as the body settles. An unhandled rejection
      // whose `unhandledrejection` event already fired while the body was
      // running (for example one surfacing during an await, as in the
      // denoland/deno#34818 repro) has already failed this test through the
      // sink. This is best-effort and bounded to the body's lifetime: a
      // rejection that only surfaces after the body returns is treated as
      // post-test asynchronous activity and not attributed, matching the
      // limitation Node also has for activity that outlives a test. We avoid
      // draining extra event-loop turns here because doing so perturbs the
      // runner's output ordering and timing.
      sink.settled = true;
      return value;
    })();
    PromisePrototypeThen(
      body,
      (value) => result.resolve(value),
      (err) => result.reject(err),
    );
    PromisePrototypeThen(failure.promise, undefined, (err) => {
      result.reject(err);
    });
    return await result.promise;
  } finally {
    sink.settled = true;
    popTestSink(sink);
    if (timeoutId !== null) {
      realClearTimeout(timeoutId);
    }
    if (onAbort !== null && signal) {
      signal.removeEventListener("abort", onAbort);
    }
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
  const guards = {
    timeout: options.timeout,
    signal: options.signal,
    abort: (reason) => nodeTestContext._abort(reason),
  };
  // Install this context as the current one for the duration of the body so a
  // top-level `test()` called inside it is routed here as a subtest (see the
  // dispatcher in `test()`). `enterWith` is a synchronous setter that binds the
  // context for the current async root and its descendants without adding a
  // stack frame - extra frames can push `ext:cli/40_test.js` out of a failure's
  // captured stack and break the source location reported for failed tests. No
  // save/restore is needed: each body runs in its own async root, so concurrent
  // subtest bodies each keep their own context even when they interleave across
  // `await`.
  getTestContextALS().enterWith(nodeTestContext);
  if (
    !options.expectFailure ||
    options.skip ||
    options.todo
  ) {
    try {
      const result = await runWithTestGuards(
        () => runNodeTestFunction(fn, nodeTestContext),
        guards,
      );
      nodeTestContext._checkPlan();
      return result;
    } finally {
      // Drain even on failure so subtests registered before the error finish
      // their Deno test steps before this test settles.
      await nodeTestContext._drainSubtests();
    }
  }

  let failed = false;
  try {
    await runWithTestGuards(
      () => runNodeTestFunction(fn, nodeTestContext),
      guards,
    );
    nodeTestContext._checkPlan();
  } catch (err) {
    failed = true;
    assertExpectedFailure(err, options.expectFailure);
  } finally {
    await nodeTestContext._drainSubtests();
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
  // Guards so the once-per-test `before()`/`after()` hooks run a single time
  // regardless of how many subtests this context has.
  #beforeHooksRun = false;
  #afterHooksRun = false;
  // When set via `runOnly(true)`, only direct subtests registered with the
  // `only: true` option are executed; all other subtests are skipped.
  #runOnly = false;
  // Promises for top-level `test()` calls routed here from inside this
  // context's body (see `_trackSubtest`). The body awaits these before settling
  // so such subtests still complete (Node semantics) and their Deno test steps
  // finish before the parent step returns.
  #subtestPromises = [];

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
        wrapped.fileSnapshot = function (...args) {
          plan.increment();
          return ReflectApply(base.fileSnapshot, this, args);
        };
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

  // Aborts this test's own AbortSignal (t.signal). Called by the test guards on
  // timeout or when a caller-supplied signal aborts, so user code awaiting
  // t.signal observes the abort.
  _abort(reason) {
    if (!this.#abortController.signal.aborted) {
      this.#abortController.abort(reason);
    }
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

  runOnly(value) {
    this.#runOnly = !!value;
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

  test(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    if (this.#plan) this.#plan.increment();
    // deno-lint-ignore no-this-alias
    const parentContext = this;
    const stepPromise = PromisePrototypeThen(
      this.#denoContext.step({
        name: prepared.name,
        fn: async (denoTestContext) => {
          const newNodeTextContext = new NodeTestContext(
            denoTestContext,
            parentContext,
            prepared.name,
          );
          let bodyOk = false;
          try {
            // The parent's `before()` hooks run once, before its first subtest.
            await parentContext._runBeforeHooksOnce();
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
            bodyOk = true;
            // This subtest's own `after()` hooks run once, after its body and
            // all of its own subtests have completed. The parent's `after()`
            // hooks run after the parent finishes, not here.
            await newNodeTextContext._runAfterHooksOnce();
          } catch (err) {
            if (!bodyOk) {
              try {
                await newNodeTextContext._runAfterHooksOnce();
              } catch { /* ignore, test is already failing */ }
            }
            if (!newNodeTextContext[skippedSymbol]) {
              throw err;
            }
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
        ignore: !!prepared.options.todo || !!prepared.options.skip ||
          (this.#runOnly && !prepared.options.only),
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      }),
      () => undefined,
      // A failed step settles `false` rather than rejecting, but guard against
      // rejection so the returned promise never surfaces as unhandled.
      () => undefined,
    );
    return stepPromise;
  }

  // Records a subtest promise so the body runner can await it before this test
  // settles. Used only for top-level `test()` calls routed here from inside the
  // body (see the dispatcher in `test()`); a user awaiting `t.test()` directly
  // sequences it themselves, and an unawaited `t.test()` keeps its existing
  // (Node-divergent) "INCOMPLETE" behavior so unref'd-timer subtests are not
  // forced to resolve.
  _trackSubtest(promise) {
    ArrayPrototypePush(this.#subtestPromises, promise);
  }

  // Awaits subtests registered via a routed top-level `test()` call. Called by
  // the body runner after the test function returns so that such subtests,
  // which are commonly not awaited (e.g. fastify's suites), still run to
  // completion before the parent test settles.
  async _drainSubtests() {
    if (this.#subtestPromises.length === 0) return;
    const promises = ArrayPrototypeSplice(
      this.#subtestPromises,
      0,
      this.#subtestPromises.length,
    );
    for (const p of new SafeArrayIterator(promises)) {
      try {
        await p;
      } catch { /* failures already reported through the subtest's own step */ }
    }
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

  // Runs this context's `before()` hooks a single time, before its first
  // subtest. Idempotent so it is safe to call from every subtest's step.
  async _runBeforeHooksOnce() {
    if (this.#beforeHooksRun) return;
    this.#beforeHooksRun = true;
    for (const hook of new SafeArrayIterator(this.#beforeHooks)) {
      await hook();
    }
  }

  // Runs this context's `after()` hooks a single time, after its body and all
  // of its subtests have completed. Idempotent so success and failure paths can
  // both invoke it without double-running.
  async _runAfterHooksOnce() {
    if (this.#afterHooksRun) return;
    this.#afterHooksRun = true;
    for (const hook of new SafeArrayIterator(this.#afterHooks)) {
      await hook();
    }
  }
}

let currentSuite = null;

// AsyncLocalStorage holding the NodeTestContext whose body is currently
// executing, or null when no test body is on the stack. Set by
// `runPossiblyExpectingFailure` for the duration of each body so that a
// top-level `test()` / `it()` call made from inside another test's body is
// registered as a subtest of that test (via `t.step()`), matching Node, rather
// than hitting Deno's "Nested Deno.test() calls are not supported" error.
//
// This must be an ALS, not a single module global with synchronous
// save/restore: the unawaited routed-subtest path this enables makes execution
// non-LIFO. With concurrent subtest bodies suspended across `await`, a plain
// variable can point at a sibling when a parent resumes and silently mis-nest
// the next routed `test()`. Each Deno test/step fn is its own async root, so
// the store stays scoped to that body's subtree and survives `await`
// boundaries, giving every interleaved body its own context. See
// https://github.com/denoland/deno/issues/35391.
let testContextALS = null;
function getTestContextALS() {
  if (testContextALS !== null) return testContextALS;
  const mod = core.loadExtScript("ext:deno_node/async_hooks.ts");
  const ALS = mod.AsyncLocalStorage;
  testContextALS = new ALS();
  return testContextALS;
}
function getCurrentTestContext() {
  if (testContextALS === null) return null;
  return testContextALS.getStore() ?? null;
}

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
  const hooks = ArrayPrototypeSplice(
    rootAfterHooks,
    0,
    rootAfterHooks.length,
  );
  for (const hook of new SafeArrayIterator(hooks)) {
    try {
      await hook(rootCtx);
    } catch { /* ignore */ }
  }
}

class TestSuite {
  #denoTestContext;
  nodeTestContext;
  // The enclosing suite, or null for a top-level suite. Used to cascade
  // beforeEach()/afterEach() hooks from every ancestor suite onto each test
  // (issue #35404).
  parent = null;
  entries = [];
  beforeAllHooks = [];
  afterAllHooks = [];
  beforeEachHooks = [];
  afterEachHooks = [];

  constructor(t, nodeTestContext, parent) {
    this.#denoTestContext = t;
    this.nodeTestContext = nodeTestContext;
    this.parent = parent ?? null;
  }

  addTest(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const suiteNodeContext = this.nodeTestContext;
    // deno-lint-ignore no-this-alias
    const suite = this;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(
          denoTestContext,
          suiteNodeContext,
          prepared.name,
        );
        // beforeEach()/afterEach() cascade through the whole ancestor chain:
        // each test runs every enclosing suite's hooks, plus the file-scope
        // (root) hooks. beforeEach runs outermost-first, afterEach runs
        // innermost-first, matching Node (issue #35404). The chain is collected
        // here, at run time, so hooks registered late in a suite body are seen.
        const suiteChain = []; // innermost-first
        for (let s = suite; s !== null; s = s.parent) {
          ArrayPrototypePush(suiteChain, s);
        }
        try {
          for (const hook of new SafeArrayIterator(rootBeforeEachHooks)) {
            await hook(newNodeTextContext);
          }
          for (let i = suiteChain.length - 1; i >= 0; i--) {
            for (
              const hook of new SafeArrayIterator(suiteChain[i].beforeEachHooks)
            ) {
              await hook(newNodeTextContext);
            }
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
          for (let i = 0; i < suiteChain.length; i++) {
            for (
              const hook of new SafeArrayIterator(suiteChain[i].afterEachHooks)
            ) {
              try {
                await hook(newNodeTextContext);
              } catch { /* ignore */ }
            }
          }
          for (const hook of new SafeArrayIterator(rootAfterEachHooks)) {
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
    const { promise, resolve } = PromiseWithResolvers();
    const parentSuiteContext = this.nodeTestContext;
    // deno-lint-ignore no-this-alias
    const parentSuite = this;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: wrapSuiteFn(
        prepared.fn,
        resolve,
        prepared.name,
        parentSuiteContext,
        parentSuite,
      ),
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
      // The test's own `after()` hooks run once, after its body and all of its
      // subtests complete (a top-level test awaits its subtests inline).
      await nodeTestContext._runAfterHooksOnce();
    } catch (err) {
      try {
        await nodeTestContext._runAfterHooksOnce();
      } catch { /* ignore, test is already failing */ }
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

function wrapSuiteFn(fn, resolve, name, parentNodeContext, parentSuite) {
  return async function (t) {
    const isTopLevel = parentNodeContext === undefined;
    if (isTopLevel) await runRootBeforeOnce();
    const suiteNodeContext = new NodeTestContext(t, parentNodeContext, name);
    const prevSuite = currentSuite;
    const suite = currentSuite = new TestSuite(
      t,
      suiteNodeContext,
      parentSuite,
    );
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
  // A top-level `test()` called from inside another test's body becomes a
  // subtest of that test, matching Node. Without this it would fall through to
  // `Deno.test()`, which rejects nested registration.
  const currentContext = getCurrentTestContext();
  if (currentContext !== null) {
    const promise = currentContext.test(name, options, fn, overrides);
    // The body that issued this call may not await it; have the running test
    // wait for it before settling so the subtest still completes (Node).
    currentContext._trackSubtest(promise);
    return promise;
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

class MockPropertyContext {
  #object;
  #propertyName;
  #value;
  #originalValue;
  #descriptor;
  #accesses = [];
  #onceValues = new SafeMap();
  _restored = false;

  constructor(object, propertyName, hasValue, value) {
    this.#object = object;
    this.#propertyName = propertyName;
    this.#descriptor = ObjectGetOwnPropertyDescriptor(object, propertyName);
    if (!this.#descriptor) {
      throw new ERR_INVALID_ARG_VALUE(
        "propertyName",
        propertyName,
        "is not a property of the object",
      );
    }
    this.#originalValue = object[propertyName];
    this.#value = hasValue ? value : this.#originalValue;

    const { configurable, enumerable } = this.#descriptor;
    ObjectDefineProperty(object, propertyName, {
      __proto__: null,
      configurable,
      enumerable,
      get: () => {
        const nextValue = this.#getAccessValue(this.#value);
        ArrayPrototypePush(this.#accesses, {
          type: "get",
          value: nextValue,
          stack: new Error(),
        });
        return nextValue;
      },
      set: (v) => this.mockImplementation(v),
    });
  }

  get accesses() {
    return ArrayPrototypeSlice(this.#accesses, 0);
  }

  accessCount() {
    return this.#accesses.length;
  }

  mockImplementation(value) {
    if (!this.#descriptor.writable) {
      throw new ERR_INVALID_ARG_VALUE(
        "propertyName",
        this.#propertyName,
        "cannot be set",
      );
    }
    const nextValue = this.#getAccessValue(value);
    ArrayPrototypePush(this.#accesses, {
      type: "set",
      value: nextValue,
      stack: new Error(),
    });
    this.#value = nextValue;
  }

  #getAccessValue(value) {
    const accessIndex = this.#accesses.length;
    let accessValue;
    if (MapPrototypeHas(this.#onceValues, accessIndex)) {
      accessValue = MapPrototypeGet(this.#onceValues, accessIndex);
      MapPrototypeDelete(this.#onceValues, accessIndex);
    } else {
      accessValue = value;
    }
    return accessValue;
  }

  mockImplementationOnce(value, onAccess) {
    const nextAccess = this.#accesses.length;
    const accessIndex = onAccess ?? nextAccess;
    validateInteger(accessIndex, "onAccess", nextAccess);
    MapPrototypeSet(this.#onceValues, accessIndex, value);
  }

  resetAccesses() {
    this.#accesses = [];
  }

  // Alias used by mock.reset() which iterates activeMocks calling resetCalls().
  resetCalls() {
    this.resetAccesses();
  }

  restore() {
    if (!this._restored) {
      // Reinstall the pristine original descriptor. Unlike Node we don't force
      // a `value` field, since the original property may be an accessor (e.g.
      // `process.platform` is a getter in Deno) and mixing `value` with
      // `get`/`set` is an invalid descriptor.
      ObjectDefineProperty(this.#object, this.#propertyName, {
        __proto__: null,
        ...this.#descriptor,
      });
      this._restored = true;
    }
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
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
  "AbortSignal.timeout",
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

// Installs/uninstalls this MockTimers on the `node:timers` module so that
// `require("node:timers")` and `import ... from "node:timers[/promises]"`
// callers route through the virtual clock too, not just `globalThis`.
const kInstallMockTimers = SymbolFor("Deno.internal.node.mockTimers");

class MockTimers {
  _enabled = false;
  _now = 0;
  _timers = new SafeMap();
  _nextId = 1;
  #originals = new SafeMap();
  #mockedApis = new SafeMap();
  // `AbortSignal.timeout` is a static method, not a `globalThis` binding, so it
  // is saved/restored separately from `#originals` (which maps global names).
  #abortSignalTimeoutOriginal = null;
  #abortSignalTimeoutMocked = false;

  #mockGlobal(name, value) {
    if (!MapPrototypeHas(this.#originals, name)) {
      MapPrototypeSet(this.#originals, name, globalThis[name]);
    }
    globalThis[name] = value;
  }

  // Whether `api` (e.g. `"setTimeout"`) is currently being intercepted. Read by
  // the `node:timers` module functions to decide per-call whether to use the
  // virtual clock, mirroring the per-api selection of `enable({ apis })`.
  _apiEnabled(api) {
    return MapPrototypeHas(this.#mockedApis, api);
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
      now = DatePrototypeGetTime(now);
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
      MapPrototypeSet(this.#mockedApis, apis[i], true);
    }

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
        this.#mockGlobal(
          "clearTimeout",
          (handle) => this._clearTimer(handle),
        );
      } else if (api === "setInterval") {
        this.#mockGlobal(
          "setInterval",
          (callback, delay, ...args) =>
            this._setInterval(callback, delay, args),
        );
        this.#mockGlobal(
          "clearInterval",
          (handle) => this._clearTimer(handle),
        );
      } else if (api === "setImmediate") {
        this.#mockGlobal(
          "setImmediate",
          (callback, ...args) => this._setTimeout(callback, 0, args, true),
        );
        this.#mockGlobal(
          "clearImmediate",
          (handle) => this._clearTimer(handle),
        );
      } else if (api === "AbortSignal.timeout") {
        this.#abortSignalTimeoutOriginal = globalThis.AbortSignal.timeout;
        this.#abortSignalTimeoutMocked = true;
        globalThis.AbortSignal.timeout = (delay) =>
          this.#mockedAbortSignalTimeout(delay);
      }
    }

    // Route the `node:timers` / `node:timers/promises` module functions through
    // this instance too (see `kInstallMockTimers` in `timers.ts`).
    core.loadExtScript("ext:deno_node/timers.ts")[kInstallMockTimers](this);
  }

  reset() {
    if (!this._enabled) return;
    core.loadExtScript("ext:deno_node/timers.ts")[kInstallMockTimers](null);
    for (
      const { 0: name, 1: original } of new SafeMapIterator(this.#originals)
    ) {
      globalThis[name] = original;
    }
    if (this.#abortSignalTimeoutMocked) {
      globalThis.AbortSignal.timeout = this.#abortSignalTimeoutOriginal;
      this.#abortSignalTimeoutOriginal = null;
      this.#abortSignalTimeoutMocked = false;
    }
    MapPrototypeClear(this.#originals);
    MapPrototypeClear(this.#mockedApis);
    MapPrototypeClear(this._timers);
    this._now = 0;
    this._nextId = 1;
    this._enabled = false;
  }

  #assertEnabled() {
    if (!this._enabled) {
      throw new ERR_INVALID_STATE(
        "You should enable MockTimers first by calling the .enable function",
      );
    }
  }

  #assertTimeArg(time) {
    if (time < 0) {
      throw new ERR_INVALID_ARG_VALUE(
        "time",
        time,
        "must be a non-negative integer",
      );
    }
  }

  // Mirrors Node's `MockTimers.tick`: advance the clock to each due timer's
  // scheduled time as it fires (not straight to the end of the window), so a
  // callback reading `Date.now()` sees the time the timer was scheduled for.
  // Intervals re-arm and may fire again within the same tick.
  tick(milliseconds = 1) {
    this.#assertEnabled();
    this.#assertTimeArg(milliseconds);
    const target = this._now + milliseconds;
    while (true) {
      const next = this.#findNextTimer();
      if (next === null || next.fireAt > target) break;
      this._now = next.fireAt;
      this.#fireTimer(next);
    }
    this._now = target;
  }

  // Mirrors Node's `MockTimers.runAll`: tick up to the longest pending timer.
  // Intervals fire as many times as fit within that window, then re-arm past
  // `_now` so the loop terminates.
  runAll() {
    this.#assertEnabled();
    const longest = this.#findLongestTimer();
    if (longest === null) return;
    this.tick(longest.fireAt - this._now);
  }

  setTime(milliseconds) {
    validateNumber(milliseconds, "time");
    this.#assertTimeArg(milliseconds);
    this.#assertEnabled();
    this._now = milliseconds;
  }

  [SymbolDispose]() {
    this.reset();
  }

  // Mirrors Node's mocked `AbortSignal.timeout`: the returned signal aborts
  // with a `TimeoutError` once the virtual clock advances past `delay`, instead
  // of using real time, so `tick()` / `runAll()` drive the timeout.
  #mockedAbortSignalTimeout(delay) {
    const controller = new AbortController();
    this._setTimeout(
      () => {
        controller.abort(
          new DOMException("The operation timed out.", "TimeoutError"),
        );
      },
      delay,
      [],
      false,
    );
    return controller.signal;
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
    for (const { 1: t } of new SafeMapIterator(this._timers)) {
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

  #findLongestTimer() {
    let longest = null;
    for (const { 1: t } of new SafeMapIterator(this._timers)) {
      if (longest === null || t.fireAt > longest.fireAt) {
        longest = t;
      }
    }
    return longest;
  }

  #fireTimer(timer) {
    // Match Node: invoke the callback first (errors propagate synchronously out
    // of `tick`), then re-arm intervals or drop one-shot timers. A timer that
    // clears itself inside its own callback is already gone from `_timers`, so
    // the bookkeeping below is a no-op for it.
    ReflectApply(timer.callback, undefined, timer.args);
    if (timer.interval !== null) {
      timer.fireAt += timer.interval;
    } else {
      MapPrototypeDelete(this._timers, timer.id);
    }
  }
}

const originalDate = globalThis.Date;
const originalDatePrototype = originalDate.prototype;

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
    __proto__: null,
    value: originalDate.prototype,
    writable: false,
  });
  ObjectDefineProperty(MockDate, "name", {
    __proto__: null,
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

// `mock.module()` support.
//
// Module mocking is built on top of `module.registerHooks()` (the Node module
// customization hooks). A single registration intercepts both `import()` and
// `require()`. The whole subsystem is lazily initialized on the first
// `mock.module()` call so that importing `node:test` without mocking modules
// pays no startup cost and registers no hooks.

// Symbol under which the live mock registry is published on `globalThis`. The
// synthetic module source generated for a mocked specifier reads the live
// export values back out of this registry at evaluation time, so functions and
// objects passed to `mock.module()` cross into the mocked module unchanged.
const kMockModuleRegistryName = "deno.internal.nodeTestMockModules";
// Search param appended to a mocked ESM url to bust the module cache when
// `cache` is false. ESM modules cannot be evicted from the loader cache, so a
// fresh url (new version) is resolved on every import instead.
const kMockSearchParam = "node-test-mock";
const kBadExportsMessage = "Cannot create mock because named exports cannot " +
  "be applied to the provided default export.";

let mockModuleRegistry = null;
let mockModuleHooksHandle = null;
let nodeModuleNamespace = null;
// Monotonic counter used to version mocked ESM urls. A global (rather than
// per-mock) counter guarantees that a freshly created mock never collides with
// a versioned url that a previous mock already cached.
let mockModuleVersion = 0;

function getNodeModuleNamespace() {
  if (nodeModuleNamespace === null) {
    nodeModuleNamespace = core.createLazyLoader("node:module")();
  }
  return nodeModuleNamespace;
}

function isUrlLike(value) {
  return value !== null && typeof value === "object" &&
    typeof value.href === "string" && typeof value.protocol === "string";
}

function ensureMockModuleHooks() {
  if (mockModuleHooksHandle !== null) {
    return;
  }
  mockModuleRegistry = new SafeMap();
  globalThis[SymbolFor(kMockModuleRegistryName)] = mockModuleRegistry;
  const { registerHooks } = getNodeModuleNamespace();
  mockModuleHooksHandle = registerHooks({
    __proto__: null,
    resolve: mockModuleResolveHook,
    load: mockModuleLoadHook,
  });
}

// Best effort discovery of the file that called `mock.module()`, used to
// resolve relative specifiers the same way Node does (eagerly, against the
// caller). Returns undefined when no user frame can be found, in which case
// the default resolver falls back to the current working directory.
const kStackFrameUrl = new SafeRegExp(
  "((?:file|https?):\\/\\/[^\\s)]+?):\\d+:\\d+",
);
function getCallerUrl() {
  const stack = new Error().stack;
  if (typeof stack !== "string") {
    return undefined;
  }
  const lines = StringPrototypeSplit(stack, "\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (
      StringPrototypeIncludes(line, "ext:deno_node/") ||
      StringPrototypeIncludes(line, "ext:core/")
    ) {
      continue;
    }
    const match = RegExpPrototypeExec(kStackFrameUrl, line);
    if (match !== null) {
      const candidate = match[1];
      if (
        StringPrototypeStartsWith(candidate, "file:") ||
        StringPrototypeStartsWith(candidate, "http")
      ) {
        return candidate;
      }
    }
  }
  return undefined;
}

let nodeUrlNamespace = null;
function getNodeUrl() {
  if (nodeUrlNamespace === null) {
    nodeUrlNamespace = core.createLazyLoader("node:url")();
  }
  return nodeUrlNamespace;
}

// Normalizes a builtin specifier so that both `readline` and `node:readline`
// map to the same registry key.
function normalizeNodeScheme(specifier) {
  if (StringPrototypeStartsWith(specifier, "node:")) {
    return specifier;
  }
  const { isBuiltin } = getNodeModuleNamespace();
  if (isBuiltin(specifier)) {
    return "node:" + specifier;
  }
  return specifier;
}

// Canonicalizes a resolved url into the registry key. Builtins are normalized
// to the `node:` scheme; file urls are resolved through the real path so that
// the key registered by `mock.module()` matches the (realpath based) url the
// require/import hooks observe, regardless of symlinks such as macOS
// `/tmp` to `/private/tmp`.
function canonicalizeUrlKey(u) {
  if (StringPrototypeStartsWith(u, "file:")) {
    try {
      const path = getNodeUrl().fileURLToPath(u);
      const real = core.ops.op_require_real_path(path);
      return getNodeUrl().pathToFileURL(real).href;
    } catch {
      return u;
    }
  }
  return normalizeNodeScheme(u);
}

// Resolves a `mock.module()` specifier to the canonical registry key (the
// resolved module url, with builtins normalized to the `node:` scheme).
function resolveSpecifierToKey(specifier) {
  if (StringPrototypeStartsWith(specifier, "node:")) {
    return specifier;
  }
  const parentUrl = getCallerUrl();
  let resolved;
  try {
    resolved = core.ops.op_module_default_resolve(specifier, parentUrl);
  } catch {
    resolved = specifier;
  }
  return canonicalizeUrlKey(resolved);
}

// Strips the cache busting search param so a versioned ESM url maps back to its
// registry key.
function stripMockParam(u) {
  const idx = StringPrototypeIndexOf(u, kMockSearchParam);
  if (idx === -1) {
    return u;
  }
  try {
    const parsed = new URL(u);
    parsed.searchParams.delete(kMockSearchParam);
    let href = parsed.href;
    if (StringPrototypeEndsWith(href, "?")) {
      href = StringPrototypeSlice(href, 0, href.length - 1);
    }
    return href;
  } catch {
    return u;
  }
}

function appendMockParam(u, version) {
  const sep = StringPrototypeIncludes(u, "?") ? "&" : "?";
  return u + sep + kMockSearchParam + "=" + version;
}

// Looks up the active mock entry (if any) for a url seen by a hook.
function lookupMockEntry(u) {
  if (mockModuleRegistry === null) {
    return undefined;
  }
  let entry = MapPrototypeGet(mockModuleRegistry, u);
  if (entry === undefined && StringPrototypeIncludes(u, kMockSearchParam)) {
    entry = MapPrototypeGet(mockModuleRegistry, stripMockParam(u));
  }
  if (entry === undefined) {
    return undefined;
  }
  return entry.active ? entry : undefined;
}

function detectFormat(key) {
  if (StringPrototypeStartsWith(key, "node:")) {
    return "commonjs";
  }
  if (
    StringPrototypeEndsWith(key, ".mjs") || StringPrototypeEndsWith(key, ".mts")
  ) {
    return "module";
  }
  if (
    StringPrototypeEndsWith(key, ".cjs") || StringPrototypeEndsWith(key, ".cts")
  ) {
    return "commonjs";
  }
  if (StringPrototypeStartsWith(key, "file:")) {
    try {
      const path = getNodeUrl().fileURLToPath(key);
      return core.ops.op_require_is_maybe_cjs(path) ? "commonjs" : "module";
    } catch {
      return "commonjs";
    }
  }
  // Remote modules (http/https) are only loadable as ESM.
  if (
    StringPrototypeStartsWith(key, "http:") ||
    StringPrototypeStartsWith(key, "https:")
  ) {
    return "module";
  }
  return "commonjs";
}

function registryAccessSource() {
  return "globalThis[Symbol.for(" + JSONStringify(kMockModuleRegistryName) +
    ")]";
}

function generateCjsSource(entry, key) {
  let src = '"use strict";\n';
  src += "const $e = " + registryAccessSource() + ".get(" +
    JSONStringify(key) + ");\n";
  src += "if ($e === undefined) { throw new Error(" +
    JSONStringify('mock exports not found for "' + key + '"') + "); }\n";
  if (entry.hasDefaultExport) {
    src += "module.exports = $e.moduleExports.default;\n";
  }
  if (entry.exportNames.length > 0) {
    src += "if (module.exports === null || typeof module.exports !== " +
      '"object") { throw new Error(' + JSONStringify(kBadExportsMessage) +
      "); }\n";
    for (let i = 0; i < entry.exportNames.length; i++) {
      const name = entry.exportNames[i];
      src += "module.exports[" + JSONStringify(name) + "] = " +
        "$e.moduleExports[" + JSONStringify(name) + "];\n";
    }
  }
  return src;
}

function generateEsmSource(entry, key) {
  let src = "const $e = " + registryAccessSource() + ".get(" +
    JSONStringify(key) + ");\n";
  src += "if ($e === undefined) { throw new Error(" +
    JSONStringify('mock exports not found for "' + key + '"') + "); }\n";
  if (entry.isCjs) {
    // For a CommonJS module the named exports are applied onto the default
    // export object, mirroring how Deno exposes a required CJS module to an
    // ESM importer (default plus the merged keys).
    src += "let $d = $e.hasDefaultExport ? $e.moduleExports.default : {};\n";
    if (entry.exportNames.length > 0) {
      src += 'if ($d === null || typeof $d !== "object") { throw new Error(' +
        JSONStringify(kBadExportsMessage) + "); }\n";
      for (let i = 0; i < entry.exportNames.length; i++) {
        const name = entry.exportNames[i];
        src += "$d[" + JSONStringify(name) + "] = $e.moduleExports[" +
          JSONStringify(name) + "];\n";
      }
    }
    src += "export default $d;\n";
    for (let i = 0; i < entry.exportNames.length; i++) {
      const name = entry.exportNames[i];
      // Use a safe local identifier plus a string export name so export names
      // that are not valid identifiers (e.g. "foo-bar") still generate valid
      // source.
      src += "let $n" + i + " = $d[" + JSONStringify(name) + "];\n";
      src += "export { $n" + i + " as " + JSONStringify(name) + " };\n";
    }
  } else {
    // For an ESM module the default export and named exports are independent.
    if (entry.hasDefaultExport) {
      src += "export default $e.moduleExports.default;\n";
    }
    for (let i = 0; i < entry.exportNames.length; i++) {
      const name = entry.exportNames[i];
      src += "let $n" + i + " = $e.moduleExports[" + JSONStringify(name) +
        "];\n";
      src += "export { $n" + i + " as " + JSONStringify(name) + " };\n";
    }
  }
  return src;
}

// The require cache (Module._cache) key for a mocked module: builtins are keyed
// by their `node:` url, file modules by their (real) path.
function cjsCacheKeyFor(key) {
  if (StringPrototypeStartsWith(key, "file:")) {
    try {
      return getNodeUrl().fileURLToPath(key);
    } catch {
      return key;
    }
  }
  return key;
}

// Removes a mocked CommonJS module from the require cache so the next
// `require()` re-runs the load hook and produces a fresh exports object.
function evictCjsCache(key) {
  const { default: Module } = getNodeModuleNamespace();
  delete Module._cache[cjsCacheKeyFor(key)];
}

function mockModuleResolveHook(specifier, context, nextResolve) {
  const result = nextResolve(specifier, context);
  if (result === null || result === undefined || result.url == null) {
    return result;
  }
  const conditions = context?.conditions;
  // Only the ESM import path needs per-import versioned urls. The require path
  // busts its cache by evicting the require cache entry in the load hook.
  if (
    conditions === undefined ||
    !ArrayPrototypeIncludes(conditions, "import")
  ) {
    return result;
  }
  const entry = lookupMockEntry(canonicalizeUrlKey(result.url));
  if (entry === undefined) {
    return result;
  }
  // A stable version (cache: true) reuses the cached module; otherwise bump the
  // global counter so every import resolves to a brand new url.
  const version = entry.cache ? entry.stableVersion : ++mockModuleVersion;
  return {
    __proto__: null,
    url: appendMockParam(result.url, version),
    shortCircuit: true,
  };
}

function mockModuleLoadHook(url, context, nextLoad) {
  const key = canonicalizeUrlKey(stripMockParam(url));
  const entry = lookupMockEntry(key);
  if (entry === undefined) {
    return nextLoad(url, context);
  }
  const conditions = context?.conditions;
  const viaRequire = conditions !== undefined &&
    ArrayPrototypeIncludes(conditions, "require");
  if (viaRequire) {
    if (!entry.cache) {
      evictCjsCache(key);
    }
    return {
      __proto__: null,
      source: generateCjsSource(entry, key),
      format: "commonjs",
      shortCircuit: true,
    };
  }
  return {
    __proto__: null,
    source: generateEsmSource(entry, key),
    format: "module",
    shortCircuit: true,
  };
}

class MockModuleContext {
  #restore;

  constructor(restore) {
    this.#restore = restore;
  }

  restore() {
    if (this.#restore !== undefined) {
      this.#restore();
      this.#restore = undefined;
    }
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

  // `mock.reset()` iterates `activeMocks` calling `resetCalls()`. Node's
  // `MockTracker.reset()` restores module mocks, so alias it to the restore
  // behavior. The splice from `activeMocks` is deferred to `restore()` to
  // avoid mutating the array mid iteration.
  resetCalls() {
    if (this.#restore !== undefined) {
      this.#restore();
      this.#restore = undefined;
    }
  }
}

function mockModule(specifier, options = { __proto__: null }) {
  let specStr;
  if (typeof specifier === "string") {
    specStr = specifier;
  } else if (isUrlLike(specifier)) {
    specStr = `${specifier}`;
  } else {
    throw new ERR_INVALID_ARG_TYPE(
      "specifier",
      ["string", "URL"],
      specifier,
    );
  }

  validateObject(options, "options");
  const cache = options.cache === undefined ? false : options.cache;
  validateBoolean(cache, "options.cache");

  // `exports` is a legacy alias that bundles `defaultExport` and
  // `namedExports` into a single object: its `default` property (if any)
  // becomes the default export and its remaining own enumerable keys become
  // named exports. It cannot be combined with either of the newer options.
  const exportsOption = options.exports;
  const namedExports = options.namedExports;
  let exportSource = namedExports;
  let hasDefaultExport = ObjectPrototypeHasOwnProperty(
    options,
    "defaultExport",
  );
  let defaultExportValue = options.defaultExport;
  if (exportsOption !== undefined) {
    if (namedExports !== undefined || options.defaultExport !== undefined) {
      throw new ERR_INVALID_ARG_VALUE(
        "options.exports",
        exportsOption,
        'cannot be used with "namedExports" or "defaultExport"',
      );
    }
    validateObject(exportsOption, "options.exports");
    exportSource = exportsOption;
    if (ObjectPrototypeHasOwnProperty(exportsOption, "default")) {
      hasDefaultExport = true;
      defaultExportValue = exportsOption.default;
    }
  } else if (namedExports !== undefined) {
    validateObject(namedExports, "options.namedExports");
  }

  ensureMockModuleHooks();

  const moduleExports = { __proto__: null };
  const exportNames = [];
  if (exportSource !== undefined && exportSource !== null) {
    const keys = ObjectKeys(exportSource);
    for (let i = 0; i < keys.length; i++) {
      const name = keys[i];
      // For the `exports` form the `default` key is the default export, not a
      // named export.
      if (exportsOption !== undefined && name === "default") {
        continue;
      }
      moduleExports[name] = exportSource[name];
      ArrayPrototypePush(exportNames, name);
    }
  }
  if (hasDefaultExport) {
    moduleExports.default = defaultExportValue;
  }

  const key = resolveSpecifierToKey(specStr);
  const existing = MapPrototypeGet(mockModuleRegistry, key);
  if (existing !== undefined && existing.active) {
    throw new ERR_INVALID_STATE(
      `Cannot mock '${specStr}'. The module is already mocked.`,
    );
  }

  const format = detectFormat(key);
  const entry = {
    __proto__: null,
    url: key,
    isCjs: format === "commonjs",
    cache,
    stableVersion: cache ? ++mockModuleVersion : 0,
    active: true,
    moduleExports,
    exportNames,
    hasDefaultExport,
  };
  MapPrototypeSet(mockModuleRegistry, key, entry);
  // Save and evict any previously loaded copy of this module so the next
  // require/import re-runs the hooks and observes the mock. The saved entry is
  // put back on restore so the exact original instance is returned again.
  const { default: Module } = getNodeModuleNamespace();
  const cacheKey = cjsCacheKeyFor(key);
  entry.savedCacheEntry = Module._cache[cacheKey];
  delete Module._cache[cacheKey];

  const restore = () => {
    if (!entry.active) {
      return;
    }
    entry.active = false;
    if (MapPrototypeGet(mockModuleRegistry, key) === entry) {
      MapPrototypeDelete(mockModuleRegistry, key);
    }
    delete Module._cache[cacheKey];
    if (entry.savedCacheEntry !== undefined) {
      Module._cache[cacheKey] = entry.savedCacheEntry;
    }
  };

  const ctx = new MockModuleContext(restore);
  ArrayPrototypePush(activeMocks, ctx);
  return ctx;
}

const mock = {
  fn: (original, implementation, options) => {
    if (original !== null && typeof original === "object") {
      options = original;
      original = undefined;
      implementation = undefined;
    } else if (
      implementation !== null && typeof implementation === "object"
    ) {
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

  module: mockModule,

  property: function (object, propertyName, value) {
    validateObject(object, "object");
    if (
      typeof propertyName !== "string" && typeof propertyName !== "symbol"
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "propertyName",
        ["string", "symbol"],
        propertyName,
      );
    }

    const hasValue = arguments.length > 2;
    const ctx = new MockPropertyContext(
      object,
      propertyName,
      hasValue,
      value,
    );
    ArrayPrototypePush(activeMocks, ctx);

    return new Proxy(object, {
      __proto__: null,
      get(target, property, receiver) {
        if (property === "mock") {
          return ctx;
        }
        return ReflectGet(target, property, receiver);
      },
    });
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
    // `setTime` is MockTimers' own method, not Date.prototype.setTime.
    // deno-lint-ignore prefer-primordials
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
