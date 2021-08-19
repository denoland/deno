// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
    Promise,
    ObjectFreeze,
    ObjectFromEntries,
    MapPrototypeGet,
    MapPrototypeDelete,
    MapPrototypeSet,
    PromisePrototypeThen,
    ObjectAssign,
  } = window.__bootstrap.primordials;

  // Available on start due to bindings.
  const { opcall } = window.Deno.core;

  let opsCache = {};
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

  function ops() {
    return opsCache;
  }

  function syncOpsCache() {
    // op id 0 is a special value to retrieve the map of registered ops.
    opsCache = ObjectFreeze(ObjectFromEntries(opcall(0)));
  }

  function opresolve() {
    for (let i = 0; i < arguments.length; i += 2) {
      const promiseId = arguments[i];
      const res = arguments[i + 1];
      const promise = getPromise(promiseId);
      promise.resolve(res);
    }
  }

  function dispatch(opName, promiseId, control, zeroCopy) {
    const opId = typeof opName === "string" ? opsCache[opName] : opName;
    return opcall(opId, promiseId, control, zeroCopy);
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
      if (!errorBuilder) {
        throw new Error(
          `Unregistered error class: "${className}"\n  ${res.message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
        );
      }
      throw errorBuilder(res.message);
    }
    return res;
  }

  function opAsync(opName, arg1 = null, arg2 = null) {
    const promiseId = nextPromiseId++;
    const maybeError = dispatch(opName, promiseId, arg1, arg2);
    // Handle sync error (e.g: error parsing args)
    if (maybeError) return unwrapOpResult(maybeError);
    return PromisePrototypeThen(setPromise(promiseId), unwrapOpResult);
  }

  function opSync(opName, arg1 = null, arg2 = null) {
    return unwrapOpResult(dispatch(opName, null, arg1, arg2));
  }

  function resources() {
    return ObjectFromEntries(opSync("op_resources"));
  }

  function close(rid) {
    opSync("op_close", rid);
  }

  function print(str, isErr = false) {
    opSync("op_print", str, isErr);
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

  class Interrupted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Interrupted";
    }
  }

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    opAsync,
    opSync,
    ops,
    close,
    print,
    resources,
    registerErrorBuilder,
    registerErrorClass,
    opresolve,
    syncOpsCache,
    BadResource,
    Interrupted,
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });
})(globalThis);
