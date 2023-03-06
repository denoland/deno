// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const primordials = globalThis.__bootstrap.primordials;
const {
  Promise,
  SafeArrayIterator,
} = primordials;
let logDebug = false;
let logSource = "JS";

function setLogDebug(debug, source) {
  logDebug = debug;
  if (source) {
    logSource = source;
  }
}

function log(...args) {
  if (logDebug) {
    // if we destructure `console` off `globalThis` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    globalThis.console.log(
      `DEBUG ${logSource} -`,
      ...new SafeArrayIterator(args),
    );
  }
}

function createResolvable() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  promise.resolve = resolve;
  promise.reject = reject;
  return promise;
}

function writable(value) {
  return {
    value,
    writable: true,
    enumerable: true,
    configurable: true,
  };
}

function nonEnumerable(value) {
  return {
    value,
    writable: true,
    enumerable: false,
    configurable: true,
  };
}

function readOnly(value) {
  return {
    value,
    enumerable: true,
    writable: false,
    configurable: true,
  };
}

function getterOnly(getter) {
  return {
    get: getter,
    set() {},
    enumerable: true,
    configurable: true,
  };
}

export {
  createResolvable,
  getterOnly,
  log,
  nonEnumerable,
  readOnly,
  setLogDebug,
  writable,
};
