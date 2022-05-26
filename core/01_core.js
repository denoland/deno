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
    PromisePrototypeCatch,
    PromisePrototypeFinally,
    StringPrototypeSlice,
    ObjectAssign,
    SymbolFor,
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
    let [p, promiseId] = ops[opName](...args);
    p = PromisePrototypeCatch(p, unwrapOpResult);
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
      core.opNames(),
      (opName, opId) => [opName, perOps[opId]],
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
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    enableOpCallTracing,
    isOpCallTracingEnabled,
    opCallTraces,
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });
})(globalThis);
