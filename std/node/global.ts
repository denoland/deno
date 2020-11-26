// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/// <reference path="./global.d.ts" />
import { process as processModule } from "./process.ts";
import { Buffer as bufferModule } from "./buffer.ts";

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

export {};
