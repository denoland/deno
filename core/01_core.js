// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Error,
    RangeError,
    ReferenceError,
    SyntaxError,
    TypeError,
    URIError,
    Map,
    Array,
    ArrayPrototypeFill,
    ArrayPrototypeMap,
    ErrorCaptureStackTrace,
    Promise,
    ObjectFromEntries,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeDelete,
    MapPrototypeSet,
    PromisePrototypeThen,
    PromisePrototypeFinally,
    StringPrototypeSlice,
    ObjectAssign,
    SymbolFor,
    setQueueMicrotask,
  } = window.__bootstrap.primordials;
  const ops = window.Deno.core.ops;

  const errorMap = {};
  // Builtin v8 / JS errors
  registerErrorClass("Error", Error);
  registerErrorClass("RangeError", RangeError);
  registerErrorClass("ReferenceError", ReferenceError);
  registerErrorClass("SyntaxError", SyntaxError);
  registerErrorClass("TypeError", TypeError);
  registerErrorClass("URIError", URIError);

  let nextPromiseId = 1;
  const promiseMap = new Map();
  const RING_SIZE = 4 * 1024;
  const NO_PROMISE = null; // Alias to null is faster than plain nulls
  const promiseRing = ArrayPrototypeFill(new Array(RING_SIZE), NO_PROMISE);
  // TODO(bartlomieju): it future use `v8::Private` so it's not visible
  // to users. Currently missing bindings.
  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  let opCallTracingEnabled = false;
  const opCallTraces = new Map();

  function enableOpCallTracing() {
    opCallTracingEnabled = true;
  }

  function isOpCallTracingEnabled() {
    return opCallTracingEnabled;
  }

  function setPromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Move old promise from ring to map
    const oldPromise = promiseRing[idx];
    if (oldPromise !== NO_PROMISE) {
      const oldPromiseId = promiseId - RING_SIZE;
      MapPrototypeSet(promiseMap, oldPromiseId, oldPromise);
    }
    // Set new promise
    return promiseRing[idx] = newPromise();
  }

  function getPromise(promiseId) {
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      const promise = MapPrototypeGet(promiseMap, promiseId);
      MapPrototypeDelete(promiseMap, promiseId);
      return promise;
    }
    // Otherwise take from ring
    const idx = promiseId % RING_SIZE;
    const promise = promiseRing[idx];
    promiseRing[idx] = NO_PROMISE;
    return promise;
  }

  function newPromise() {
    let resolve, reject;
    const promise = new Promise((resolve_, reject_) => {
      resolve = resolve_;
      reject = reject_;
    });
    promise.resolve = resolve;
    promise.reject = reject;
    return promise;
  }

  function hasPromise(promiseId) {
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      return MapPrototypeHas(promiseMap, promiseId);
    }
    // Otherwise check it in ring
    const idx = promiseId % RING_SIZE;
    return promiseRing[idx] != NO_PROMISE;
  }

  function opresolve() {
    for (let i = 0; i < arguments.length; i += 2) {
      const promiseId = arguments[i];
      const res = arguments[i + 1];
      const promise = getPromise(promiseId);
      promise.resolve(res);
    }
  }

  function registerErrorClass(className, errorClass) {
    registerErrorBuilder(className, (msg) => new errorClass(msg));
  }

  function registerErrorBuilder(className, errorBuilder) {
    if (typeof errorMap[className] !== "undefined") {
      throw new TypeError(`Error class for "${className}" already registered`);
    }
    errorMap[className] = errorBuilder;
  }

  function unwrapOpResult(res) {
    // .$err_class_name is a special key that should only exist on errors
    if (res?.$err_class_name) {
      const className = res.$err_class_name;
      const errorBuilder = errorMap[className];
      const err = errorBuilder ? errorBuilder(res.message) : new Error(
        `Unregistered error class: "${className}"\n  ${res.message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
      );
      // Set .code if error was a known OS error, see error_codes.rs
      if (res.code) {
        err.code = res.code;
      }
      // Strip unwrapOpResult() and errorBuilder() calls from stack trace
      ErrorCaptureStackTrace(err, unwrapOpResult);
      throw err;
    }
    return res;
  }

  function opAsync(opName, ...args) {
    const promiseId = nextPromiseId++;
    const maybeError = ops[opName](promiseId, ...args);
    // Handle sync error (e.g: error parsing args)
    if (maybeError) return unwrapOpResult(maybeError);
    let p = PromisePrototypeThen(setPromise(promiseId), unwrapOpResult);
    if (opCallTracingEnabled) {
      // Capture a stack trace by creating a new `Error` object. We remove the
      // first 6 characters (the `Error\n` prefix) to get just the stack trace.
      const stack = StringPrototypeSlice(new Error().stack, 6);
      MapPrototypeSet(opCallTraces, promiseId, { opName, stack });
      p = PromisePrototypeFinally(
        p,
        () => MapPrototypeDelete(opCallTraces, promiseId),
      );
    }
    // Save the id on the promise so it can later be ref'ed or unref'ed
    p[promiseIdSymbol] = promiseId;
    return p;
  }

  function opSync(opName, ...args) {
    return unwrapOpResult(ops[opName](...args));
  }

  function refOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    opSync("op_ref_op", promiseId);
  }

  function unrefOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    opSync("op_unref_op", promiseId);
  }

  function resources() {
    return ObjectFromEntries(opSync("op_resources"));
  }

  function metrics() {
    const [aggregate, perOps] = opSync("op_metrics");
    aggregate.ops = ObjectFromEntries(ArrayPrototypeMap(
      core.opSync("op_op_names"),
      (opName, opId) => [opName, perOps[opId]],
    ));
    return aggregate;
  }

  function queueMicrotask(...args) {
    return opSync("op_queue_microtask", ...args);
  }

  // Some "extensions" rely on "BadResource" and "Interrupted" errors in the
  // JS code (eg. "deno_net") so they are provided in "Deno.core" but later
  // reexported on "Deno.errors"
  class BadResource extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BadResource";
    }
  }
  const BadResourcePrototype = BadResource.prototype;

  class Interrupted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Interrupted";
    }
  }
  const InterruptedPrototype = Interrupted.prototype;

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    opAsync,
    opSync,
    resources,
    metrics,
    registerErrorBuilder,
    registerErrorClass,
    opresolve,
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    enableOpCallTracing,
    isOpCallTracingEnabled,
    opCallTraces,
    refOp,
    unrefOp,
    close: opSync.bind(null, "op_close"),
    tryClose: opSync.bind(null, "op_try_close"),
    read: opAsync.bind(null, "op_read"),
    write: opAsync.bind(null, "op_write"),
    shutdown: opAsync.bind(null, "op_shutdown"),
    print: opSync.bind(null, "op_print"),
    setMacrotaskCallback: opSync.bind(null, "op_set_macrotask_callback"),
    setNextTickCallback: opSync.bind(null, "op_set_next_tick_callback"),
    runMicrotasks: opSync.bind(null, "op_run_microtasks"),
    hasTickScheduled: opSync.bind(null, "op_has_tick_scheduled"),
    setHasTickScheduled: opSync.bind(null, "op_set_has_tick_scheduled"),
    evalContext: opSync.bind(null, "op_eval_context"),
    createHostObject: opSync.bind(null, "op_create_host_object"),
    encode: opSync.bind(null, "op_encode"),
    decode: opSync.bind(null, "op_decode"),
    serialize: opSync.bind(null, "op_serialize"),
    deserialize: opSync.bind(null, "op_deserialize"),
    getPromiseDetails: opSync.bind(null, "op_get_promise_details"),
    getProxyDetails: opSync.bind(null, "op_get_proxy_details"),
    isProxy: opSync.bind(null, "op_is_proxy"),
    memoryUsage: opSync.bind(null, "op_memory_usage"),
    setWasmStreamingCallback: opSync.bind(
      null,
      "op_set_wasm_streaming_callback",
    ),
    abortWasmStreaming: opSync.bind(null, "op_abort_wasm_streaming"),
    destructureError: opSync.bind(null, "op_destructure_error"),
    terminate: opSync.bind(null, "op_terminate"),
    opNames: opSync.bind(null, "op_op_names"),
    eventLoopHasMoreWork: opSync.bind(null, "op_event_loop_has_more_work"),
    setPromiseRejectCallback: opSync.bind(
      null,
      "op_set_promise_reject_callback",
    ),
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });

  // Direct bindings on `globalThis`
  ObjectAssign(globalThis, { queueMicrotask });
  setQueueMicrotask(queueMicrotask);
})(globalThis);
