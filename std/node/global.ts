// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import processModule from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";

Object.defineProperty(globalThis, "global", {
  value: globalThis,
  writable: false,
  enumerable: false,
  configurable: true,
});
type GlobalType = {
  process: typeof processModule;
  Buffer: typeof bufferModule;
};

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

declare global {
  interface Window {
    global: GlobalType;
  }

  interface globalThis {
    global: GlobalType;
  }

  var global: GlobalType;
  var process: typeof processModule;
  var Buffer: bufferModule;
}

export {};
