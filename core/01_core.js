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
    ArrayPrototypeMap,
    ErrorCaptureStackTrace,
    ObjectFromEntries,
    MapPrototypeDelete,
    MapPrototypeSet,
    PromisePrototypeFinally,
    PromisePrototypeThen,
    // PromiseResolve,
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

  function registerErrorClass(className, errorClass) {
    registerErrorBuilder(className, (msg) => new errorClass(msg));
  }

  function registerErrorBuilder(className, errorBuilder) {
    if (typeof errorMap[className] !== "undefined") {
      throw new TypeError(`Error class for "${className}" already registered`);
    }
    errorMap[className] = errorBuilder;
  }

  function buildAndThrowCustomError([className, message, code]) {
    const errorBuilder = errorMap[className];
    const error = errorBuilder ? errorBuilder(message) : new Error(
      `Unregistered error class: "${className}"\n  ${message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
    );
    if (code) {
      // Set .code if error was a known OS error, see error_codes.rs
      error.code = code;
    }
    // Strip buildAndThrowCustomError() and errorBuilder() calls from stack trace
    ErrorCaptureStackTrace(error, buildAndThrowCustomError);
    throw error;
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
    const promiseId = ops[opName](...args);
    // Postpone the `op_get_promise` call in the hopes that `Promise.resolve(promiseId).then()`
    // is optimised by V8 to be much faster than a binding-layer call would be.
    // If this theory is true, then it should mean that multiple V8 Fast API
    // async op calls could be called synchronously before a single slow op call needs to be called
    // to create the related promises.
    // let p = PromisePrototypeThen(PromiseResolve(promiseId), ops.op_get_promise);
    let p = PromisePrototypeThen(
      ops.op_get_promise(promiseId),
      undefined,
      buildAndThrowCustomError,
    );
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
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    enableOpCallTracing,
    isOpCallTracingEnabled,
    opCallTraces,
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
