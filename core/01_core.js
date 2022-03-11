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
    ArrayPrototypeMap,
    ErrorCaptureStackTrace,
    ObjectEntries,
    ObjectFreeze,
    ObjectFromEntries,
    ObjectAssign,
  } = window.__bootstrap.primordials;

  // Available on start due to bindings.
  const { opcallSync, opcallAsync } = window.Deno.core;

  let opsCache = {};
  const errorMap = {};
  // Builtin v8 / JS errors
  registerErrorClass("Error", Error);
  registerErrorClass("RangeError", RangeError);
  registerErrorClass("ReferenceError", ReferenceError);
  registerErrorClass("SyntaxError", SyntaxError);
  registerErrorClass("TypeError", TypeError);
  registerErrorClass("URIError", URIError);

  function ops() {
    return opsCache;
  }

  function syncOpsCache() {
    // op id 0 is a special value to retrieve the map of registered ops.
    opsCache = ObjectFreeze(ObjectFromEntries(opcallSync(0)));
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

  function opAsync(opName, arg1 = null, arg2 = null) {
    const promiseOrErr = opcallAsync(opsCache[opName], arg1, arg2);
    // Handle sync error (e.g: error parsing args
    return unwrapOpResult(promiseOrErr).catch(unwrapOpResult);
    // const promise = unwrapOpResult(promiseOrErr);
    // TODO(@AaronO): remove by moving rejection rust-side
    // return PromisePrototypeCatch(promise, unwrapOpResult);
  }

  function opSync(opName, arg1 = null, arg2 = null) {
    return unwrapOpResult(opcallSync(opsCache[opName], arg1, arg2));
  }

  function resources() {
    return ObjectFromEntries(opSync("op_resources"));
  }

  function read(rid, buf) {
    return opAsync("op_read", rid, buf);
  }

  function write(rid, buf) {
    return opAsync("op_write", rid, buf);
  }

  function shutdown(rid) {
    return opAsync("op_shutdown", rid);
  }

  function close(rid) {
    opSync("op_close", rid);
  }

  function tryClose(rid) {
    opSync("op_try_close", rid);
  }

  function print(str, isErr = false) {
    opSync("op_print", str, isErr);
  }

  function metrics() {
    const [aggregate, perOps] = opSync("op_metrics");
    aggregate.ops = ObjectFromEntries(ArrayPrototypeMap(
      ObjectEntries(opsCache),
      ([opName, opId]) => [opName, perOps[opId]],
    ));
    return aggregate;
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
    ops,
    close,
    tryClose,
    read,
    write,
    shutdown,
    print,
    resources,
    metrics,
    registerErrorBuilder,
    registerErrorClass,
    syncOpsCache,
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    // enableOpCallTracing,
    // isOpCallTracingEnabled,
    // opCallTraces,
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });
})(globalThis);
