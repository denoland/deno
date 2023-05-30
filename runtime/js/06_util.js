// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const primordials = globalThis.__bootstrap.primordials;
const {
  Promise,
  SafeArrayIterator,
} = primordials;

// WARNING: Keep this in sync with Rust (search for LogLevel)
const LogLevel = {
  Error: 1,
  Warn: 2,
  Info: 3,
  Debug: 4,
};

let logLevel = 3;
let logSource = "JS";

function setLogLevel(level, source) {
  logLevel = level;
  if (source) {
    logSource = source;
  }
}

function logDebug(...args) {
  if (logLevel >= LogLevel.Debug) {
    // if we destructure `console` off `globalThis` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    globalThis.console.error(
      `DEBUG ${logSource} -`,
      ...new SafeArrayIterator(args),
    );
  }
}

function logWarn(...args) {
  if (logLevel >= LogLevel.Warn) {
    globalThis.console.warn(...new SafeArrayIterator(args));
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
  logDebug,
  logWarn,
  nonEnumerable,
  readOnly,
  setLogLevel,
  writable,
};
