// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-var
import processModule from "internal:deno_node/polyfills/process.ts";
import { Buffer as bufferModule } from "internal:deno_node/polyfills/buffer.ts";
import {
  clearInterval,
  clearTimeout,
  setInterval,
  setTimeout,
} from "internal:deno_node/polyfills/timers.ts";
import timers from "internal:deno_node/polyfills/timers.ts";

type GlobalType = {
  process: typeof processModule;
  Buffer: typeof bufferModule;
  setImmediate: typeof timers.setImmediate;
  clearImmediate: typeof timers.clearImmediate;
  setTimeout: typeof timers.setTimeout;
  clearTimeout: typeof timers.clearTimeout;
  setInterval: typeof timers.setInterval;
  clearInterval: typeof timers.clearInterval;
};

declare global {
  interface Window {
    global: GlobalType;
  }

  interface globalThis {
    global: GlobalType;
  }

  var global: GlobalType;
  var process: typeof processModule;
  var Buffer: typeof bufferModule;
  type Buffer = bufferModule;
  var setImmediate: typeof timers.setImmediate;
  var clearImmediate: typeof timers.clearImmediate;
}

Object.defineProperty(globalThis, "global", {
  value: new Proxy(globalThis, {
    get(target, prop, receiver) {
      switch (prop) {
        case "setInterval":
          return setInterval;
        case "setTimeout":
          return setTimeout;
        case "clearInterval":
          return clearInterval;
        case "clearTimeout":
          return clearTimeout;
        default:
          return Reflect.get(target, prop, receiver);
      }
    },
  }),
  writable: false,
  enumerable: false,
  configurable: true,
});

Object.defineProperty(globalThis, "process", {
  value: processModule,
  enumerable: false,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "Buffer", {
  value: bufferModule,
  enumerable: false,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "setImmediate", {
  value: timers.setImmediate,
  enumerable: true,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "clearImmediate", {
  value: timers.clearImmediate,
  enumerable: true,
  writable: true,
  configurable: true,
});

export {};
