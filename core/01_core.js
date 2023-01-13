// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
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
    ArrayPrototypePush,
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
  const { ops } = window.Deno.core;

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

  function buildCustomError(className, message, code) {
    let error;
    try {
      error = errorMap[className]?.(message);
    } catch (e) {
      throw new Error(
        `Unsable to build custom error for "${className}"\n  ${e.message}`,
      );
    }
    // Strip buildCustomError() calls from stack trace
    if (typeof error == "object") {
      ErrorCaptureStackTrace(error, buildCustomError);
      if (code) {
        error.code = code;
      }
    }
    return error;
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

  function rollPromiseId() {
    return nextPromiseId++;
  }

  // Generate async op wrappers. See core/bindings.rs
  function initializeAsyncOps() {
    function genAsyncOp(op, name, args) {
      return new Function(
        "setPromise",
        "getPromise",
        "promiseIdSymbol",
        "rollPromiseId",
        "handleOpCallTracing",
        "op",
        "unwrapOpResult",
        "PromisePrototypeThen",
        `
        return function ${name}(${args}) {
          const id = rollPromiseId();
          let promise = PromisePrototypeThen(setPromise(id), unwrapOpResult);
          try {
            op(id, ${args});
          } catch (err) {
            // Cleanup the just-created promise
            getPromise(id);
            // Rethrow the error
            throw err;
          }
          promise = handleOpCallTracing("${name}", id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        }
      `,
      )(
        setPromise,
        getPromise,
        promiseIdSymbol,
        rollPromiseId,
        handleOpCallTracing,
        op,
        unwrapOpResult,
        PromisePrototypeThen,
      );
    }

    // { <name>: <argc>, ... }
    for (const ele of Object.entries(ops.asyncOpsInfo())) {
      if (!ele) continue;
      const [name, argc] = ele;
      const op = ops[name];
      const args = Array.from({ length: argc }, (_, i) => `arg${i}`).join(", ");
      ops[name] = genAsyncOp(op, name, args);
    }
  }

  function handleOpCallTracing(opName, promiseId, p) {
    if (opCallTracingEnabled) {
      const stack = StringPrototypeSlice(new Error().stack, 6);
      MapPrototypeSet(opCallTraces, promiseId, { opName, stack });
      return PromisePrototypeFinally(
        p,
        () => MapPrototypeDelete(opCallTraces, promiseId),
      );
    } else {
      return p;
    }
  }

  function opAsync(opName, ...args) {
    return ops[opName](...args);
  }

  function refOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    ops.op_ref_op(promiseId);
  }

  function unrefOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    ops.op_unref_op(promiseId);
  }

  function resources() {
    return ObjectFromEntries(ops.op_resources());
  }

  function metrics() {
    const [aggregate, perOps] = ops.op_metrics();
    aggregate.ops = ObjectFromEntries(ArrayPrototypeMap(
      ops.op_op_names(),
      (opName, opId) => [opName, perOps[opId]],
    ));
    return aggregate;
  }

  let reportExceptionCallback = undefined;

  // Used to report errors thrown from functions passed to `queueMicrotask()`.
  // The callback will be passed the thrown error. For example, you can use this
  // to dispatch an error event to the global scope.
  // In other words, set the implementation for
  // https://html.spec.whatwg.org/multipage/webappapis.html#report-the-exception
  function setReportExceptionCallback(cb) {
    if (typeof cb != "function") {
      throw new TypeError("expected a function");
    }
    reportExceptionCallback = cb;
  }

  function queueMicrotask(cb) {
    if (typeof cb != "function") {
      throw new TypeError("expected a function");
    }
    return ops.op_queue_microtask(() => {
      try {
        cb();
      } catch (error) {
        if (reportExceptionCallback) {
          reportExceptionCallback(error);
        } else {
          throw error;
        }
      }
    });
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

  const promiseHooks = {
    init: [],
    before: [],
    after: [],
    resolve: [],
    hasBeenSet: false,
  };

  function setPromiseHooks(init, before, after, resolve) {
    if (init) ArrayPrototypePush(promiseHooks.init, init);
    if (before) ArrayPrototypePush(promiseHooks.before, before);
    if (after) ArrayPrototypePush(promiseHooks.after, after);
    if (resolve) ArrayPrototypePush(promiseHooks.resolve, resolve);

    if (!promiseHooks.hasBeenSet) {
      promiseHooks.hasBeenSet = true;

      ops.op_set_promise_hooks((promise, parentPromise) => {
        for (let i = 0; i < promiseHooks.init.length; ++i) {
          promiseHooks.init[i](promise, parentPromise);
        }
      }, (promise) => {
        for (let i = 0; i < promiseHooks.before.length; ++i) {
          promiseHooks.before[i](promise);
        }
      }, (promise) => {
        for (let i = 0; i < promiseHooks.after.length; ++i) {
          promiseHooks.after[i](promise);
        }
      }, (promise) => {
        for (let i = 0; i < promiseHooks.resolve.length; ++i) {
          promiseHooks.resolve[i](promise);
        }
      });
    }
  }

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    opAsync,
    initializeAsyncOps,
    resources,
    metrics,
    registerErrorBuilder,
    registerErrorClass,
    buildCustomError,
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
    setReportExceptionCallback,
    setPromiseHooks,
    close: (rid) => ops.op_close(rid),
    tryClose: (rid) => ops.op_try_close(rid),
    read: (rid, buffer) => ops.op_read(rid, buffer),
    readAll: (rid) => ops.op_read_all(rid),
    write: (rid, buffer) => ops.op_write(rid, buffer),
    writeAll: (rid, buffer) => ops.op_write_all(rid, buffer),
    shutdown: (rid) => ops.op_shutdown(rid),
    print: (msg, isErr) => ops.op_print(msg, isErr),
    setMacrotaskCallback: (fn) => ops.op_set_macrotask_callback(fn),
    setNextTickCallback: (fn) => ops.op_set_next_tick_callback(fn),
    runMicrotasks: () => ops.op_run_microtasks(),
    hasTickScheduled: () => ops.op_has_tick_scheduled(),
    setHasTickScheduled: (bool) => ops.op_set_has_tick_scheduled(bool),
    evalContext: (
      source,
      specifier,
    ) => ops.op_eval_context(source, specifier),
    createHostObject: () => ops.op_create_host_object(),
    encode: (text) => ops.op_encode(text),
    decode: (buffer) => ops.op_decode(buffer),
    serialize: (
      value,
      options,
      errorCallback,
    ) => ops.op_serialize(value, options, errorCallback),
    deserialize: (buffer, options) => ops.op_deserialize(buffer, options),
    getPromiseDetails: (promise) => ops.op_get_promise_details(promise),
    getProxyDetails: (proxy) => ops.op_get_proxy_details(proxy),
    isProxy: (value) => ops.op_is_proxy(value),
    memoryUsage: () => ops.op_memory_usage(),
    setWasmStreamingCallback: (fn) => ops.op_set_wasm_streaming_callback(fn),
    abortWasmStreaming: (
      rid,
      error,
    ) => ops.op_abort_wasm_streaming(rid, error),
    destructureError: (error) => ops.op_destructure_error(error),
    opNames: () => ops.op_op_names(),
    eventLoopHasMoreWork: () => ops.op_event_loop_has_more_work(),
    setPromiseRejectCallback: (fn) => ops.op_set_promise_reject_callback(fn),
    byteLength: (str) => ops.op_str_byte_length(str),
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });

  // Direct bindings on `globalThis`
  ObjectAssign(globalThis, { queueMicrotask });
  setQueueMicrotask(queueMicrotask);
})(globalThis);
