// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
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

const logSource = "JS";

let logLevel_ = null;
function logLevel() {
  if (logLevel_ === null) {
    logLevel_ = ops.op_bootstrap_log_level() || 3;
  }
  return logLevel_;
}

function log(...args) {
  if (logLevel() >= LogLevel.Debug) {
    // if we destructure `console` off `globalThis` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    globalThis.console.error(
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

export { createResolvable, getterOnly, log, nonEnumerable, readOnly, writable };
