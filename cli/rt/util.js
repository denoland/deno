// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/util.ts", [], function (exports_2, context_2) {
  "use strict";
  let logDebug, logSource;
  const __moduleName = context_2 && context_2.id;
  // @internal
  function setLogDebug(debug, source) {
    logDebug = debug;
    if (source) {
      logSource = source;
    }
  }
  exports_2("setLogDebug", setLogDebug);
  function log(...args) {
    if (logDebug) {
      // if we destructure `console` off `globalThis` too early, we don't bind to
      // the right console, therefore we don't log anything out.
      globalThis.console.log(`DEBUG ${logSource} -`, ...args);
    }
  }
  exports_2("log", log);
  // @internal
  function assert(cond, msg = "assert") {
    if (!cond) {
      throw Error(msg);
    }
  }
  exports_2("assert", assert);
  // @internal
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
  exports_2("createResolvable", createResolvable);
  // @internal
  function notImplemented() {
    throw new Error("not implemented");
  }
  exports_2("notImplemented", notImplemented);
  // @internal
  function immutableDefine(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    o,
    p,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }
  exports_2("immutableDefine", immutableDefine);
  return {
    setters: [],
    execute: function () {
      logDebug = false;
      logSource = "JS";
    },
  };
});
