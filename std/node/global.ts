// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

Object.defineProperty(globalThis, Symbol.toStringTag, {
  value: "global",
  writable: false,
  enumerable: false,
  configurable: true,
});

// deno-lint-ignore no-explicit-any
(globalThis as any)["global"] = globalThis;

export {};
