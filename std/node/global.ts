// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/// <reference path="./global.d.ts" />
import { process as processModule } from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";
import timers from "./timers.ts";

Object.defineProperty(globalThis, "global", {
  value: globalThis,
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
  enumerable: false,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "clearImmediate", {
  value: timers.clearImmediate,
  enumerable: false,
  writable: true,
  configurable: true,
});

export {};
