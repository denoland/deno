// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { bootstrapMainRuntime } from "./runtime_main.ts";
import { bootstrapWorkerRuntime } from "./runtime_worker.ts";

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete (Object.prototype as any).__proto__;

Object.defineProperties(globalThis, {
  bootstrapMainRuntime: {
    value: bootstrapMainRuntime,
    enumerable: false,
    writable: false,
    configurable: false,
  },
  bootstrapWorkerRuntime: {
    value: bootstrapWorkerRuntime,
    enumerable: false,
    writable: false,
    configurable: false,
  },
});
