// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import {
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties
} from "./globals.ts";
import { mainRuntimeGlobalProperties, bootstrapMainRuntime } from "./main.ts";

import {
  workerRuntimeGlobalProperties,
  bootstrapWorkerRuntime
} from "./worker_main.ts";

function setupWorkerRuntimeGlobals(): void {
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
  Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
  Object.defineProperties(globalThis, eventTargetProperties);
  Object.defineProperties(globalThis, {
    bootstrapWorkerRuntime: {
      value: bootstrapWorkerRuntime,
      enumerable: false,
      writable: false,
      configurable: false
    }
  });
}

function setupMainRuntimeGlobals(): void {
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
  Object.defineProperties(globalThis, eventTargetProperties);
  Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
  Object.defineProperties(globalThis, {
    bootstrapMainRuntime: {
      value: bootstrapMainRuntime,
      enumerable: false,
      writable: false,
      configurable: false
    }
  });

  // Registers the handler for window.onload function.
  globalThis.addEventListener("load", (e: domTypes.Event): void => {
    const { onload } = globalThis;
    if (typeof onload === "function") {
      onload(e);
    }
  });
  // Registers the handler for window.onunload function.
  globalThis.addEventListener("unload", (e: domTypes.Event): void => {
    const { onunload } = globalThis;
    if (typeof onunload === "function") {
      onunload(e);
    }
  });
}

Object.defineProperties(globalThis, {
  setupWorkerRuntimeGlobals: {
    value: setupWorkerRuntimeGlobals,
    enumerable: false,
    writable: false,
    configurable: false
  },
  setupMainRuntimeGlobals: {
    value: setupMainRuntimeGlobals,
    enumerable: false,
    writable: false,
    configurable: false
  }
});
