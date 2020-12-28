// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import process from "./process.ts";
import { Buffer as buffer } from "./buffer.ts";

Object.defineProperty(globalThis, Symbol.toStringTag, {
  value: "global",
  writable: false,
  enumerable: false,
  configurable: true,
});

// deno-lint-ignore no-explicit-any
(globalThis as any)["global"] = globalThis;

// Define the type for the global declration
type Process = typeof process;
type Buffer = typeof buffer;

Object.defineProperty(globalThis, "process", {
  value: process,
  enumerable: false,
  writable: true,
  configurable: true,
});

declare global {
  const process: Process;
  const Buffer: Buffer;
}

Object.defineProperty(globalThis, "Buffer", {
  value: buffer,
  enumerable: false,
  writable: true,
  configurable: true,
});

export {};
