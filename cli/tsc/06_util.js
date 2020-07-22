// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = Deno.core;
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
      const stringifiedArgs = args.map(JSON.stringify).join(" ");
      core.print(`DEBUG ${logSource} - ${stringifiedArgs}\n`);
    }
  }

  class AssertionError extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
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

  function notImplemented() {
    throw new Error("not implemented");
  }

  window.__bootstrap.util = {
    log,
    setLogDebug,
    notImplemented,
    createResolvable,
    assert,
    AssertionError,
  };
})(this);
