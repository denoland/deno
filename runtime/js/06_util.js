// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    decodeURIComponent,
    ObjectPrototypeIsPrototypeOf,
    Promise,
    SafeArrayIterator,
    StringPrototypeReplace,
    TypeError,
  } = window.__bootstrap.primordials;
  const { build } = window.__bootstrap.build;
  const { URLPrototype } = window.__bootstrap.url;
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

  // Keep in sync with `fromFileUrl()` in `std/path/win32.ts`.
  function pathFromURLWin32(url) {
    let p = StringPrototypeReplace(
      url.pathname,
      /^\/*([A-Za-z]:)(\/|$)/,
      "$1/",
    );
    p = StringPrototypeReplace(
      p,
      /\//g,
      "\\",
    );
    p = StringPrototypeReplace(
      p,
      /%(?![0-9A-Fa-f]{2})/g,
      "%25",
    );
    let path = decodeURIComponent(p);
    if (url.hostname != "") {
      // Note: The `URL` implementation guarantees that the drive letter and
      // hostname are mutually exclusive. Otherwise it would not have been valid
      // to append the hostname and path like this.
      path = `\\\\${url.hostname}${path}`;
    }
    return path;
  }

  // Keep in sync with `fromFileUrl()` in `std/path/posix.ts`.
  function pathFromURLPosix(url) {
    if (url.hostname !== "") {
      throw new TypeError(`Host must be empty.`);
    }

    return decodeURIComponent(
      StringPrototypeReplace(url.pathname, /%(?![0-9A-Fa-f]{2})/g, "%25"),
    );
  }

  function pathFromURL(pathOrUrl) {
    if (ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrUrl)) {
      if (pathOrUrl.protocol != "file:") {
        throw new TypeError("Must be a file URL.");
      }

      return build.os == "windows"
        ? pathFromURLWin32(pathOrUrl)
        : pathFromURLPosix(pathOrUrl);
    }
    return pathOrUrl;
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    pathFromURL,
  };

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
      enumerable: true,
      configurable: true,
    };
  }

  window.__bootstrap.util = {
    log,
    setLogDebug,
    createResolvable,
    pathFromURL,
    writable,
    nonEnumerable,
    readOnly,
    getterOnly,
  };
})(this);
