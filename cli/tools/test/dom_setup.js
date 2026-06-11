// Copyright 2018-2026 the Deno authors. MIT license.

// This module is injected as a preload module when the DOM test environment
// is enabled (`deno test --dom`). The `__DENO_TEST_DOM_PACKAGE__` and
// `__DENO_TEST_DOM_LIB__` placeholders are replaced by the test runner
// before the module is evaluated.
import * as domLibrary from "__DENO_TEST_DOM_PACKAGE__";

const DOM_LIB = "__DENO_TEST_DOM_LIB__";
const DOM_URL = "http://localhost:3000/";

// Keys that are never copied from the DOM window onto globalThis. `window`,
// `self`, `top` and `parent` are instead defined as aliases of globalThis
// below, like in a real browser where they all point at the global scope.
const SKIP_KEYS = new Set([
  "window",
  "self",
  "top",
  "parent",
  "global",
  "globalThis",
  "undefined",
  "Infinity",
  "NaN",
  "eval",
  "constructor",
]);

// Keys that exist on Deno's globalThis but must be taken from the DOM
// library anyway so that DOM integration works: events constructed by test
// code must be dispatchable on DOM nodes, `new FormData(form)` must accept
// the library's form elements, and `location`/`history` must reflect the
// simulated browsing context. Everything else that already exists on
// globalThis (fetch, URL, streams, encoders, timers, crypto, ...) keeps
// Deno's native implementation.
const OVERRIDE_KEYS = new Set([
  "Event",
  "EventTarget",
  "DOMException",
  "FormData",
  "location",
  "history",
]);

function isClassLikeName(name) {
  const code = name.charCodeAt(0);
  return code >= 65 /* A */ && code <= 90 /* Z */;
}

// Any `Event` subclass provided by the DOM library overrides a same-named
// global (MessageEvent, ErrorEvent, CustomEvent, ...), so that all event
// classes come from a single realm and can be dispatched on DOM nodes.
function isEventClass(win, value) {
  return typeof value === "function" && value.prototype != null &&
    (value === win.Event || value.prototype instanceof win.Event);
}

function collectKeys(win) {
  const keys = new Set();
  let current = win;
  while (current != null && current !== Object.prototype) {
    for (const key of Object.getOwnPropertyNames(current)) {
      keys.add(key);
    }
    current = Object.getPrototypeOf(current);
  }
  return keys;
}

function shouldDefine(win, key) {
  if (SKIP_KEYS.has(key)) {
    return false;
  }
  if (!(key in globalThis)) {
    return true;
  }
  if (OVERRIDE_KEYS.has(key)) {
    return true;
  }
  let value;
  try {
    value = win[key];
  } catch {
    return false;
  }
  return isEventClass(win, value);
}

function populateGlobal(win) {
  const overrides = new Map();
  for (const key of collectKeys(win)) {
    if (!shouldDefine(win, key)) {
      continue;
    }
    let boundFunction;
    try {
      const value = win[key];
      if (typeof value === "function" && !isClassLikeName(key)) {
        boundFunction = value.bind(win);
      }
    } catch {
      continue;
    }
    try {
      Object.defineProperty(globalThis, key, {
        get() {
          if (overrides.has(key)) {
            return overrides.get(key);
          }
          if (boundFunction !== undefined) {
            return boundFunction;
          }
          return win[key];
        },
        set(value) {
          overrides.set(key, value);
          try {
            win[key] = value;
          } catch {
            // ignore read-only window properties
          }
        },
        configurable: true,
        enumerable: true,
      });
    } catch {
      // ignore non-configurable globals
    }
  }

  // In a browser `window`, `self`, `top` and `parent` all refer to the
  // global scope. Tests interact with the populated globalThis, so aliasing
  // these to globalThis keeps `window.document === document` etc. true.
  for (const key of ["window", "self", "top", "parent"]) {
    try {
      Object.defineProperty(globalThis, key, {
        value: globalThis,
        configurable: true,
        enumerable: true,
        writable: true,
      });
    } catch {
      // ignore non-configurable globals
    }
  }

  // Testing libraries discover the window through `document.defaultView`.
  try {
    Object.defineProperty(win.document, "defaultView", {
      get: () => globalThis,
      configurable: true,
      enumerable: true,
    });
  } catch {
    // ignore
  }
}

if (DOM_LIB === "jsdom") {
  const { JSDOM } = domLibrary;
  const dom = new JSDOM("<!DOCTYPE html>", {
    url: DOM_URL,
    pretendToBeVisual: true,
    runScripts: "dangerously",
  });
  // Expose the JSDOM instance for advanced use (matches what the vitest
  // jsdom environment does).
  Object.defineProperty(globalThis, "jsdom", {
    value: dom,
    configurable: true,
    enumerable: true,
    writable: true,
  });
  populateGlobal(dom.window);
} else {
  const { GlobalWindow, Window } = domLibrary;
  const win = new (GlobalWindow ?? Window)({
    url: DOM_URL,
    settings: { disableErrorCapturing: true },
  });
  populateGlobal(win);
}
