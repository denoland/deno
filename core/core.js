// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  // Available on start due to bindings.
  const { opcall } = window.Deno.core;

  let opsCache = {};
  const errorMap = {
    // Builtin v8 / JS errors
    Error,
    RangeError,
    ReferenceError,
    SyntaxError,
    TypeError,
    URIError,
  };
  let nextPromiseId = 1;
  const promiseMap = new Map();
  const RING_SIZE = 4 * 1024;
  const NO_PROMISE = null; // Alias to null is faster than plain nulls
  const promiseRing = new Array(RING_SIZE).fill(NO_PROMISE);

  function setPromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Move old promise from ring to map
    const oldPromise = promiseRing[idx];
    if (oldPromise !== NO_PROMISE) {
      const oldPromiseId = promiseId - RING_SIZE;
      promiseMap.set(oldPromiseId, oldPromise);
    }
    // Set new promise
    return promiseRing[idx] = newPromise();
  }

  function getPromise(promiseId) {
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      const promise = promiseMap.get(promiseId);
      promiseMap.delete(promiseId);
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
    opsCache = Object.freeze(Object.fromEntries(opcall(0)));
  }

  function handleAsyncMsgFromRust() {
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
    if (typeof errorMap[className] !== "undefined") {
      throw new TypeError(`Error class for "${className}" already registered`);
    }
    errorMap[className] = errorClass;
  }

  function unwrapOpResult(res) {
    // .$err_class_name is a special key that should only exist on errors
    if (res?.$err_class_name) {
      const className = res.$err_class_name;
      const ErrorClass = errorMap[className];
      if (!ErrorClass) {
        throw new Error(
          `Unregistered error class: "${className}"\n  ${res.message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
        );
      }
      throw new ErrorClass(res.message);
    }
    return res;
  }

  function opAsync(opName, args = null, zeroCopy = null) {
    const promiseId = nextPromiseId++;
    const maybeError = dispatch(opName, promiseId, args, zeroCopy);
    // Handle sync error (e.g: error parsing args)
    if (maybeError) return unwrapOpResult(maybeError);
    return setPromise(promiseId).then(unwrapOpResult);
  }

  function opSync(opName, args = null, zeroCopy = null) {
    return unwrapOpResult(dispatch(opName, null, args, zeroCopy));
  }

  function resources() {
    return Object.fromEntries(opSync("op_resources"));
  }

  function close(rid) {
    opSync("op_close", rid);
  }

  Object.assign(window.Deno.core, {
    opAsync,
    opSync,
    ops,
    close,
    resources,
    registerErrorClass,
    handleAsyncMsgFromRust,
    syncOpsCache,
  });
})(this);
