// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { build } = window.__bootstrap.build;
  const internals = window.__bootstrap.internals;
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
      globalThis.console.log(`DEBUG ${logSource} -`, ...args);
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

  function immutableDefine(
    o,
    p,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value,
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }

  function pathFromURLWin32(url) {
    const hostname = url.hostname;
    const pathname = decodeURIComponent(url.pathname.replace(/\//g, "\\"));

    if (hostname !== "") {
      //TODO(actual-size) Node adds a punycode decoding step, we should consider adding this
      return `\\\\${hostname}${pathname}`;
    }

    const validPath = /^\\(?<driveLetter>[A-Za-z]):\\/;
    const matches = validPath.exec(pathname);

    if (!matches?.groups?.driveLetter) {
      throw new TypeError("A URL with the file schema must be absolute.");
    }

    // we don't want a leading slash on an absolute path in Windows
    return pathname.slice(1);
  }

  function pathFromURLPosix(url) {
    if (url.hostname !== "") {
      throw new TypeError(`Host must be empty.`);
    }

    return decodeURIComponent(url.pathname);
  }

  function pathFromURL(pathOrUrl) {
    if (pathOrUrl instanceof URL) {
      if (pathOrUrl.protocol != "file:") {
        throw new TypeError("Must be a file URL.");
      }

      return build.os == "windows"
        ? pathFromURLWin32(pathOrUrl)
        : pathFromURLPosix(pathOrUrl);
    }
    return pathOrUrl;
  }

  internals.exposeForTest("pathFromURL", pathFromURL);

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
      configurable: true,
    };
  }

  function readOnly(value) {
    return {
      value,
      enumerable: true,
    };
  }

  function getterOnly(getter) {
    return {
      get: getter,
      enumerable: true,
    };
  }

  window.__bootstrap.util = {
    log,
    setLogDebug,
    notImplemented,
    createResolvable,
    assert,
    AssertionError,
    immutableDefine,
    pathFromURL,
    writable,
    nonEnumerable,
    readOnly,
    getterOnly,
  };
})(this);
